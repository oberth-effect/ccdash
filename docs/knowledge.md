# Knowledge: memories, issues, progress, documents

The hub holds what Claude should know across sessions and machines, as
notes with `[[slug]]` links, backlinks and full-text search, each in one
workspace.

1. [Notes](#1-notes)
2. [Tools](#2-tools)
3. [Injections](#3-injections)
4. [Offline](#4-offline)
5. [Lifecycle](#5-lifecycle)
6. [Import and export](#6-import-and-export)

## 1. Notes

| Kind     | For                                                                      |
| -------- | ------------------------------------------------------------------------ |
| memory   | facts worth keeping                                                      |
| issue    | open work, with a status                                                 |
| progress | dated entries                                                            |
| doc      | plans, specs, anything long                                              |
| report   | an HTML report's abstract; the file is the hub's ([reports](reports.md)) |

The **Projects** page shows every project's notes, searches and edits them.
Tick notes in its search results, across searches, and **Export** gives a
`.zip` for someone without ccdash: markdown with frontmatter under
`<kind>/<slug>.md` and an `INDEX.md`, so their Claude can read it like a memory
directory. Links inside the selection stay `[[slug]]`; a link out of it
becomes that note's title.

## 2. Tools

Sessions reach the notes through tools the channel server exposes:
`recall`, `remember`, `verify`, `issue_add`, `issue_update`, `progress`,
`doc_write`, `report_publish` ([reports](reports.md)), and between
sessions `claim`, `release` and `tell_session`.

- Every call is scoped to the calling session's workspace; a session never
  sees another workspace's notes.
- A note belongs to a **project** or to the whole workspace. The project
  is a label from a closed set: a directory directly under the workspace
  on some host, or one that already has notes; anything else is refused.
  A call names it with `project`, else the note takes the session's. An
  empty `project` on `remember` or `issue_update` makes a note workspace-wide.
- A session's project is the launch's (the panel), or `claude --project
<name>` in a terminal (which sets `CCDASH_PROJECT`; the `channel` wrapper
  takes the flag), else the one it `claim`s with `project`, `/ccdash:project
<name>` asks for, or the owner picks on its page (Details), else
  the directory it runs in, else none. Claude Code runs at the workspace
  root as a rule, so a session there is told at its start to claim its
  project; claim `paths` resolve under the project's directory. Only the
  launch and `claude --project` place a session before its first context,
  so only they scope the notes it starts with. Claude reads the project as
  that context's heading (`## ccdash knowledge: workspace / project`); the
  prompt and the working directory stay as they are. A name the workspace
  has no directory for leaves the session where it is, and Claude is told.
- A call is one round trip session → collector → hub with a 5 s budget.
- `doc_write` carries the version the edit was based on, and the hub
  refuses a stale one, so a shared document never loses an edit silently.

## 3. Injections

The hooks add what matters to the session's context:

| When                  | What                                                                | Switch                 |
| --------------------- | ------------------------------------------------------------------- | ---------------------- |
| session start         | open issues, relevant memories, documents, reports, recent progress | `inject.session_start` |
| every prompt          | other sessions active in the project, messages for it               | `inject.prompt`        |
| before editing a file | who else edited it lately, claims covering it                       | `inject.edits`         |

## 4. Offline

While the hub is unreachable, writes wait in the collector's outbox (the
tool says so) and are replayed in order when the link returns. Reads come
from a cache of the live notes the hub pushes to every collector
(`~/.local/state/ccdash/notes-cache.json`). A session that started during
an outage catches up at its first prompt after the link returns.

## 5. Lifecycle

A daily pass marks memories stale when they were not verified for 90 days
or not recalled for 60 (`knowledge.stale_unverified_days`,
`knowledge.stale_unrecalled_days`; `knowledge.reflect = false` turns it
off). Stale notes stay searchable but are no longer injected; the Projects
page lists them to verify, rewrite or archive. A note that `supersedes`
another retires it.

## 6. Import and export

```sh
ccdash import memory ~/code/work/.claude/memory              # one note per file, subdirectories too
ccdash import memory ~/code/work/.claude/memory/ops          # one subdirectory (its own MEMORY.md or INDEX.md)
ccdash import plan ~/code/work/meta/plans/x.md --project x   # doc, checkboxes as issues, log as progress
ccdash import plan ~/code/work/notes/design.md --doc --slug design   # the document alone
ccdash export --config /etc/ccdash/hub.toml ./notes          # every note as markdown
```

`import` runs on a workstation and sends through its collector; `export`
reads the database (or a backup copy with `--db`) directly. Every import is
safe to rerun: an unchanged file changes nothing, and a note the hub changed
after the file's modification time is kept (the report counts it as
`kept`). A memory's title is its index line's, its summary the frontmatter
`description` or the index hook when that says more; `status: archived` in
the frontmatter retires the note, so an export imports back as it was. A
plan's checkbox is keyed by its section and text: tick it without rewording
it.
