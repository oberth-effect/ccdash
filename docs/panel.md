# The panel

The web panel (`web/`, SvelteKit) is served by the hub at `/` and is built
phone first. Every view can be narrowed to workspaces and hosts: the scope
is kept per device (browser storage), across pages and reloads, until you
change it. The time range and the analytics section live in the URL, so
links can be shared and the back button works.

1. [Now and Sessions](#1-now-and-sessions)
2. [A session's page](#2-a-sessions-page)
3. [Projects](#3-projects)
4. [History and Analytics](#4-history-and-analytics)
5. [Machines, Claude state and Workspace](#5-machines-claude-state-and-workspace)
6. [Desktop (DankMaterialShell)](#6-desktop-dankmaterialshell)

## 1. Now and Sessions

- **Now** lists what needs you, with the answer inline (permission prompts,
  questions, plans, sessions on your turn with a quick reply) and
  **Resolved** and **Kill** at hand, then what is working, the hosts with
  their link state, the account limits and the last progress entries.
  **Start a session** is here ([sessions](sessions.md)).
- **Sessions** groups every session: those that need you, then working,
  parked and idle, then the ones Claude started (which show the session that
  started them and "Finished" instead of "Your turn").
- A session left on your turn for an hour is **parked**: most likely done,
  it no longer counts as needing you.
- Every session card has an **Actions** menu: Open on, Resolved, Keep open,
  Continue, Kill, Forget, whichever apply ([sessions](sessions.md)).

## 2. A session's page

| Tab     | Shows                                                                                                                    |
| ------- | ------------------------------------------------------------------------------------------------------------------------ |
| Chat    | the exchanges with Claude's text, its steps, subagents, task notifications, subagent messages, messages between sessions |
| Shell   | its Bash calls as a terminal transcript with their output                                                                |
| Details | the project (pick one), the turn, files, tasks, processes, the claim, totals                                             |

**Steps** are the tool calls between Claude's messages, written to be read
at a glance: a run of them has one line ("Ran 9 commands, edited 2 files ·
1 failed · 2m") and its time range; each step is a sentence with an icon
for what kind of thing it is: for a shell command, what Claude said it does
("Run the tests through the dev env", the command dim below it; older
calls, recorded before the description was kept, show the command), for
files "Edited `src/state.rs`", for searches the pattern, for a plugin tool
its server and action with the argument that tells most ("playwright:
click `Units tab`"). Consecutive reads, edits or searches of one kind fold
into one row ("Read 4 files", their names below). A failed step is red
with the error's first line. Tap a step for the command, its output and
when it ran; the Shell tab has every command in full. The run of the
exchange Claude is working on stays open, older ones are folded.

The block under the newest exchange shows what is happening now: the
state, the running tool (what it does, the call below), the text Claude
is writing, the turn's tools and files, subagents and tasks, and a pending
**Stop** ([sessions](sessions.md#4-interrupt)).

**Task notifications** are what Claude Code tells a session when a
background task reports: a subagent finished, a background command exited,
a monitor saw an event, an MCP task completed. **Subagent messages** are a
subagent's report handed back, or a message it sends while it runs. Both
reach the hooks as if you had typed them; the hub records them apart, so
the chat shows them on Claude's side in their own colour (what it is, the
subagent's or task's name, the outcome, the report or event) and they
never count as your prompts: not in the prompt history, the session's last
prompt, prompt counts or waits. A notification Claude Code appends to
Claude's text, when one arrives while Claude writes, is cut from it; Claude
Code delivers it again on its own.

Open prompts are pinned above a composer that sends a turn through the
channel or leaves a note for the next prompt; while Claude works, **Stop**
interrupts the turn, so what you type next is what Claude reads
([sessions](sessions.md#4-interrupt)). The composer walks the prompt
history, inserts `[[note]]` links and keeps drafts. The title renames in
place; **Open on** and **Resolved** sit beside it, the other actions in its
**Actions** menu, **Continue** at the foot once it ended
([sessions](sessions.md)).

## 3. Projects

Each workspace's projects with their board (open issues), live sessions,
progress, documents, memories and the note graph, and a reader and editor
for every note ([knowledge](knowledge.md)).

**Progress** is a timeline, newest day first (with its entries, issues and
sessions counted): each entry with its time, title and first lines, the
issue it is on and whether that is open or closed now, the session that
logged it (a link, with its host) and the project. **All of it** unfolds
the whole entry in place.

## 4. History and Analytics

**History** draws the sessions of a time range as lanes coloured by state,
with the totals, time per project, the most used tools and the 5-hour
limit over time.

**Analytics** sections:

| Section     | Answers                                                                                                            |
| ----------- | ------------------------------------------------------------------------------------------------------------------ |
| Overview    | daily activity, when you prompt (weekday × hour), work per prompt, cost, lines changed, CPU and memory per project |
| Waits       | how long sessions waited on you and how fast you replied, apart from parked time                                   |
| Parallel    | how many sessions worked or waited at once; time everything waited with nothing working                            |
| Limits      | 5-hour, 7-day and per-model weekly limits, their pace, each workspace's share                                      |
| Projects    | time, prompts, cost, lines, tool calls, compactions and CPU per directory                                          |
| Turns       | what fills a turn, the slowest tools and commands, the commands that fail most                                     |
| Context     | compactions and how full sessions got                                                                              |
| Tokens      | tokens by type, cache hits, API latency, commits, edits by language (needs telemetry)                              |
| Permissions | what asks you most, grouped by the rule that would cover it, how you answered                                      |

A **wait** runs from Claude handing the turn back to what came next: a
reply within an hour counts as waiting on you; a later reply, closing the
session or leaving it open counts as parked; Claude carrying on by itself
(a task notification, a wakeup) counts as neither. Permission prompts
always count.

The time range in the top bar: quick ranges or an exact span, `‹ ›` to
step, `⊖` to double it, and dragging across a time chart zooms in. Every
chart has a tooltip and a table view.

The Tokens section and the approval sources under Permissions need Claude
Code's OpenTelemetry export: `services.ccdash.claudeCode.telemetry.enable`
sets the `OTEL_*` variables and starts a receiver in the collector on
`127.0.0.1:7839`. It keeps counts and timings only; prompt text stays
redacted, as Claude Code sends it by default.

## 5. Machines, Claude state and Workspace

**Machines** lists every host with a collector (its link and round trip,
the sessions on it, its copy of Claude's state, its share of the workspace
sync); a machine's page gathers what the other areas know about it: the
sessions on it now, Claude's usage there over the range in the bar
(sessions, working time, cost, lines, CPU, activity by state), its copy of
Claude's state, and its workspace sync (units in step, what moved and what
it cost in the range, the units not in step there, its large transfers).
Host names on the Now page and in the Workspace area open it.

**Claude state** is each machine's copy of `~/.claude` (the [state
sync](state-sync.md)): machines with Resync, conflicts with their
resolutions, sizes and growth.

**Workspace** is the workspace root on every machine (the [workspace
sync](workspace.md#7-the-workspace-area)), in sections: **Overview** (the
tiles, what needs you with the buttons at hand, the machines in a line
each, the last day's traffic, the units that changed last), **Units** (a
sortable table with a search, state filters and a chip per machine, new
directories, held snapshots, conflicts; a unit's page with
its state on every machine, diverged branches, conflicts, the ignored
directories with a switch each, overrides and the snapshot history),
**Machines** (a card per collector: the link's round trip, what it moved
today and this week, the rate of its large transfers, what the sync cost
in CPU, I/O and memory), **Statistics** (the range in the bar, a year back: traffic,
pushes, CPU and memory per machine, the hub's repositories, conflicts) and
**Configuration** (the rules with live match counts and what each never-
travelling name keeps home, previewed by the hub as you type; then the
coverage: what travels and what stays home on every machine, and why).

## 6. Desktop (DankMaterialShell)

`services.ccdash.dms.enable = true` links the [plugin](../dms-plugin/) into
DankMaterialShell: a bar pill with the sessions that need you, a dash tab
and card, notifications when a session starts waiting, and
`dms ipc call ccdash focusWaiting` to jump to the session that has waited
longest. Enable it once under Settings → Plugins and add the widget to a
bar. niri binds:

```kdl
Mod+C { spawn "dms" "ipc" "call" "dash" "toggle" "plugin_ccdash"; }
Mod+Shift+C { spawn "dms" "ipc" "call" "ccdash" "focusWaiting"; }
```

Clicking a session focuses its terminal window. The plugin opens the panel
without a token, so pair the desktop browser once ([security](security.md)).
