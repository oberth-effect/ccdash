//! A scripted session as the hook payloads and statusline readings Claude
//! Code would have sent for it, with their times, and the process numbers
//! its machine would have read.

use serde_json::{Value, json};

use super::script::{Now, ROOT, Session, Step};
use crate::{
    state::ProcPoll,
    wire::{Envelope, Source},
};

/// How long Claude thinks before a step: 45 s to 2 min.
fn think(s: &Session, i: usize, j: usize) -> i64 {
    let seed = s.id.bytes().fold((i * 131 + j) as u64, |a, c| {
        a.wrapping_mul(31) + u64::from(c)
    });
    45_000 + (noise(seed) * 75_000.0) as i64
}
/// How often the collector polls the processes.
pub const POLL_MS: i64 = 20_000;
/// Dollars of usage a five-hour window and a week hold (100 %).
const FIVE_HOUR_USD: f64 = 8.0;
const WEEK_USD: f64 = 40.0;

pub struct Built {
    pub envelopes: Vec<Envelope>,
    /// When Claude worked, and whether a shell command ran then.
    pub busy: Vec<(i64, i64, bool)>,
    pub start: i64,
    pub end: i64,
    /// A live session's open permission prompt.
    pub ask: Option<Envelope>,
}

/// The session's project directory.
pub fn cwd(s: &Session) -> String {
    format!("{ROOT}/{}", s.dir)
}

fn envelope(source: Source, ts: i64, payload: Value) -> Envelope {
    Envelope {
        v: 1,
        source,
        ts,
        pid: None,
        remote_id: None,
        applied_title: None,
        launch: None,
        project: None,
        branch: Some("main".into()),
        payload,
    }
}

fn hook(s: &Session, ts: i64, event: &str, extra: Value) -> Envelope {
    let mut payload = json!({
        "session_id": s.id,
        "cwd": cwd(s),
        "hook_event_name": event,
    });
    if let (Some(p), Value::Object(extra)) = (payload.as_object_mut(), extra) {
        p.extend(extra);
    }
    envelope(Source::Hook, ts, payload)
}

/// Its numbers so far; the rate limits are filled in by [`Limits`].
struct Meter {
    cost: f64,
    added: u64,
    removed: u64,
    context: f64,
}

fn statusline(s: &Session, ts: i64, m: &Meter) -> Envelope {
    envelope(
        Source::Statusline,
        ts,
        json!({
            "session_id": s.id,
            "cwd": cwd(s),
            "session_name": s.title,
            "model": {"id": s.model.0, "display_name": s.model.1},
            "cost": {
                "total_cost_usd": (m.cost * 100.0).round() / 100.0,
                "total_lines_added": m.added,
                "total_lines_removed": m.removed,
            },
            "context_window": {"used_percentage": m.context.round()},
        }),
    )
}

fn input(s: &Session, json: &str) -> Value {
    serde_json::from_str(&json.replace("{dir}", &cwd(s))).unwrap_or_else(|_| json!({}))
}

/// Builds `s` from `start`. A live session (`now` given) stops where it
/// stands: in its last step, at its permission prompt, or after its reply.
pub fn build(s: &Session, start: i64, now: Option<&Now>) -> Built {
    let mut out = Built {
        envelopes: Vec::new(),
        busy: Vec::new(),
        start,
        end: start,
        ask: None,
    };
    let mut m = Meter {
        cost: 0.0,
        added: 0,
        removed: 0,
        context: 7.0,
    };
    let mut t = start;
    out.envelopes.push(hook(
        s,
        t,
        "SessionStart",
        json!({"source": "startup", "model": s.model.0}),
    ));
    out.envelopes.push(statusline(s, t + 1_000, &m));
    let turns = s.turns.len();
    for (i, turn) in s.turns.iter().enumerate() {
        t += if i == 0 {
            6_000
        } else {
            turn.idle_min * 60_000
        };
        out.envelopes.push(hook(
            s,
            t,
            "UserPromptSubmit",
            json!({"prompt": turn.prompt, "permission_mode": "default"}),
        ));
        let began = t;
        t += think(s, i, 99);
        let last_turn = i + 1 == turns;
        for (j, step) in turn.steps.iter().enumerate() {
            let last = last_turn && j + 1 == turn.steps.len();
            let stop_here = last && matches!(now, Some(Now::Working | Now::Asking));
            match step {
                Step::Say(text) => {
                    out.envelopes.push(hook(
                        s,
                        t,
                        "MessageDisplay",
                        json!({"message_id": format!("{}-{i}-{j}", &s.id[..8]), "delta": text, "final": true}),
                    ));
                    t += 2_000;
                }
                Step::Tool(name, json, secs) => {
                    let tool_input = input(s, json);
                    out.envelopes.push(hook(
                        s,
                        t,
                        "PreToolUse",
                        json!({"tool_name": name, "tool_input": tool_input}),
                    ));
                    if stop_here {
                        out.end = t;
                        out.busy.push((began, t, false));
                        return out;
                    }
                    let ms = i64::from(*secs) * 1_000;
                    t += ms;
                    out.envelopes.push(hook(
                        s,
                        t,
                        "PostToolUse",
                        json!({"tool_name": name, "tool_input": tool_input, "duration_ms": ms}),
                    ));
                    if matches!(*name, "Edit" | "Write") {
                        m.added += 9;
                        m.removed += 3;
                    }
                }
                Step::Bash(command, output, secs)
                | Step::Asked {
                    command,
                    output,
                    secs,
                    ..
                } => {
                    let tool_input =
                        json!({"command": command, "description": format!("Run {command}")});
                    out.envelopes.push(hook(
                        s,
                        t,
                        "PreToolUse",
                        json!({"tool_name": "Bash", "tool_input": tool_input}),
                    ));
                    if let Step::Asked { wait, .. } = step {
                        let ask = hook(
                            s,
                            t + 300,
                            "PermissionRequest",
                            json!({"tool_name": "Bash", "tool_input": tool_input}),
                        );
                        if stop_here {
                            out.end = t + 300;
                            out.busy.push((began, t, false));
                            out.ask = Some(ask);
                            return out;
                        }
                        out.envelopes.push(ask);
                        t += 300 + i64::from(*wait) * 1_000;
                    }
                    let ms = i64::from(*secs) * 1_000;
                    out.busy.push((t, t + ms, true));
                    if stop_here {
                        out.end = t;
                        out.busy.push((began, t, false));
                        return out;
                    }
                    t += ms;
                    out.envelopes.push(hook(
                        s,
                        t,
                        "PostToolUse",
                        json!({
                            "tool_name": "Bash",
                            "tool_input": tool_input,
                            "duration_ms": ms,
                            "tool_response": {"stdout": output, "stderr": "", "interrupted": false},
                        }),
                    ));
                }
            }
            m.cost += 0.12;
            m.context += 2.4;
            out.envelopes.push(statusline(s, t + 400, &m));
            t += think(s, i, j);
        }
        out.envelopes.push(hook(
            s,
            t,
            "Stop",
            json!({"last_assistant_message": turn.reply}),
        ));
        out.busy.push((began, t, false));
        m.cost += 0.21;
        m.context += 1.5;
        out.envelopes.push(statusline(s, t + 500, &m));
        t += 1_000;
    }
    if now.is_none() {
        // NOTE: the owner leaves a few minutes later; the lane needs the end
        t += 4 * 60_000;
        out.envelopes.push(hook(
            s,
            t,
            "SessionEnd",
            json!({"reason": "prompt_input_exit"}),
        ));
    }
    out.end = t;
    out
}

/// Moves a built session by `by` ms.
pub fn shift(b: &mut Built, by: i64) {
    for e in b.envelopes.iter_mut().chain(b.ask.as_mut()) {
        e.ts += by;
    }
    for span in &mut b.busy {
        span.0 += by;
        span.1 += by;
    }
    b.start += by;
    b.end += by;
}

/// A cheap deterministic noise in [0, 1).
fn noise(seed: u64) -> f64 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x % 10_000) as f64 / 10_000.0
}

/// What the collector would have read of the session's processes at `ts`.
pub fn poll(s: &Session, b: &Built, ts: i64) -> ProcPoll {
    let seed =
        s.id.bytes()
            .fold(ts as u64, |a, c| a.wrapping_mul(31) + u64::from(c));
    let shell = b.busy.iter().any(|&(a, z, sh)| sh && a <= ts && ts <= z);
    let working = b.busy.iter().any(|&(a, z, _)| a <= ts && ts <= z);
    let n = noise(seed);
    let cpu_pct = if shell {
        70.0 + 60.0 * n
    } else if working {
        12.0 + 20.0 * n
    } else {
        0.3 + 0.8 * n
    };
    let base = 360.0 + 40.0 * ((ts - b.start) as f64 / 900_000.0).sin();
    let mem_mib = base + if working { 90.0 } else { 0.0 } + if shell { 140.0 } else { 0.0 };
    ProcPoll {
        session: s.id.to_owned(),
        cpu_pct,
        mem_bytes: (mem_mib * 1024.0 * 1024.0) as u64,
        procs: if shell {
            7
        } else if working {
            4
        } else {
            3
        },
        top: Vec::new(),
        forked_from: None,
    }
}

/// The account's five-hour and weekly usage as the sessions spend it,
/// filled into each statusline reading in time order.
pub struct Limits {
    anchor_5h: i64,
    anchor_week: i64,
    window: (i64, f64),
    week: (i64, f64),
    last: std::collections::HashMap<String, f64>,
}

impl Limits {
    /// The current five-hour window began 2 h 10 min before `now`, the
    /// week five days before.
    pub fn new(now: i64) -> Self {
        Self {
            anchor_5h: now - 130 * 60_000,
            anchor_week: now - 5 * 24 * 3_600_000,
            window: (i64::MIN, 0.0),
            week: (i64::MIN, 0.0),
            last: std::collections::HashMap::new(),
        }
    }

    fn start(anchor: i64, len: i64, ts: i64) -> i64 {
        anchor + (ts - anchor).div_euclid(len) * len
    }

    /// Counts the reading's spend and adds `rate_limits` to it.
    pub fn fill(&mut self, env: &mut Envelope) {
        let Some(p) = env.payload.as_object_mut() else {
            return;
        };
        let session = p["session_id"].as_str().unwrap_or_default().to_owned();
        let cost = p["cost"]["total_cost_usd"].as_f64().unwrap_or_default();
        let spent = cost - self.last.insert(session, cost).unwrap_or(0.0);
        let (five, week) = self.spend(env.ts, spent.max(0.0));
        p.insert("rate_limits".into(), Self::json(five, week));
    }

    /// Adds `spent` at `ts`; the two limits with their resets (unix seconds).
    pub fn spend(&mut self, ts: i64, spent: f64) -> ((f64, i64), (f64, i64)) {
        const FIVE_H: i64 = 5 * 3_600_000;
        const WEEK: i64 = 7 * 24 * 3_600_000;
        let w = Self::start(self.anchor_5h, FIVE_H, ts);
        if w != self.window.0 {
            self.window = (w, 0.0);
        }
        let k = Self::start(self.anchor_week, WEEK, ts);
        if k != self.week.0 {
            self.week = (k, 0.0);
        }
        self.window.1 += spent;
        self.week.1 += spent;
        let pct = |used: f64, of: f64| (used / of * 100.0).min(97.0).round();
        (
            (pct(self.window.1, FIVE_HOUR_USD), (w + FIVE_H) / 1_000),
            (pct(self.week.1, WEEK_USD), (k + WEEK) / 1_000),
        )
    }

    pub fn json(five: (f64, i64), week: (f64, i64)) -> Value {
        json!({
            "five_hour": {"used_percentage": five.0, "resets_at": five.1},
            "seven_day": {"used_percentage": week.0, "resets_at": week.1},
        })
    }
}
