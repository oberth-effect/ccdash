//! What a run of transcript lines says: your prompts and Claude's text in
//! order, the titles, where and on what it ran, and its numbers. Only
//! complete lines count, so a chunk may end in the middle of one; a line
//! that is not JSON (or not a transcript line) is skipped.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::util::{parse_rfc3339, truncate};

/// Bytes of one prompt or reply kept for search and reading.
pub const MAX_TEXT: usize = 16 << 10;
/// Bytes of the first and last prompt and the last reply on the session's row.
pub const MAX_PREVIEW: usize = 4 << 10;
/// Tools whose `file_path` (or `notebook_path`) counts as a file edited.
const EDITS: &[&str] = &["Edit", "MultiEdit", "Write", "NotebookEdit"];
/// What Claude Code appends to its recaps.
const RECAP_HINT: &str = "(disable recaps in /config)";

/// Who said a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    You,
    Claude,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::Claude => "claude",
        }
    }
}

/// A prompt or a block of Claude's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub at: i64,
    pub role: Role,
    pub text: String,
}

/// The session's cost so far as Claude Code keeps it (cumulative, restored
/// on a resume).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    pub usd: f64,
    pub lines_added: i64,
    pub lines_removed: i64,
}

/// What a chunk of lines adds to a session: texts in order, the newest
/// value of what is replaced, counts of what adds up.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Chunk {
    pub cwd: Option<String>,
    pub first_at: Option<i64>,
    pub last_at: Option<i64>,
    pub custom_title: Option<String>,
    pub ai_title: Option<String>,
    pub summary: Option<String>,
    pub recap: Option<String>,
    pub texts: Vec<Text>,
    pub tools: i64,
    pub active_ms: i64,
    pub compactions: i64,
    pub models: BTreeMap<String, i64>,
    pub files: BTreeMap<String, i64>,
    pub branch: Option<String>,
    pub version: Option<String>,
    pub entrypoint: Option<String>,
    pub cost: Option<Cost>,
    pub continued_in: Option<String>,
}

impl Chunk {
    pub fn prompts(&self) -> impl Iterator<Item = &Text> {
        self.texts.iter().filter(|t| t.role == Role::You)
    }

    pub fn replies(&self) -> impl Iterator<Item = &Text> {
        self.texts.iter().filter(|t| t.role == Role::Claude)
    }
}

/// Reads the complete lines of `bytes`; returns what they say and how many
/// bytes they took (up to and including the last newline).
pub fn chunk(bytes: &[u8]) -> (Chunk, usize) {
    let used = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let mut out = Chunk::default();
    for line in bytes[..used].split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if let Ok(line) = serde_json::from_slice::<Line>(line) {
            out.add(line);
        }
    }
    (out, used)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    subtype: Option<String>,
    timestamp: Option<String>,
    #[serde(default)]
    is_sidechain: bool,
    #[serde(default)]
    is_meta: bool,
    #[serde(default)]
    is_compact_summary: bool,
    prompt_source: Option<String>,
    cwd: Option<String>,
    git_branch: Option<String>,
    version: Option<String>,
    entrypoint: Option<String>,
    message: Option<Message>,
    custom_title: Option<String>,
    ai_title: Option<String>,
    summary: Option<String>,
    /// A system line's text (`away_summary`: Claude Code's recap).
    content: Option<serde_json::Value>,
    duration_ms: Option<i64>,
    #[serde(rename = "totalCostUSD")]
    total_cost_usd: Option<f64>,
    total_lines_added: Option<i64>,
    total_lines_removed: Option<i64>,
    continued_in_session_id: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    model: Option<String>,
    content: Option<Content>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    name: Option<String>,
    input: Option<Input>,
}

#[derive(Deserialize)]
struct Input {
    file_path: Option<String>,
    notebook_path: Option<String>,
}

impl Chunk {
    fn add(&mut self, line: Line) {
        let at = line.timestamp.as_deref().and_then(parse_rfc3339);
        if let Some(at) = at {
            self.first_at.get_or_insert(at);
            self.last_at = Some(self.last_at.map_or(at, |last| last.max(at)));
        }
        if self.cwd.is_none() {
            self.cwd = line.cwd.clone().filter(|c| !c.is_empty());
        }
        if line.git_branch.as_deref().is_some_and(|b| !b.is_empty()) {
            self.branch.clone_from(&line.git_branch);
        }
        if line.version.is_some() {
            self.version.clone_from(&line.version);
        }
        if self.entrypoint.is_none() {
            self.entrypoint.clone_from(&line.entrypoint);
        }
        let at = at.or(self.last_at).unwrap_or(0);
        match line.kind.as_str() {
            "user" => self.user(line, at),
            "assistant" if !line.is_sidechain => self.assistant(line, at),
            "system" => match line.subtype.as_deref() {
                Some("turn_duration") => self.active_ms += line.duration_ms.unwrap_or(0).max(0),
                Some("compact_boundary") => self.compactions += 1,
                Some("away_summary") => {
                    if let Some(text) = line.content.as_ref().and_then(|c| c.as_str()) {
                        let text = text.trim().trim_end_matches(RECAP_HINT).trim();
                        if !text.is_empty() {
                            self.recap = Some(truncate(text, MAX_PREVIEW));
                        }
                    }
                }
                _ => {}
            },
            "custom-title" => set(&mut self.custom_title, line.custom_title),
            "ai-title" => set(&mut self.ai_title, line.ai_title),
            "summary" => set(&mut self.summary, line.summary),
            "cost-state" => {
                if let Some(usd) = line.total_cost_usd {
                    self.cost = Some(Cost {
                        usd,
                        lines_added: line.total_lines_added.unwrap_or(0),
                        lines_removed: line.total_lines_removed.unwrap_or(0),
                    });
                }
            }
            "continued-in" => set(&mut self.continued_in, line.continued_in_session_id),
            _ => {}
        }
    }

    fn user(&mut self, line: Line, at: i64) {
        if line.is_sidechain
            || line.is_meta
            || line.is_compact_summary
            || line.prompt_source.as_deref() == Some("system")
        {
            return;
        }
        let text = match line.message.and_then(|m| m.content) {
            Some(Content::Text(text)) => prompt_text(&text),
            Some(Content::Blocks(blocks)) => {
                // NOTE: tool results come back as user lines; a prompt has text blocks only (and images)
                if blocks.iter().any(|b| b.kind == "tool_result") {
                    return;
                }
                let joined: Vec<&str> = blocks
                    .iter()
                    .filter(|b| b.kind == "text")
                    .filter_map(|b| b.text.as_deref())
                    .collect();
                prompt_text(&joined.join("\n\n"))
            }
            None => None,
        };
        if let Some(text) = text {
            self.texts.push(Text {
                at,
                role: Role::You,
                text: truncate(&text, MAX_TEXT),
            });
        }
    }

    fn assistant(&mut self, line: Line, at: i64) {
        let Some(message) = line.message else { return };
        if let Some(model) = message.model.filter(|m| !m.starts_with('<')) {
            *self.models.entry(model).or_default() += 1;
        }
        let Some(Content::Blocks(blocks)) = message.content else {
            return;
        };
        for block in blocks {
            match block.kind.as_str() {
                "text" => {
                    let text = block.text.as_deref().unwrap_or_default().trim();
                    if !text.is_empty() {
                        self.texts.push(Text {
                            at,
                            role: Role::Claude,
                            text: truncate(text, MAX_TEXT),
                        });
                    }
                }
                "tool_use" => {
                    self.tools += 1;
                    let edits = block.name.as_deref().is_some_and(|n| EDITS.contains(&n));
                    if let Some(path) = block
                        .input
                        .filter(|_| edits)
                        .and_then(|i| i.file_path.or(i.notebook_path))
                    {
                        *self.files.entry(path).or_default() += 1;
                    }
                }
                _ => {}
            }
        }
    }
}

/// A slash command without arguments (`/model`, `/clear`): a prompt, but
/// not what the session was about.
pub fn bare_command(text: &str) -> bool {
    text.starts_with('/') && !text.contains(char::is_whitespace)
}

fn set(slot: &mut Option<String>, value: Option<String>) {
    if let Some(value) = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) {
        *slot = Some(value);
    }
}

/// A user line's text as the prompt it was, or None for what Claude Code
/// writes there itself: command output, caveats, notices.
fn prompt_text(raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() || text.starts_with("[Request interrupted") {
        return None;
    }
    if let Some(name) = between(text, "<command-name>", "</command-name>") {
        let args = between(text, "<command-args>", "</command-args>").unwrap_or_default();
        let command = format!("{} {}", name.trim(), args.trim());
        return Some(command.trim().to_owned()).filter(|c| !c.is_empty());
    }
    if [
        "<local-command-",
        "<command-message>",
        "<system-reminder>",
        "<task-notification>",
        "<channel ",
        "<bash-stdout>",
        "<bash-stderr>",
    ]
    .iter()
    .any(|tag| text.starts_with(tag))
    {
        return None;
    }
    if let Some(command) = between(text, "<bash-input>", "</bash-input>") {
        return Some(format!("! {}", command.trim()));
    }
    Some(text.to_owned())
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(lines: &[serde_json::Value]) -> Vec<u8> {
        let mut out = Vec::new();
        for line in lines {
            out.extend(serde_json::to_vec(line).unwrap());
            out.push(b'\n');
        }
        out
    }

    fn user(content: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
        let mut line = serde_json::json!({
            "type": "user",
            "timestamp": "2026-10-22T10:00:00Z",
            "cwd": "/home/me/w",
            "gitBranch": "main",
            "version": "2.1.312",
            "entrypoint": "cli",
            "isSidechain": false,
            "message": { "role": "user", "content": content },
        });
        line.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        line
    }

    fn texts(c: &Chunk) -> Vec<(Role, &str)> {
        c.texts.iter().map(|t| (t.role, t.text.as_str())).collect()
    }

    #[test]
    fn prompts_and_what_is_not_one() {
        let bytes = lines(&[
            user("Find me a webcam".into(), serde_json::json!({"promptSource": "typed"})),
            user(
                serde_json::json!([{"type": "tool_result", "tool_use_id": "x", "content": "ok"}]),
                serde_json::json!({}),
            ),
            user(
                "<task-notification>done</task-notification>".into(),
                serde_json::json!({"promptSource": "system"}),
            ),
            user("caveat".into(), serde_json::json!({"isMeta": true})),
            user(
                "<command-name>/review</command-name>\n<command-message>review</command-message>\n<command-args>src/</command-args>"
                    .into(),
                serde_json::json!({}),
            ),
            user(
                "<local-command-stdout>output</local-command-stdout>".into(),
                serde_json::json!({}),
            ),
            user("summary of before".into(), serde_json::json!({"isCompactSummary": true})),
            user(
                serde_json::json!([{"type": "text", "text": "look at this"}, {"type": "image"}]),
                serde_json::json!({}),
            ),
            user("[Request interrupted by user]".into(), serde_json::json!({})),
            user("side".into(), serde_json::json!({"isSidechain": true})),
        ]);
        let (c, used) = chunk(&bytes);
        assert_eq!(used, bytes.len());
        assert_eq!(
            texts(&c),
            [
                (Role::You, "Find me a webcam"),
                (Role::You, "/review src/"),
                (Role::You, "look at this"),
            ]
        );
        assert_eq!(c.cwd.as_deref(), Some("/home/me/w"));
        assert_eq!(c.branch.as_deref(), Some("main"));
        assert_eq!(c.entrypoint.as_deref(), Some("cli"));
    }

    #[test]
    fn replies_tools_titles_and_numbers() {
        let bytes = lines(&[
            user("go".into(), serde_json::json!({})),
            serde_json::json!({
                "type": "assistant", "timestamp": "2026-10-22T10:00:05Z",
                "message": {"model": "claude-opus-5-5", "content": [
                    {"type": "thinking", "thinking": "hm"},
                    {"type": "text", "text": "  On it.  "},
                    {"type": "tool_use", "name": "Edit", "input": {"file_path": "/w/a.rs", "old_string": "x"}},
                    {"type": "tool_use", "name": "Bash", "input": {"command": "ls", "file_path": "/nope"}},
                ]},
            }),
            serde_json::json!({
                "type": "assistant", "timestamp": "2026-10-22T10:00:06Z", "isSidechain": true,
                "message": {"model": "claude-haiku-5-5", "content": [{"type": "text", "text": "sub"}]},
            }),
            serde_json::json!({"type": "assistant", "timestamp": "2026-10-22T10:00:07Z",
                "message": {"model": "<synthetic>", "content": [{"type": "tool_use", "name": "Write", "input": {"file_path": "/w/a.rs"}}]}}),
            serde_json::json!({"type": "system", "subtype": "turn_duration", "durationMs": 1500, "timestamp": "2026-10-22T10:01:00Z"}),
            serde_json::json!({"type": "system", "subtype": "compact_boundary", "timestamp": "2026-10-22T10:02:00Z"}),
            serde_json::json!({"type": "system", "subtype": "away_summary", "content": "We fixed it. Next: ship. (disable recaps in /config)"}),
            serde_json::json!({"type": "ai-title", "aiTitle": "First"}),
            serde_json::json!({"type": "ai-title", "aiTitle": "Webcam search"}),
            serde_json::json!({"type": "custom-title", "customTitle": " "}),
            serde_json::json!({"type": "cost-state", "totalCostUSD": 1.5, "totalLinesAdded": 10, "totalLinesRemoved": 2}),
            serde_json::json!({"type": "continued-in", "continuedInSessionId": "abc"}),
            serde_json::json!({"type": "api-request-blob", "message": {"huge": true}}),
        ]);
        let (c, _) = chunk(&bytes);
        assert_eq!(texts(&c), [(Role::You, "go"), (Role::Claude, "On it.")]);
        assert_eq!(c.tools, 3);
        assert_eq!(c.files.get("/w/a.rs"), Some(&2));
        assert_eq!(c.files.len(), 1);
        assert_eq!(c.models.get("claude-opus-5-5"), Some(&1));
        assert_eq!(c.models.len(), 1);
        assert_eq!(c.active_ms, 1500);
        assert_eq!(c.compactions, 1);
        assert_eq!(c.recap.as_deref(), Some("We fixed it. Next: ship."));
        assert_eq!(c.ai_title.as_deref(), Some("Webcam search"));
        assert_eq!(c.custom_title, None);
        assert_eq!(c.cost.map(|c| c.usd), Some(1.5));
        assert_eq!(c.continued_in.as_deref(), Some("abc"));
        assert_eq!(c.first_at, parse_rfc3339("2026-10-22T10:00:00Z"));
        assert_eq!(c.last_at, parse_rfc3339("2026-10-22T10:02:00Z"));
    }

    #[test]
    fn partial_and_broken_lines() {
        let mut bytes = lines(&[user("one".into(), serde_json::json!({}))]);
        bytes.extend(b"not json\n");
        let whole = bytes.len();
        bytes.extend(br#"{"type":"user","message":{"content":"two"#);
        let (c, used) = chunk(&bytes);
        assert_eq!(used, whole);
        assert_eq!(texts(&c), [(Role::You, "one")]);
        assert_eq!(chunk(b"no newline yet").1, 0);
    }

    #[test]
    fn long_texts_are_cut() {
        let long = "x".repeat(MAX_TEXT * 2);
        let (c, _) = chunk(&lines(&[user(long.into(), serde_json::json!({}))]));
        assert!(c.texts[0].text.len() <= MAX_TEXT);
        assert!(c.texts[0].text.ends_with('…'));
    }

    #[test]
    fn shell_escapes_read_as_commands() {
        assert_eq!(
            prompt_text("<bash-input>ls -la</bash-input>").as_deref(),
            Some("! ls -la")
        );
        assert_eq!(prompt_text("<bash-stdout>x</bash-stdout>"), None);
    }
}
