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
6. [Themes](#6-themes)
7. [Desktop (DankMaterialShell)](#7-desktop-dankmaterialshell)

## 1. Now and Sessions

- **Now** is the page to leave open on a second monitor; a look at it should
  replace opening the sessions. What needs you has the answer inline
  (permission prompts, questions, plans, sessions on your turn with a quick
  reply) and **Resolved** and **Kill** at hand. Every session that needs you
  or is working shows what you asked, what Claude last said, its last steps
  with the running one under them (failures in red with their error), tasks,
  subagents, the files of the turn and its numbers. A working session that
  has sent no hook for 10 minutes (30 while a tool runs) says "quiet" with how
  long: most likely a hung command or a dead process. The other open sessions
  follow in a line each. On wide screens a column beside them holds the
  [limits](#limits-on-now), the system (machines and their link, open issues,
  the workspace sync, Claude's state) and the last progress across projects
  by day, with the issue and the session of each entry; a phone stacks them.
  **Start a session** is here ([sessions](sessions.md)).
- **Sessions** groups every session: those that need you, then working,
  parked and idle, then the ones Claude started (which show the session that
  started them and "Finished" instead of "Your turn").
- **Sessions › Past** is every earlier conversation the hub holds, from
  every machine: a search over what was said, filters for the machines
  (any number), project and time, cards with what you asked and where it
  ended, an overview and the conversation beside the list, and **Resume…**
  on the machine you pick ([sessions](sessions.md#2-resume)). **Resume…** on
  Now opens it.
- A session left on your turn for an hour is **parked**: most likely done,
  it no longer counts as needing you.
- Every session card has an **Actions** menu: Open on, Resolved, Keep open,
  Resume, Kill, Forget, whichever apply ([sessions](sessions.md)).

### Limits on Now

Each limit's tile draws its current window behind the numbers: the readings
so far, a dashed line to where the recent pace leads at the reset, and a faint
diagonal for an even pace, so a line above it is running hot. Hovering reads
the value off the line and a click opens Analytics › Limits. A window that
reset with no reading since says so, rather than keeping the old number.

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

**Code blocks** in replies, notes and the steps of a command have syntax
colours in the theme's palette, line numbers (kept out of a selection) and a
copy button. A language the panel does not know, or a chunk it could not
fetch, leaves the block plain.

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
([sessions](sessions.md#4-interrupt)). The composer is an editor made for
long prompts: Enter is always a new line and only `Ctrl Enter` sends, on
every device; it wraps, highlights markdown lightly, keeps an undo history
that survives what the toolbar inserts, and **Expand** (`Ctrl Shift F`)
gives it most of the screen. With a mouse it has vim keys (Settings ›
Composer turns them on or off, and holds a vimrc of maps it starts with:
`jk` leaves insert mode, `Space w` is `:w`); `:w` keeps the draft now,
`:x` is done editing. Line numbers (none, absolute, or relative to the
cursor's line with the current one absolute, as Neovim's `number` +
`relativenumber`; Auto is relative with vim keys), the markdown
highlighting and the wrapping are switches there too, kept per device.
Two more go with vim keys, each a switch: the **system clipboard** is
the unnamed register (yanks and deletes go to it; the browser's paste
keys, `Ctrl Shift V` and `Shift Insert`, paste as `p` does with no
prompt anywhere, and a bare `p` reads the clipboard, which is a
permission: Chromium asks once, Firefox puts a small Paste button under
the cursor whenever the text came from another program; Settings can
make `p` read the register instead), and **hop**: `,` in normal or
visual mode, then two characters, labels every match in view with the
keys you set (`asdfghjkl`, nearest match first, two-character labels
when one does not suffice) while the rest of the text dims and a line
under the text echoes what was typed; the label puts the cursor there,
or in visual mode extends the selection through it; Escape or any other
key cancels.
The composer walks the prompt history, inserts `[[note]]` links,
and when a message starts with `/` offers the skills Claude Code loads for
that session (the machine's, the plugins', the workspace's), as the
terminal does. Its keyboard button lists the shortcuts:

| Keys               | Does                                          |
| ------------------ | --------------------------------------------- |
| `Ctrl Enter`       | Send (`Enter` is a new line)                  |
| `Ctrl Shift Enter` | Next prompt instead of a turn                 |
| `Ctrl Shift 1–9`   | That quick reply                              |
| `Ctrl .`           | Stop the turn                                 |
| `Ctrl G`           | Edit in nvim on a machine (below)             |
| `Ctrl Shift F`     | Expand the field, and back                    |
| `Ctrl J`           | Go to the open question                       |
| `Ctrl ↑` `Ctrl ↓`  | Walk the prompt history                       |
| `Ctrl R`           | Search the prompt history                     |
| `Ctrl E`           | Code fence around the selection               |
| `Ctrl P`           | Preview                                       |
| `Esc`, then `i`    | Leave the field (vim: normal mode), come back |

**Drafts live on the hub.** What you type is kept there after a short
pause (and on the device meanwhile), so a prompt begun on the phone is on
the desktop, a reload or a closed tab loses nothing, and the start sheet
opens with what you left in it. A draft is dropped when its prompt is
sent, or after 90 days untouched.

**Edit in nvim** (`Ctrl G`, the terminal button) opens the draft in an
editor on a machine, as Claude Code's own `Ctrl G` opens the prompt in
`$EDITOR`: the collector there writes the text to a file and runs its
`edit.command` (the default is `ghostty -e nvim {file}`,
[configuration](configuration.md#4-collector-keys)); the field waits,
with Cancel, until the editor closes, then takes the file's content. Exit
without saving (`:cq`) and the text stays as it was. Which machine:
Settings › Composer names one; else the machine this device sits on,
which the hub tells from the address (a connected collector linking from
the same one; behind the module's nginx the client address is passed
on); else the session's own machine; else the first connected one whose
collector has the command. A machine without the command cannot be
picked; one that disconnects while the editor is open frees the draft,
and the text saved later still lands when its collector comes back.

The title
renames in
place; **Open on** and **Resolved** sit beside it, the other actions in its
**Actions** menu, **Resume** at the foot once it ended
([sessions](sessions.md)).

## 3. Projects

Each workspace's projects with their board (open issues), live sessions,
progress, documents, memories and the note graph, and a reader and editor
for every note ([knowledge](knowledge.md)).

**Search** is one thing everywhere: the box on Projects and `Ctrl K`, from any
page. It looks through every note's title, summary and body by word prefix,
ranked across workspaces (those in the scope), with the matching text shown.
Narrow it by kind (or `kind:issue` in the text), order it by best match,
newest or most recalled, and leave out retired notes, which otherwise come
after the live ones. `Ctrl K` finds sessions and pages too (and offers to
search the past sessions for the text), and a note opens beside the page you
are on. Typing `[[` in the composer searches the same way and inserts the
link.

**Graph** is for exploring what the notes say about each other.

- **Search** (`/`) is the same full-text search: matches stay bright and the
  rest dims, `n` steps through them, **Only matches** hides the others.
- A click selects a note: it and its neighbours stay bright, and the panel
  beside the graph shows its links to walk along. A double click, or
  **Open**, reads it. With nothing selected, the panel has the numbers and the
  most linked notes.
- **Local graph** (`l`) keeps only what is within 1 to 4 links of the note.
- **Path from here** (`p`), then a click on another note, finds the shortest
  way between them.
- The bar filters by kind, project, retired notes and notes without links; the
  slider goes back in time and plays the growth.
- **Display** sets the colour (kind, project, age, links, recalls), the size,
  the layout (free, by project, along a timeline), which kinds of link count
  and the physics. It is remembered on the device.
- **2D or 3D** (`d`). 3D uses WebGL and is fetched the first time it is
  asked for; drag rotates, the wheel zooms, right drag pans. Layouts are 2D's.
- **Full screen** (`f`). Notes and the start sheet open on top of it.
- `0` fits the view, Esc steps back: the path, the selection, the local graph,
  then full screen. Two fingers zoom on a phone.

**In the graph**, on any note, opens the graph with that note selected.

**Work on this**, on an open issue's card and in the issue itself, opens the
start sheet with its workspace and project picked and a prompt written, to
edit. The session claims the issue as soon as it reports, so the card moves
to In progress with the session on it.

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

**Machines** shows the hub's own machine (uptime, CPU, temperature,
memory, disks, system), then every node, that is every machine with a
collector. A node's card has how it is doing (up, CPU, temperature,
memory, battery), its link and round trip, the sessions on it, its copy
of Claude's state, its share of the workspace sync, whether its screen is
unlocked (with **Lock**), and **Wake** while it is down. A node's page
starts with the machine itself:

- tiles for uptime (with the time slept and the last resume), CPU,
  memory, each disk and the battery
- the OS, the kernel and whether a reboot is pending
- its power: Lock, Suspend, Wake, Forget ([machines and power](machines.md))

Then it gathers what the other areas know about the node:

- the sessions on it now
- Claude's usage there over the range in the bar (sessions, working time,
  cost, lines, CPU, activity by state)
- its copy of Claude's state
- its workspace sync (units in step, what moved and what it cost in the
  range, the units not in step there, its large transfers)

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

## 6. Themes

The theme menu in the top bar and Settings › Colours pick the mode (system,
light, dark) and the colour scheme, per device: Neutral, ccdash, Catppuccin,
Gruvbox, Nord, Tokyo Night, Rosé Pine, Everforest, Kanagawa, Solarized,
Dracula (dark only), or **Wallpaper**.

**Wallpaper** follows a machine's desktop colours, as they change. The
collector reads them from the files the desktop already generates, on every
machine that has one, and reports them to the hub, so the phone can follow
them too:

| Read first                          | Otherwise         |
| ----------------------------------- | ----------------- |
| `DankMaterialShell/dms-colors.json` | `wal/colors.json` |

Both under `$XDG_CACHE_HOME` (`~/.cache`).

DankMaterialShell's has both modes and Material's colour roles; pywal's has
one mode, which then decides light or dark. The newer file wins, so running
`wal` on a machine that used to run DankMaterialShell takes effect. pywalfox
needs nothing here: it shows the same `wal/colors.json` in Firefox, and a page
cannot read an extension. With several machines reporting, Settings › Colours
picks which one to follow; a machine that stops reporting leaves its last
colours in place.

A palette names colours, not meanings: pywal fills its slots in any order. So
the panel takes each state's hue from the nearest colour the wallpaper has,
makes one at the wallpaper's strength when it has none, and moves lightness
until text reads against the background (`web/src/lib/theme.ts`).

## 7. Desktop (DankMaterialShell)

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
