//! Prompts being written in the panel, kept on the hub: a draft survives a
//! reload, a closed tab and a change of device, and it is what an editor
//! on a workstation edits (`Edit in nvim`). One draft per key: the
//! composer of a session (`session:<id>`), the start sheet (`start`,
//! `issue:<slug>`, `resume:<id>`).

use anyhow::{Result, bail};
use rusqlite::{Connection, params};

use crate::{actions::MAX_PROMPT, model::Draft};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS drafts (
    key TEXT PRIMARY KEY,
    text TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
";

/// Characters of a draft: what a prompt may be.
pub const MAX_TEXT: usize = MAX_PROMPT;
/// A draft nobody touched for this long is dropped.
const KEEP_MS: i64 = 90 * 24 * 3_600_000;
const MAX_KEY: usize = 128;

/// `start`, or `<kind>:<id>` with a kind the panel has.
pub fn valid_key(key: &str) -> bool {
    if key == "start" {
        return true;
    }
    if key.len() > MAX_KEY {
        return false;
    }
    let Some((kind, id)) = key.split_once(':') else {
        return false;
    };
    matches!(kind, "session" | "issue" | "resume")
        && !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Refuses a key or a text the store does not take.
pub fn check(key: &str, text: &str) -> Result<(), String> {
    if !valid_key(key) {
        return Err(format!("{key:?} is not a draft key"));
    }
    if text.chars().count() > MAX_TEXT {
        return Err(format!("drafts are at most {MAX_TEXT} characters"));
    }
    Ok(())
}

/// Writes the draft and drops the ones nobody touched for `KEEP_MS`.
pub fn put(conn: &Connection, key: &str, text: &str, now: i64) -> Result<()> {
    if let Err(e) = check(key, text) {
        bail!("{e}");
    }
    conn.execute(
        "INSERT INTO drafts (key, text, updated_at) VALUES (?1, ?2, ?3)
            ON CONFLICT (key) DO UPDATE SET text = excluded.text, updated_at = excluded.updated_at",
        params![key, text, now],
    )?;
    conn.execute(
        "DELETE FROM drafts WHERE updated_at < ?1",
        [now.saturating_sub(KEEP_MS)],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM drafts WHERE key = ?1", [key])?;
    Ok(())
}

/// Every kept draft, oldest first.
pub fn all(conn: &Connection) -> Result<Vec<Draft>> {
    let mut stmt = conn.prepare("SELECT key, text, updated_at FROM drafts ORDER BY updated_at")?;
    let rows = stmt.query_map([], |r| {
        Ok(Draft {
            key: r.get(0)?,
            text: r.get(1)?,
            updated_at: r.get(2)?,
            editing: None,
            notice: None,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn
    }

    #[test]
    fn keys() {
        assert!(valid_key("start"));
        assert!(valid_key("session:5e3f1c2a-0000-4000-8000-000000000001"));
        assert!(valid_key("issue:ccdash--panel-closes-live-sessions"));
        assert!(valid_key("resume:abc.def_1"));
        assert!(!valid_key(""));
        assert!(!valid_key("session:"));
        assert!(!valid_key("draft:x"));
        assert!(!valid_key("session:a/b"));
        assert!(!valid_key("session:a b"));
        assert!(!valid_key(&format!("session:{}", "a".repeat(200))));
    }

    #[test]
    fn put_read_remove_and_expire() {
        let conn = conn();
        put(&conn, "start", "hello", 1_000).unwrap();
        put(&conn, "session:s1", "first", 2_000).unwrap();
        put(&conn, "session:s1", "second", 3_000).unwrap();
        let kept = all(&conn).unwrap();
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].key, "start");
        assert_eq!(kept[1].text, "second");
        assert_eq!(kept[1].updated_at, 3_000);
        assert!(put(&conn, "bogus", "x", 1).is_err());
        assert!(put(&conn, "start", &"x".repeat(MAX_TEXT + 1), 1).is_err());
        remove(&conn, "start").unwrap();
        assert_eq!(all(&conn).unwrap().len(), 1);
        // a write much later drops what nobody touched
        put(&conn, "resume:r", "later", 3_000 + KEEP_MS + 1).unwrap();
        let keys: Vec<String> = all(&conn).unwrap().into_iter().map(|d| d.key).collect();
        assert_eq!(keys, ["resume:r"]);
    }
}
