---
name: report
description: Make an HTML report (a research write-up, a comparison, an audit, a dashboard, any page-shaped deliverable) in ccdash's shared design and publish it to the hub, where the panel shows, lists and downloads it. Use whenever the deliverable is a report, dashboard, visualization or "an HTML page", or the owner asks for one; never a hand-rolled standalone HTML file or an Artifact instead.
---

# HTML reports

A report is one self-contained HTML file in ccdash's chrome (top bar with
the title, the date and a search box; a System/Light/Dark switch and a
Small/Normal/Big text size; the contents on the left; numbered headings),
published to the hub. The hub's
copy is the deliverable: the panel's **Reports** area lists it, frames it
with its abstract, searches it and downloads it to send on; `recall` finds
it like any note. The local file is scaffolding.

## 1. Write the body

Write only the article's content, an HTML fragment, to a file (the
project's `.claude-tmp/reports/<slug>/body.html`, or the scratchpad):

- Start with `<p class="lead">` (two sentences: the question and the
  answer). Then `<h2>` sections, `<h3>` subsections; the chrome numbers
  them and lists them in the contents, so make every heading say what the
  section concludes, not what it is about.
- Key numbers in `.tiles`, comparisons in `.table-wrap > table` with a
  `<thead>` (`.num` on number cells, `tr.total` for a last row that stays
  last): every such table sorts by a click on its headings, and one with
  more than eight rows gets a filter box, with no work on your side. The
  one thing to know in a `.callout.{info,good,warn,bad}`, status words as
  `.badge`, facts as `dl.kv`, long evidence in `<details>`. Code in
  `<pre><code class="language-rust">` (escape `<` and `&`): the chrome
  adds the colours, line numbers and Copy. Every element: `reference.md`.
- Charts: inline SVG (the `dataviz` skill for form and colour) in a
  `<figure>` with a `<figcaption>`. Colours only through the theme's
  variables (`var(--series-1)`…`--series-6`, `--seq-1`…`--seq-6`,
  `--good`, `--warn`, `--bad`, `--muted`, `--border`), never hex, so the
  chart flips with the theme.
- End with the sources (`<h2>Sources</h2>`, a list, class `sources`) and
  the method or caveats.

Hard rules:

- **Self-contained.** No `<script src>`, `<link href>`, `<img src="http…">`,
  `url(http…)`, `@import`, `<iframe>`. An image goes in as a `data:` URI.
  `ccdash report build` refuses anything else, and so does the hub: the
  panel frames reports under a policy that loads nothing from the network.
- **No secrets**, no token values, no customer data the owner did not ask
  to see in the report.
- **Light.** Tables of hundreds of rows, raw data and screenshots belong
  elsewhere; a report is read, not browsed. Under a few MB.

## 2. Build

```sh
ccdash report build body.html --title "Hair dryer research (Alza.cz)" -o report.html
```

The date defaults to today (`--date YYYY-MM-DD`). The command wraps the
fragment in the chrome, gives headings ids and refuses a body that fetches
anything; it prints the size and the section count. Look at the result
once (the Playwright MCP on `file://…/report.html`, a screenshot): charts
and tables are where mistakes hide.

## 3. Publish

Call `report_publish`:

| Argument  | What                                                                                                                                                    |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `path`    | `report.html`, absolute or relative to the session's directory                                                                                          |
| `title`   | the title (default: the file's `<title>`)                                                                                                               |
| `slug`    | `<topic>--<YYYY-MM-DD>` (`hair-dryer--2026-11-02`); publishing to an existing slug replaces that report                                                 |
| `summary` | one line for lists and session starts: what it is about, and when                                                                                       |
| `body`    | the abstract, markdown: the question, the findings with their numbers, the recommendation, the caveats; link notes as `[[slug]]`. What `recall` returns |
| `project` | the repo directory name, when the session's is not it                                                                                                   |
| `date`    | `YYYY-MM-DD` when the report was made, for an older file (default: today)                                                                               |

The reply carries the panel's address: give it to the owner. The session's
chat in ccdash shows a "Published a report" bubble linking to it. When the
hub is unreachable the tool says so and the file stays where it is:
publish again later (also `ccdash report publish report.html --title …
--body-file abstract.md` from a shell).

Older reports lying around on a machine: `ccdash report import --dry-run
PATHS` (files or directories) lists what would be published (the title
from the document's `title` element, the date from the file's modification
time, files that fetch from the network skipped with the reason); without
`--dry-run` it publishes them.

## 4. Tell the owner

The panel address, the local path, and the abstract's key line. Link the
report from the progress entry or the issue it closes (`[[slug]]`). The
abstract is the note: do not write the same content into a document too.
