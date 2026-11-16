//! Files the owner attaches to a prompt in the panel: a photo of some
//! hardware, a screenshot, a recording, a document. A channel message is
//! text and nothing else, so an attachment travels as a marker in the
//! prompt (`[ccdash-attachment <id>]`); the file itself waits on the hub
//! (`hub::attachments`), and the collector of the session's machine
//! fetches it when the prompt is delivered and puts the local path in the
//! marker's place (`collector::attachments`), which is what Claude reads
//! with its own tools. This module is what both sides share: the row, the
//! marker, the file's kind from its first bytes, the table.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    mime TEXT NOT NULL,
    bytes INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    session TEXT,
    workspace TEXT,
    created_at INTEGER NOT NULL,
    fetched_at INTEGER,
    pinned INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS attachments_created ON attachments (created_at);
";

/// What an attachment is to Claude and to the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "types.ts", rename = "AttachmentKind")]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Claude sees it (PNG, JPEG, GIF, WebP).
    Image,
    Audio,
    Video,
    /// Anything else; a PDF or text Claude reads, the rest it opens with tools.
    File,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Audio => "audio",
            Kind::Video => "video",
            Kind::File => "file",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "image" => Kind::Image,
            "audio" => Kind::Audio,
            "video" => Kind::Video,
            _ => Kind::File,
        }
    }
}

/// A file kept on the hub for a prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "types.ts")]
pub struct Attachment {
    pub id: String,
    /// The file's name as uploaded, made safe (`safe_name`).
    pub name: String,
    pub kind: Kind,
    /// From the file's first bytes, never from what the browser said.
    pub mime: String,
    #[ts(type = "number")]
    pub bytes: u64,
    pub sha256: String,
    /// The session it was attached in, when the panel said.
    pub session: Option<String>,
    pub workspace: Option<String>,
    pub created_at: i64,
    /// When a collector last fetched it for a session; None: never sent.
    pub fetched_at: Option<i64>,
    /// Kept past `attachments.retention_days` and the quota.
    pub pinned: bool,
    /// When the retention drops it; None when pinned or kept for good.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expires_at: Option<i64>,
    /// A link the panel can show or play it through for a while, without
    /// its token (`/attachments/-/view/<key>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub url: Option<String>,
}

const COLUMNS: &str = "id, name, kind, mime, bytes, sha256, session, workspace, created_at, \
    fetched_at, pinned";

fn from_row(r: &Row) -> rusqlite::Result<Attachment> {
    Ok(Attachment {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: Kind::parse(&r.get::<_, String>(2)?),
        mime: r.get(3)?,
        bytes: r.get::<_, i64>(4)?.max(0) as u64,
        sha256: r.get(5)?,
        session: r.get(6)?,
        workspace: r.get(7)?,
        created_at: r.get(8)?,
        fetched_at: r.get(9)?,
        pinned: r.get::<_, i64>(10)? != 0,
        expires_at: None,
        url: None,
    })
}

pub fn insert(conn: &Connection, a: &Attachment) -> Result<()> {
    conn.execute(
        "INSERT INTO attachments (id, name, kind, mime, bytes, sha256, session, workspace,
            created_at, fetched_at, pinned)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            a.id,
            a.name,
            a.kind.as_str(),
            a.mime,
            a.bytes as i64,
            a.sha256,
            a.session,
            a.workspace,
            a.created_at,
            a.fetched_at,
            a.pinned
        ],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Attachment>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM attachments WHERE id = ?1"),
            [id],
            from_row,
        )
        .optional()?)
}

/// Every attachment, newest first; `session` narrows to one session's.
pub fn list(conn: &Connection, session: Option<&str>) -> Result<Vec<Attachment>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM attachments WHERE (?1 IS NULL OR session = ?1)
            ORDER BY created_at DESC, id"
    ))?;
    let rows = stmt.query_map([session], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The bytes kept, all attachments together.
pub fn total_bytes(conn: &Connection) -> Result<u64> {
    let total: i64 =
        conn.query_row("SELECT COALESCE(SUM(bytes), 0) FROM attachments", [], |r| {
            r.get(0)
        })?;
    Ok(total.max(0) as u64)
}

pub fn set_pinned(conn: &Connection, id: &str, pinned: bool) -> Result<bool> {
    Ok(conn.execute(
        "UPDATE attachments SET pinned = ?2 WHERE id = ?1",
        params![id, pinned],
    )? > 0)
}

pub fn mark_fetched(conn: &Connection, id: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE attachments SET fetched_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.execute("DELETE FROM attachments WHERE id = ?1", [id])? > 0)
}

/// The ids to drop, oldest first: unpinned ones made before `before`
/// (None keeps them whatever their age), then as many more unpinned ones
/// as it takes for the rest plus `incoming` bytes to fit `quota` (0: no
/// quota).
pub fn to_evict(
    conn: &Connection,
    before: Option<i64>,
    quota: u64,
    incoming: u64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, bytes, created_at FROM attachments WHERE pinned = 0 ORDER BY created_at, id",
    )?;
    let unpinned: Vec<(String, u64, i64)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get::<_, i64>(1)?.max(0) as u64, r.get(2)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut total = total_bytes(conn)?.saturating_add(incoming);
    let mut out = Vec::new();
    for (id, bytes, created) in unpinned {
        let old = before.is_some_and(|t| created < t);
        let over = quota > 0 && total > quota;
        if !old && !over {
            continue;
        }
        total = total.saturating_sub(bytes);
        out.push(id);
    }
    Ok(out)
}

/// Hex characters of an id (`new_id` on the hub draws them).
pub const ID_LEN: usize = 24;

pub fn valid_id(id: &str) -> bool {
    id.len() == ID_LEN && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

const MARKER_OPEN: &str = "[ccdash-attachment ";

/// The marker a prompt carries for attachment `id`; the panel writes them
/// (`web/src/lib/attachments.ts`).
#[cfg(test)]
pub fn marker(id: &str) -> String {
    format!("{MARKER_OPEN}{id}]")
}

/// The ids of the markers in `text`, in order, each once.
pub fn marked(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(MARKER_OPEN) {
        let after = &rest[at + MARKER_OPEN.len()..];
        let id = after.get(..ID_LEN).unwrap_or_default();
        if valid_id(id) && after[ID_LEN..].starts_with(']') {
            if !out.iter().any(|seen| seen == id) {
                out.push(id.to_owned());
            }
            rest = &after[ID_LEN + 1..];
        } else {
            rest = after;
        }
    }
    out
}

/// `text` with every marker replaced by what `line` says for its id.
pub fn resolve(text: &str, mut line: impl FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(MARKER_OPEN) {
        let after = &rest[at + MARKER_OPEN.len()..];
        let id = after.get(..ID_LEN).unwrap_or_default();
        if valid_id(id) && after[ID_LEN..].starts_with(']') {
            out.push_str(&rest[..at]);
            out.push_str(&line(id));
            rest = &after[ID_LEN + 1..];
        } else {
            out.push_str(&rest[..at + MARKER_OPEN.len()]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// What Claude reads where the marker was: the file on its machine.
/// NOTE: the panel finds the id in the path (`…/attachments/<id>/<name>`)
/// to show the file in the chat; keep the two in step (`web/src/lib/attachments.ts`).
pub fn delivered(kind: Kind, path: &str) -> String {
    let what = match kind {
        Kind::Image => "image",
        Kind::Audio => "audio recording, which you cannot hear: use tools on the file",
        Kind::Video => "video, which you cannot watch: use tools on the file",
        Kind::File => "file",
    };
    format!("[Attached {what}: {path}]")
}

/// What Claude reads when the file could not be put on its machine.
pub fn undelivered(id: &str, why: &str) -> String {
    format!(
        "[An attachment ({id}) did not reach this machine: {why}. Tell the user it is missing.]"
    )
}

/// A file name that is one path component and nothing a shell or a
/// terminal minds: letters, digits, `.`, `_`, `-`; the rest becomes `_`.
pub fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let mut out: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    while out.starts_with('.') {
        out.remove(0);
    }
    if out.chars().count() > 120 {
        let ext = out
            .rfind('.')
            .map(|i| out[i..].to_owned())
            .filter(|e| e.len() <= 12);
        out = out.chars().take(100).collect();
        if let Some(ext) = ext {
            out.push_str(&ext);
        }
    }
    if out.is_empty() { "file".into() } else { out }
}

/// The kind and media type of a file from its first bytes. What is not
/// recognised is a plain file, whatever its name says.
pub fn sniff(head: &[u8]) -> (Kind, &'static str) {
    let at = |offset: usize, magic: &[u8]| head.get(offset..offset + magic.len()) == Some(magic);
    if at(0, b"\xff\xd8\xff") {
        return (Kind::Image, "image/jpeg");
    }
    if at(0, b"\x89PNG\r\n\x1a\n") {
        return (Kind::Image, "image/png");
    }
    if at(0, b"GIF87a") || at(0, b"GIF89a") {
        return (Kind::Image, "image/gif");
    }
    if at(0, b"RIFF") && at(8, b"WEBP") {
        return (Kind::Image, "image/webp");
    }
    if at(0, b"RIFF") && at(8, b"WAVE") {
        return (Kind::Audio, "audio/wav");
    }
    if at(0, b"ID3") || at(0, b"\xff\xfb") || at(0, b"\xff\xf3") || at(0, b"\xff\xf2") {
        return (Kind::Audio, "audio/mpeg");
    }
    if at(0, b"fLaC") {
        return (Kind::Audio, "audio/flac");
    }
    if at(0, b"OggS") {
        // NOTE: Ogg holds Opus or Vorbis sound far more often than Theora video
        return (Kind::Audio, "audio/ogg");
    }
    if at(0, b"\x1a\x45\xdf\xa3") {
        // NOTE: a recorder's sound-only WebM has no video track id in its first bytes
        let video = head.windows(5).any(|w| w == b"V_VP8" || w == b"V_VP9")
            || head.windows(5).any(|w| w == b"V_AV1")
            || head.windows(6).any(|w| w == b"V_MPEG");
        return if video {
            (Kind::Video, "video/webm")
        } else {
            (Kind::Audio, "audio/webm")
        };
    }
    if at(4, b"ftyp") {
        let brand = head.get(8..12).unwrap_or_default();
        return match brand {
            b"M4A " | b"M4B " => (Kind::Audio, "audio/mp4"),
            b"qt  " => (Kind::Video, "video/quicktime"),
            b"heic" | b"heix" | b"mif1" | b"heif" => (Kind::File, "image/heic"),
            b"avif" => (Kind::File, "image/avif"),
            _ => (Kind::Video, "video/mp4"),
        };
    }
    if at(0, b"%PDF-") {
        return (Kind::File, "application/pdf");
    }
    if !head.is_empty() && std::str::from_utf8(head).is_ok() && !head.contains(&0) {
        return (Kind::File, "text/plain; charset=utf-8");
    }
    (Kind::File, "application/octet-stream")
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0123456789abcdef01234567";
    const B: &str = "89abcdef0123456789abcdef";

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn
    }

    fn row(id: &str, bytes: u64, created_at: i64, pinned: bool) -> Attachment {
        Attachment {
            id: id.into(),
            name: "a.jpg".into(),
            kind: Kind::Image,
            mime: "image/jpeg".into(),
            bytes,
            sha256: "x".into(),
            session: Some("s".into()),
            workspace: None,
            created_at,
            fetched_at: None,
            pinned,
            expires_at: None,
            url: None,
        }
    }

    #[test]
    fn markers_are_found_and_resolved() {
        let text = format!(
            "look\n{}\n{} and again {}\n[ccdash-attachment nope] [ccdash-attachment ",
            marker(A),
            marker(B),
            marker(A)
        );
        assert_eq!(marked(&text), [A, B]);
        let out = resolve(&text, |id| format!("<{}>", &id[..4]));
        assert_eq!(
            out,
            "look\n<0123>\n<89ab> and again <0123>\n[ccdash-attachment nope] [ccdash-attachment "
        );
        assert_eq!(resolve("plain", |_| unreachable!()), "plain");
        assert!(marked("[ccdash-attachment 0123]").is_empty());
    }

    #[test]
    fn names_stay_one_harmless_component() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(
            safe_name("C:\\Users\\me\\My photo (1).JPG"),
            "My_photo__1_.JPG"
        );
        assert_eq!(safe_name(".bashrc"), "bashrc");
        assert_eq!(safe_name("deska č. 2.jpg"), "deska_č._2.jpg");
        assert_eq!(safe_name("$(rm -rf ~);`x`"), "__rm_-rf_____x_");
        assert_eq!(safe_name(""), "file");
        let long = format!("{}.jpeg", "a".repeat(300));
        let safe = safe_name(&long);
        assert!(safe.chars().count() <= 120 && safe.ends_with(".jpeg"));
    }

    #[test]
    fn kinds_come_from_the_bytes() {
        assert_eq!(sniff(b"\xff\xd8\xff\xe0rest"), (Kind::Image, "image/jpeg"));
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n...."), (Kind::Image, "image/png"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), (Kind::Image, "image/webp"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), (Kind::Audio, "audio/wav"));
        assert_eq!(sniff(b"\0\0\0\x18ftypmp42"), (Kind::Video, "video/mp4"));
        assert_eq!(sniff(b"\0\0\0\x18ftypM4A "), (Kind::Audio, "audio/mp4"));
        assert_eq!(sniff(b"\0\0\0\x18ftypheic"), (Kind::File, "image/heic"));
        assert_eq!(
            sniff(b"\x1a\x45\xdf\xa3....webm....A_OPUS"),
            (Kind::Audio, "audio/webm")
        );
        assert_eq!(
            sniff(b"\x1a\x45\xdf\xa3....webm....V_VP9"),
            (Kind::Video, "video/webm")
        );
        assert_eq!(sniff(b"%PDF-1.7"), (Kind::File, "application/pdf"));
        // NOTE: an SVG or a page is text to the hub, so it is never shown as an image
        assert_eq!(
            sniff(b"<svg onload=alert(1)>"),
            (Kind::File, "text/plain; charset=utf-8")
        );
        assert_eq!(
            sniff(b"\0\x01\x02"),
            (Kind::File, "application/octet-stream")
        );
        assert_eq!(sniff(b""), (Kind::File, "application/octet-stream"));
    }

    #[test]
    fn rows_and_eviction() {
        let conn = db();
        insert(&conn, &row(A, 600, 10, false)).unwrap();
        insert(&conn, &row(B, 600, 20, false)).unwrap();
        let pinned = "ffffffffffffffffffffffff";
        insert(&conn, &row(pinned, 600, 1, true)).unwrap();
        assert_eq!(total_bytes(&conn).unwrap(), 1800);
        assert_eq!(list(&conn, None).unwrap()[0].id, B);
        assert_eq!(list(&conn, Some("other")).unwrap().len(), 0);

        // nothing old, no quota
        assert!(to_evict(&conn, Some(5), 0, 0).unwrap().is_empty());
        // age: the pinned one stays whatever its age
        assert_eq!(to_evict(&conn, Some(15), 0, 0).unwrap(), [A]);
        // quota: oldest unpinned first, only as many as it takes
        assert_eq!(to_evict(&conn, None, 1800, 100).unwrap(), [A]);
        assert_eq!(to_evict(&conn, None, 1000, 0).unwrap(), [A, B]);
        assert!(to_evict(&conn, None, 1800, 0).unwrap().is_empty());

        assert!(set_pinned(&conn, A, true).unwrap());
        assert_eq!(
            to_evict(&conn, Some(15), 0, 0).unwrap(),
            Vec::<String>::new()
        );
        mark_fetched(&conn, A, 99).unwrap();
        let a = get(&conn, A).unwrap().unwrap();
        assert!(a.pinned && a.fetched_at == Some(99));
        assert!(remove(&conn, A).unwrap());
        assert!(!remove(&conn, A).unwrap());
        assert!(get(&conn, A).unwrap().is_none());
    }

    #[test]
    fn delivered_lines_name_the_path() {
        assert_eq!(
            delivered(
                Kind::Image,
                "/home/u/.local/state/ccdash/attachments/x/a.jpg"
            ),
            "[Attached image: /home/u/.local/state/ccdash/attachments/x/a.jpg]"
        );
        assert!(delivered(Kind::Video, "/p").contains("cannot watch"));
        assert!(undelivered(A, "the hub is unreachable").contains(A));
    }
}
