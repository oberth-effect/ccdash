# Windows

A Windows machine runs the collector, the hooks, the CLI and the channel
against a hub on Linux. The hub, the DankMaterialShell plugin and niri are
Linux only.

1. [Install](#1-install)
2. [Hooks and statusline](#2-hooks-and-statusline)
3. [Start at logon](#3-start-at-logon)
4. [The hub's side](#4-the-hubs-side)
5. [Where things are](#5-where-things-are)
6. [Differences](#6-differences)

## 1. Install

```powershell
cargo build --release      # MSVC or MinGW toolchain; ring and SQLite need a C compiler
copy target\release\ccdash.exe $env:USERPROFILE\.local\bin\   # any directory on PATH
```

`%APPDATA%\ccdash\config.toml`:

```toml
[hub]
url = "wss://ccdash.example.org/collect"
token_file = 'C:\Users\me\AppData\Roaming\ccdash\token'
```

The host name defaults to the DNS host name, lowercased; `host = "…"`
sets another. The hub must hold a token under it.

## 2. Hooks and statusline

ccdash on PATH, PowerShell 7. This replaces the `hooks` and `statusLine`
of `%USERPROFILE%\.claude\settings.json`, as the Home Manager module does;
with a statusline of your own, use `ccdash tee -- <its command>`.

```powershell
$path = "$env:USERPROFILE\.claude\settings.json"
$settings = if (Test-Path $path) { Get-Content $path -Raw | ConvertFrom-Json -AsHashtable } else { @{} }
$tools = 'PreToolUse', 'PostToolUse', 'PostToolUseFailure', 'PermissionRequest', 'PermissionDenied'
$events = $tools + 'SessionStart', 'SessionEnd', 'UserPromptSubmit', 'Notification', 'Stop',
  'StopFailure', 'PostCompact', 'SubagentStart', 'SubagentStop', 'TaskCreated', 'TaskCompleted',
  'MessageDisplay'
$settings.hooks = @{}
foreach ($event in $events) {
  $hook = if ($event -eq 'PermissionRequest') {
    @{ type = 'command'; command = 'ccdash ask'; timeout = 86400 }
  } else {
    @{ type = 'command'; command = 'ccdash emit'; timeout = 5 }
  }
  $entry = @{ hooks = @($hook) }
  if ($tools -contains $event) { $entry.matcher = '*' }
  $settings.hooks[$event] = @($entry)
}
$settings.statusLine = @{ type = 'command'; command = 'ccdash tee'; padding = 0 }
$settings | ConvertTo-Json -Depth 10 | Set-Content $path -Encoding utf8
```

Replies from the panel need the channel: start sessions with
`--channels plugin:ccdash@inline --plugin-dir <ccdash>\claude-plugin`
(`--dangerously-load-development-channels` instead of `--channels` until
the channel is allowed in Claude Code's managed settings).

## 3. Start at logon

```powershell
$exe = (Get-Command ccdash).Source
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
Set-ItemProperty $run ccdash-collector "`"$exe`" collector --background"
```

The `Run` key needs no elevation; a scheduled task (`schtasks /SC
ONLOGON`) does. `--background` starts the collector again with no console
window and returns; a window opened for the start alone (the `Run` key's)
closes at once. Its log is `%LOCALAPPDATA%\ccdash\state\collector.log`,
rewritten at each start; two starts at once leave one collector and its
log. Programs it runs share that hidden console; an editor (Edit in nvim)
or an attach opens a window of its own.

## 4. The hub's side

- A collector token for the machine's host name (`collectors` /
  `collect.tokens`).
- The machine's workspace directories in `workspaces.dirs`, as Windows
  writes them: `'C:\Users\me\code\work'`. A directory's last component
  names its workspace, so `C:\Users\me\code\work` and
  `/home/me/code/work` are the same workspace.

## 5. Where things are

| What        | Where                                                                     |
| ----------- | ------------------------------------------------------------------------- |
| config      | `%APPDATA%\ccdash\config.toml`                                            |
| state       | `%LOCALAPPDATA%\ccdash\state` (outbox, sync index, notes cache, the log)  |
| runtime     | `%LOCALAPPDATA%\ccdash\run` (pending titles)                              |
| sockets     | named pipes `\\.\pipe\ccdash-api-<hash>`, `\\.\pipe\ccdash-ingest-<hash>` |
| Claude Code | `%USERPROFILE%\.claude`, or `CLAUDE_CONFIG_DIR`                           |

The pipes' names hash the socket path under the runtime directory, so
`CCDASH_RUNTIME_DIR` still gives a second collector a place of its own.
They let in the current user only, and no remote client. The state and
runtime folders get a DACL of their own for the user alone, which what
they hold inherits, as mode 0700 does on Linux. `XDG_*` variables still
win over the folders above, but for `XDG_RUNTIME_DIR`: the pipes are
named after the runtime folder, which the collector and every hook must
agree on, and only `LOCALAPPDATA` is the same for all of them. The
collector's log names its pipes.

## 6. Differences

| Area                | On Windows                                                                                            |
| ------------------- | ----------------------------------------------------------------------------------------------------- |
| stop a session      | Claude Code and every process under it end at once (`TerminateProcess`); no SessionEnd hook           |
| interrupt           | the running Bash call's process tree is ended at once                                                 |
| open                | no window focus; a background session is attached in a new console                                    |
| editor              | `nvim {file}` in a new console; Windows Terminal keeps a tab open after a non-zero exit (`:cq`)       |
| machine             | no load average (reads 0) or CPU temperature; wake-on-LAN: the card is armed (`powercfg`, 10 min)     |
| lock, suspend       | `LockWorkStation`; `SetSuspendState` from a hidden PowerShell (sleep, not hibernation)                |
| state sync          | `ReadDirectoryChangesW` instead of inotify; sessions do not resume across Linux and Windows           |
| workspace sync      | one recursive watch on the root; no open-file check for quiescence; git dirs outside a unit unwatched |
| network shares      | not watched (no extended change records): a root on one is swept every 30 min                         |
| launches from panel | into the machine's own directory of the workspace, when the hub lists one per machine                 |
| npm's `claude.cmd`  | runs, through `cmd.exe`; a batch file takes no multi-line argument, so such prompts need `claude.exe` |
| claims              | compared as absolute paths: they only meet on machines with the same paths                            |

A session's transcripts live under its directory's name
(`C--Users-me-code-x`), which no Linux machine has, so the hub's search
finds Windows sessions but they resume only on Windows.
