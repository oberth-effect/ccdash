//! Every past session on the hub, searchable: a catalog derived from the
//! transcripts the state sync keeps (`sync.dir/projects/<dir>/<id>.jsonl`,
//! every machine's), in its own database next to the store. It holds one
//! row per transcript (titles, the first and last prompt, Claude's last
//! text, dates, size, numbers, the machines that wrote it) and every
//! prompt and every block of Claude's text for full-text search.
//!
//! Derived data only: deleting `catalog.db`, or a new [`PARSER`], rebuilds
//! it from the transcripts. The indexer follows the sync index
//! (`sync_files`): a transcript that grew is read from where it stopped, one
//! that was replaced (a new `base`) or shrank is read again.

mod index;
mod parse;
mod query;

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

pub use query::{Conversation, Page, PastDetail, Query, Sort, conversation, detail, find, list};

/// Bump when what is extracted changes: the catalog is then rebuilt.
pub const PARSER: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS catalog_meta (
    key TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS catalog (
    n INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    id TEXT NOT NULL,
    base INTEGER NOT NULL,
    size INTEGER NOT NULL,
    offset INTEGER NOT NULL,
    cwd TEXT,
    first_at INTEGER,
    last_at INTEGER,
    custom_title TEXT,
    ai_title TEXT,
    summary TEXT,
    recap TEXT,
    first_prompt TEXT,
    first_prompt_at INTEGER,
    last_prompt TEXT,
    last_prompt_at INTEGER,
    last_reply TEXT,
    last_reply_at INTEGER,
    prompts INTEGER NOT NULL DEFAULT 0,
    replies INTEGER NOT NULL DEFAULT 0,
    tools INTEGER NOT NULL DEFAULT 0,
    active_ms INTEGER NOT NULL DEFAULT 0,
    compactions INTEGER NOT NULL DEFAULT 0,
    models TEXT NOT NULL DEFAULT '{}',
    files TEXT NOT NULL DEFAULT '{}',
    branch TEXT,
    version TEXT,
    entrypoint TEXT,
    cost_usd REAL,
    lines_added INTEGER,
    lines_removed INTEGER,
    continued_in TEXT,
    host TEXT
);
CREATE INDEX IF NOT EXISTS catalog_id ON catalog (id);
CREATE TABLE IF NOT EXISTS catalog_hosts (
    session INTEGER NOT NULL,
    host TEXT NOT NULL,
    PRIMARY KEY (session, host)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS catalog_texts (
    id INTEGER PRIMARY KEY,
    session INTEGER NOT NULL,
    at INTEGER NOT NULL,
    role TEXT NOT NULL,
    text TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS catalog_texts_session ON catalog_texts (session, id);
CREATE VIRTUAL TABLE IF NOT EXISTS catalog_fts USING fts5(
    text,
    content = 'catalog_texts',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER IF NOT EXISTS catalog_texts_ai AFTER INSERT ON catalog_texts BEGIN
    INSERT INTO catalog_fts (rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER IF NOT EXISTS catalog_texts_ad AFTER DELETE ON catalog_texts BEGIN
    INSERT INTO catalog_fts (catalog_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE VIRTUAL TABLE IF NOT EXISTS catalog_titles USING fts5(
    title,
    tokenize = 'unicode61 remove_diacritics 2'
);
";

const DROP: &str = "
DROP TRIGGER IF EXISTS catalog_texts_ai;
DROP TRIGGER IF EXISTS catalog_texts_ad;
DROP TABLE IF EXISTS catalog_fts;
DROP TABLE IF EXISTS catalog_titles;
DROP TABLE IF EXISTS catalog_texts;
DROP TABLE IF EXISTS catalog_hosts;
DROP TABLE IF EXISTS catalog;
";

/// How far the indexer is: transcripts it still has to read.
#[derive(Debug, Default)]
pub struct Progress {
    pending: AtomicU64,
    passes: AtomicU64,
}

impl Progress {
    /// Transcripts left in the running pass.
    pub fn pending(&self) -> u64 {
        self.pending.load(Ordering::Relaxed)
    }

    /// Whether a pass has finished since the hub started.
    pub fn ready(&self) -> bool {
        self.passes.load(Ordering::Relaxed) > 0
    }
}

/// The catalog's database and the store it reads labels from.
#[derive(Debug, Clone)]
pub struct Catalog {
    path: PathBuf,
    store: PathBuf,
    pub progress: Arc<Progress>,
}

impl Catalog {
    /// Opens (creating, or rebuilding for a new [`PARSER`]) the catalog at
    /// `path`; `store` is the hub's database, read for the sync index and
    /// the sessions' labels.
    ///
    /// # Errors
    ///
    /// The database cannot be created or migrated.
    pub fn open(path: &Path, store: &Path) -> Result<Self> {
        let conn = writer(path)?;
        let parser: Option<i64> = conn
            .query_row(
                "SELECT value FROM catalog_meta WHERE key = 'parser'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if parser != Some(PARSER) {
            conn.execute_batch(DROP)?;
            conn.execute_batch(SCHEMA)?;
            conn.execute(
                "INSERT INTO catalog_meta (key, value) VALUES ('parser', ?1)
                ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [PARSER],
            )?;
        }
        Ok(Self {
            path: path.to_owned(),
            store: store.to_owned(),
            progress: Arc::default(),
        })
    }

    /// A read-only connection with the store attached as `store` (its
    /// `sessions` labels).
    ///
    /// # Errors
    ///
    /// Either database cannot be opened.
    pub fn reader(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("opening {}", self.path.display()))?;
        conn.busy_timeout(Duration::from_secs(2))?;
        conn.execute(
            "ATTACH DATABASE ?1 AS store",
            [self.store.to_string_lossy()],
        )?;
        Ok(conn)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn store_path(&self) -> &Path {
        &self.store
    }
}

/// The catalog's writer: private to the hub's user (it holds prompts), WAL,
/// normal syncs (it is rebuilt from the transcripts, so a lost last write
/// costs nothing).
fn writer(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::store::make_private(path)?;
    let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
    conn.pragma_update(None, "page_size", 8192)?;
    conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub use index::spawn;

#[cfg(test)]
pub(crate) mod tests;
