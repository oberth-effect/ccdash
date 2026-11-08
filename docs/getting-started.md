# Getting started

What to do once the hub and a collector run
([installation](installation.md)).

1. [A first session](#1-a-first-session)
2. [Workspaces and projects](#2-workspaces-and-projects)
3. [The knowledge tools and skills](#3-the-knowledge-tools-and-skills)
4. [Bringing in what you have](#4-bringing-in-what-you-have)
5. [From the phone](#5-from-the-phone)
6. [Settings worth a look](#6-settings-worth-a-look)

## 1. A first session

Start `claude` in a project as you always do. It shows up on **Now** and
under **Sessions** within a second: working, waiting on a permission prompt,
your turn, or parked when you have not answered for a while. Its page has
the conversation, the shell output, the files it touched and its cost
(with `claudeCode.statusLine.enable`). A permission prompt can be answered
in the terminal or in the panel, whichever comes first.

On a workstation, `ccdash status` lists the machine's sessions as the hub
sees them; `ccdash url` prints the panel's address.

## 2. Workspaces and projects

A **workspace** is a directory you listed in the hub's `workspaces`, such as
`/home/me/code`; it is named by its last component (`code`). A **project**
is a directory directly under it, a repository clone as a rule:
`/home/me/code/shop` is the project `shop` of the workspace `code`.

- Memories, issues, progress, documents and reports belong to a workspace,
  and to a project of it or to the whole workspace. Every session of the
  workspace shares them, on every machine.
- A session takes its project from the directory it runs in. At the
  workspace root, name it: `/ccdash:project shop` in the session, or
  `claude --project shop` to start one (the channel's `claude` wrapper
  takes the flag).
- A session outside every workspace still shows in the panel, but the
  knowledge tools refuse it, and the Now page warns while the hub has no
  workspace at all.
- Keep the paths the same on every machine: the workspace is found by path.

## 3. The knowledge tools and skills

Every session gets the ccdash plugin (`claudeCode.plugin.enable`, on by
default):

| Tools                       | For                                            |
| --------------------------- | ---------------------------------------------- |
| `recall`                    | search the notes, or read one in full          |
| `remember`, `verify`        | keep a fact, confirm one still holds           |
| `issue_add`, `issue_update` | open work, with a status and a priority        |
| `progress`                  | a dated entry: what landed, what was decided   |
| `doc_write`                 | plans, specs, anything long                    |
| `report_publish`            | an HTML report, kept on the hub                |
| `claim`, `release`          | tell the other sessions what this one works on |
| `tell_session`              | a message to another running session           |

| Skill             | Does                                                                    |
| ----------------- | ----------------------------------------------------------------------- |
| `/ccdash:wrap-up` | records what the session learned and did, then lets it close (Resolved) |
| `/ccdash:report`  | writes a report in the shared design and publishes it                   |
| `/ccdash:project` | places the session under a project                                      |

At its start a session is told the project's open issues, memories and
recent progress ([knowledge](knowledge.md#3-injections)). Claude uses the
tools more when your `CLAUDE.md` asks it to, for instance:

```markdown
## Knowledge

Durable knowledge lives in ccdash: `recall` before asking about past
decisions, `remember` facts worth keeping, `issue_add` for work we agree on,
`progress` when something lands.
```

## 4. Bringing in what you have

`ccdash import memory` reads a workspace's `.claude/memory` directory (one
note per file, titled by its `MEMORY.md` line) and sends it to the hub
through this machine's collector; it is safe to repeat:

```sh
ccdash import memory ~/code/.claude/memory
```

Notes kept elsewhere, such as Claude Code's own memory directory, go into
the workspace's `.claude/memory` first. Plans come in with
`ccdash import plan`, and `ccdash export` writes every note back out as
markdown ([knowledge](knowledge.md#6-import-and-export)).

## 5. From the phone

Pair it ([installation](installation.md#6-your-phone)) and leave **Now**
open: prompts are answered inline or from the notification's buttons, a
quick reply or a note for the next prompt goes to a session on your turn,
and **Start a session** launches one in the background on a machine you
pick. A notification waits 15 s, so a prompt answered at the desk never
sends one ([prompts and notifications](prompts-and-notifications.md)).

## 6. Settings worth a look

- **Settings › Devices**: the paired devices, push on this one, a test.
- **Settings › Composer**: vim keys (off by default), line numbers, quick
  replies and templates, per device.
- **Settings › Appearance**: light, dark or the system's, and the colour
  schemes.
