//! The demo's week of history, written through the hub's own reducer
//! (`State`) into a fresh store, with the notes and the report.

use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{
    script::{CLOSED, HISTORY, NOTES, REPORT},
    timeline::{self, Built, Limits, POLL_MS},
};
use crate::{
    hub::reports::Reports,
    knowledge::{self, NoteKind, NoteOp, Status, Upsert},
    report::{self, Meta},
    state::{Effects, ProcPoll, State},
    store::Store,
    wire::{Envelope, Source},
    workspace::Workspaces,
};

const MINUTE: i64 = 60_000;

enum Item {
    Hook(Envelope),
    Polls(Vec<ProcPoll>),
}

/// Writes the history as of `now`; what the account had spent by then
/// goes on in `limits`.
pub async fn seed(
    store: &Store,
    reports: &Reports,
    workspaces: &Workspaces,
    now: i64,
    limits: &mut Limits,
) -> Result<()> {
    let mut timeline: Vec<(i64, &'static str, Item)> = Vec::new();
    let mut polls: BTreeMap<(i64, &'static str), Vec<ProcPoll>> = BTreeMap::new();
    for s in HISTORY {
        let built: Built = timeline::build(s, now - s.ago_min * MINUTE, None);
        let mut tick = built.start - built.start.rem_euclid(POLL_MS) + POLL_MS;
        while tick < built.end {
            polls
                .entry((tick, s.host))
                .or_default()
                .push(timeline::poll(s, &built, tick));
            tick += POLL_MS;
        }
        timeline.extend(
            built
                .envelopes
                .into_iter()
                .map(|e| (e.ts, s.host, Item::Hook(e))),
        );
    }
    timeline.extend(
        polls
            .into_iter()
            .map(|((ts, host), polls)| (ts, host, Item::Polls(polls))),
    );
    timeline.sort_by_key(|(ts, ..)| *ts);

    let mut states: HashMap<&str, State> = HashMap::new();
    for (ts, host, item) in timeline {
        let state = states.entry(host).or_insert_with(|| {
            State::new(host.to_owned(), Vec::new(), workspaces.clone())
                .with_tool_output_bytes(8_192)
        });
        let mut fx = Effects::default();
        match item {
            Item::Hook(mut env) => {
                if env.source == Source::Statusline {
                    limits.fill(&mut env);
                }
                state.apply(&env, &mut fx);
            }
            Item::Polls(polls) => state.resources(polls, ts, &mut fx),
        }
        for msg in fx.store {
            store.send(host, msg);
        }
    }

    let (ws, slug, title, summary, ago, body) = REPORT;
    let published = now - ago * MINUTE;
    let date = knowledge::ymd(published);
    let html = report::build(&Meta { title, date: &date }, body);
    reports
        .stage(ws, slug, html.as_bytes())
        .context("the demo report")?
        .commit()?;
    let bytes = html.len();
    store
        .exec(move |conn| -> Result<()> {
            let mut slugs = HashMap::new();
            for note in NOTES {
                let op: NoteOp = serde_json::from_str(note.op)
                    .with_context(|| format!("demo note {}", note.op))?;
                let written = knowledge::apply(
                    conn,
                    note.workspace,
                    note.project,
                    note.by,
                    &op,
                    now - note.ago_min * MINUTE,
                )?;
                slugs.insert(written.title.clone(), (note.workspace, written.slug));
            }
            for (title, ago, by) in CLOSED {
                let (ws, slug) = slugs.get(*title).context("a closed issue the demo lacks")?;
                let op: NoteOp = serde_json::from_value(json!({
                    "op": "issue_update",
                    "slug": slug,
                    "status": "closed",
                    "comment": "Fixed.",
                }))?;
                knowledge::apply(conn, ws, None, Some(by), &op, now - ago * MINUTE)?;
            }
            let up = Upsert {
                slug: slug.to_owned(),
                kind: NoteKind::Report,
                project: None,
                title: title.to_owned(),
                summary: Some(summary.to_owned()),
                status: Status::Active,
                extra: json!({
                    "bytes": bytes,
                    "sha256": Value::Null,
                    "host": "desk",
                    "path": Value::Null,
                    "published_at": published,
                    "date": date,
                }),
                body: summary.to_owned(),
                links: Vec::new(),
                created_at: Some(published),
                updated_at: Some(published),
                verified: true,
            };
            knowledge::upsert(conn, ws, &up, None, published)?;
            Ok(())
        })
        .await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        demo::script::{ROOT, WORKSPACES},
        util::now_ms,
    };

    #[tokio::test]
    async fn the_week_goes_through_the_reducer_into_every_table() {
        let dir = std::env::temp_dir().join(format!("ccdash-demo-seed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(&dir.join("ccdash.db"), 30).unwrap();
        let reports = Reports::open(&dir.join("reports"), 32).unwrap();
        let dirs: Vec<String> = WORKSPACES.iter().map(|w| format!("{ROOT}/{w}")).collect();
        let now = now_ms();
        let mut limits = Limits::new(now);
        seed(
            &store,
            &reports,
            &Workspaces::new(&dirs, ""),
            now,
            &mut limits,
        )
        .await
        .unwrap();
        let counts = store
            .exec(|conn| {
                let n = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
                [
                    n("SELECT count(*) FROM sessions"),
                    n("SELECT count(*) FROM sessions WHERE ended_at IS NOT NULL"),
                    n("SELECT count(*) FROM notes"),
                    n("SELECT count(*) FROM events WHERE kind = 'permission'"),
                    n("SELECT count(*) FROM samples WHERE five_hour IS NOT NULL"),
                    n("SELECT count(*) FROM proc_samples"),
                ]
            })
            .await
            .unwrap();
        let [sessions, ended, notes, asks, samples, procs] = counts;
        assert_eq!(sessions, HISTORY.len() as i64);
        assert_eq!(ended, sessions, "every past session ends");
        assert_eq!(notes, NOTES.len() as i64 + 1, "the notes and the report");
        assert!(asks > 0 && samples > 0 && procs > 0, "{counts:?}");
        assert!(
            dir.join("reports/acme/dependency-audit--2026-11.html")
                .is_file()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
