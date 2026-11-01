//! CPU and memory of every session's process tree, read from `/proc`.

use std::{collections::HashMap, fs, time::Duration};

use crate::{
    collector::{Local, LocalCmd},
    model::ProcInfo,
    state::ProcPoll,
    util::{now_ms, truncate},
};

/// `USER_HZ`, the unit of CPU times in `/proc/<pid>/stat`.
const TICKS_PER_SEC: f64 = 100.0;
const TOP: usize = 5;
const CMD_LEN: usize = 160;
/// PSS costs ~1.5 ms per large process to read, so it is refreshed less often.
const MEM_EVERY_MS: i64 = 30_000;
const SECRET_WORDS: &[&str] = &["token", "secret", "passw", "apikey", "api-key", "api_key"];
/// In the command line of the shell Claude Code runs a Bash call in.
const SHELL_MARK: &str = "/shell-snapshots/";
/// A shell may have started this long before its call's PreToolUse was
/// stamped: start times come in ticks, and the stamps from another process.
const SHELL_SLACK_MS: i64 = 2_000;
/// How long a command gets to honour SIGTERM.
const TERM_GRACE: Duration = Duration::from_secs(3);

struct Stat {
    ppid: u32,
    /// The process group.
    pgrp: u32,
    comm: String,
    /// utime + stime + cutime + cstime. A reaped child's total moves into its
    /// parent's cutime, so the sum over a tree only grows.
    ticks: u64,
    /// Start time in ticks since boot.
    start: u64,
}

/// CPU totals from the previous poll, keyed by `(pid, start)` so a reused pid
/// starts fresh.
pub struct Poller {
    procs: HashMap<(u32, u64), u64>,
    /// Memory per process and when it was read.
    mem: HashMap<(u32, u64), (u64, i64)>,
    /// Parent session of forked session roots, read once per process.
    forks: HashMap<(u32, u64), Option<String>>,
    trees: HashMap<(u32, u64), u64>,
    last: Option<i64>,
    boot_ms: i64,
}

impl Poller {
    pub fn new() -> Self {
        Self {
            procs: HashMap::new(),
            mem: HashMap::new(),
            forks: HashMap::new(),
            trees: HashMap::new(),
            last: None,
            boot_ms: boot_ms(),
        }
    }

    /// Measures the trees under `roots` (`(session id, pid)`). CPU figures are
    /// averages since the previous call and read 0 on the first one.
    pub fn poll(&mut self, roots: &[(String, u32)], now: i64) -> Vec<ProcPoll> {
        let table = scan();
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (pid, stat) in &table {
            children.entry(stat.ppid).or_default().push(*pid);
        }
        let elapsed = self
            .last
            .map(|last| (now - last) as f64 / 1000.0)
            .filter(|secs| *secs > 0.0);
        let pct =
            |ticks: u64| elapsed.map_or(0.0, |secs| ticks as f64 / TICKS_PER_SEC / secs * 100.0);

        let (mut procs, mut trees, mut out) = (HashMap::new(), HashMap::new(), Vec::new());
        let mut mem = HashMap::new();
        let mut forks = HashMap::new();
        for (session, root) in roots {
            let Some(root_stat) = table.get(root) else {
                continue;
            };
            let members = subtree(*root, &children);
            let total: u64 = members.iter().map(|pid| table[pid].ticks).sum();
            let tree_key = (*root, root_stat.start);
            let cpu_pct = self
                .trees
                .get(&tree_key)
                .map_or(0.0, |prev| pct(total.saturating_sub(*prev)));
            trees.insert(tree_key, total);
            let forked_from = self
                .forks
                .remove(&tree_key)
                .unwrap_or_else(|| fork_parent(&args(*root)));
            forks.insert(tree_key, forked_from.clone());

            let mut mem_total = 0;
            let mut infos: Vec<ProcInfo> = members
                .iter()
                .map(|pid| {
                    let stat = &table[pid];
                    let key = (*pid, stat.start);
                    let started_at =
                        self.boot_ms + (stat.start as f64 * 1000.0 / TICKS_PER_SEC) as i64;
                    let ticks = match self.procs.get(&key) {
                        Some(prev) => stat.ticks.saturating_sub(*prev),
                        None if self.last.is_some_and(|last| started_at >= last) => stat.ticks,
                        None => 0,
                    };
                    procs.insert(key, stat.ticks);
                    let mem_bytes = match self.mem.get(&key) {
                        Some(&(bytes, at)) if now - at < MEM_EVERY_MS => {
                            mem.insert(key, (bytes, at));
                            bytes
                        }
                        _ => {
                            let bytes = mem_bytes(*pid);
                            mem.insert(key, (bytes, now));
                            bytes
                        }
                    };
                    mem_total += mem_bytes;
                    ProcInfo {
                        pid: *pid,
                        name: stat.comm.clone(),
                        cmd: String::new(),
                        cpu_pct: pct(ticks),
                        mem_bytes,
                        started_at,
                    }
                })
                .collect();
            infos.sort_by(|a, b| {
                b.cpu_pct
                    .total_cmp(&a.cpu_pct)
                    .then(b.mem_bytes.cmp(&a.mem_bytes))
            });
            infos.truncate(TOP);
            for info in &mut infos {
                let args = args(info.pid);
                info.name = display_name(&info.name, &args);
                info.cmd = truncate(&mask_secrets(&args).join(" "), CMD_LEN);
            }
            out.push(ProcPoll {
                session: session.clone(),
                cpu_pct,
                mem_bytes: mem_total,
                procs: u32::try_from(members.len()).unwrap_or(u32::MAX),
                top: infos,
                forked_from,
            });
        }
        self.procs = procs;
        self.mem = mem;
        self.forks = forks;
        self.trees = trees;
        self.last = Some(now);
        out
    }
}

pub async fn run(local: Local, every: Duration) {
    let tx = local.sender();
    let mut poller = Poller::new();
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        let Ok(roots) = local.pids().await else {
            return;
        };
        let now = now_ms();
        let Ok((back, polls)) = tokio::task::spawn_blocking(move || {
            let polls = poller.poll(&roots, now);
            (poller, polls)
        })
        .await
        else {
            return;
        };
        poller = back;
        if tx.send(LocalCmd::Procs(polls, now)).await.is_err() {
            return;
        }
    }
}

fn boot_ms() -> i64 {
    fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|stat| {
            stat.lines()
                .find_map(|line| line.strip_prefix("btime ")?.trim().parse::<i64>().ok())
        })
        .map_or(0, |secs| secs * 1000)
}

/// A child of a Claude process, as `running_shells` judges it.
struct Child {
    pid: u32,
    pgrp: u32,
    /// Ticks since boot: with the pid, who this is.
    start: u64,
    started_ms: i64,
    args: String,
}

/// Which children are the shells of the Bash calls running since `since_ms`.
/// Claude Code starts each call's shell in a process group of its own, and
/// the shell sources its snapshot first; its MCP servers stay in Claude's
/// group, and a background shell from earlier is older than the running call.
fn running_shells(children: &[Child], since_ms: i64) -> Vec<(u32, u64)> {
    children
        .iter()
        .filter(|c| c.pid == c.pgrp && c.args.contains(SHELL_MARK))
        .filter(|c| c.started_ms >= since_ms - SHELL_SLACK_MS)
        .map(|c| (c.pid, c.start))
        .collect()
}

fn signal_group(pgrp: u32, signal: i32) {
    // SAFETY: `kill` takes plain integers; a gone group is an error we ignore
    unsafe { libc::kill(-(pgrp as i32), signal) };
}

/// Ends the Bash calls the Claude process `claude` is waiting on, as Esc in
/// the terminal does: the shell's whole group, so the command goes too, and
/// by force if it ignores the request. A background shell stays. How many
/// were asked to stop.
pub async fn end_tool_shells(claude: u32, since_ms: i64) -> usize {
    let boot = boot_ms();
    let children: Vec<Child> = scan()
        .into_iter()
        .filter(|(_, stat)| stat.ppid == claude)
        .map(|(pid, stat)| Child {
            pid,
            pgrp: stat.pgrp,
            start: stat.start,
            started_ms: boot + (stat.start as f64 / TICKS_PER_SEC * 1000.0) as i64,
            args: args(pid).join(" "),
        })
        .collect();
    let shells = running_shells(&children, since_ms);
    for &(pid, _) in &shells {
        signal_group(pid, libc::SIGTERM);
    }
    let count = shells.len();
    tokio::spawn(async move {
        tokio::time::sleep(TERM_GRACE).await;
        for (pid, start) in shells {
            // NOTE: the start time tells a stubborn shell from a reused pid
            if read_stat(pid).is_some_and(|stat| stat.start == start) {
                signal_group(pid, libc::SIGKILL);
            }
        }
    });
    count
}

fn scan() -> HashMap<u32, Stat> {
    let Ok(dir) = fs::read_dir("/proc") else {
        return HashMap::new();
    };
    dir.filter_map(|entry| {
        let pid = entry.ok()?.file_name().to_str()?.parse::<u32>().ok()?;
        Some((pid, read_stat(pid)?))
    })
    .collect()
}

fn read_stat(pid: u32) -> Option<Stat> {
    parse_stat(&fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

fn parse_stat(raw: &str) -> Option<Stat> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    // NOTE: comm may contain spaces and parens; fields restart after the last ')'
    let fields: Vec<&str> = raw.get(close + 2..)?.split(' ').collect();
    let field = |n: usize| fields.get(n - 3)?.parse::<i64>().ok();
    let ticks = [14, 15, 16, 17]
        .iter()
        .map(|&n| field(n).map(|v| v.max(0) as u64))
        .sum::<Option<u64>>()?;
    Some(Stat {
        ppid: u32::try_from(field(4)?).ok()?,
        pgrp: u32::try_from(field(5)?).ok()?,
        comm: raw[open + 1..close].to_owned(),
        ticks,
        start: u64::try_from(field(22)?).ok()?,
    })
}

fn subtree(root: u32, children: &HashMap<u32, Vec<u32>>) -> Vec<u32> {
    let mut members = vec![root];
    let mut i = 0;
    while i < members.len() {
        let parent = members[i];
        if let Some(kids) = children.get(&parent) {
            members.extend(kids.iter().filter(|kid| **kid != parent));
        }
        i += 1;
    }
    members
}

/// PSS from `smaps_rollup`, falling back to RSS where that is unreadable.
fn mem_bytes(pid: u32) -> u64 {
    let field = |file: &str, key: &str| {
        let text = fs::read_to_string(format!("/proc/{pid}/{file}")).ok()?;
        let line = text.lines().find(|l| l.starts_with(key))?;
        line.split_whitespace().nth(1)?.parse::<u64>().ok()
    };
    field("smaps_rollup", "Pss:")
        .or_else(|| field("status", "VmRSS:"))
        .map_or(0, |kib| kib * 1024)
}

fn args(pid: u32) -> Vec<String> {
    fs::read(format!("/proc/{pid}/cmdline"))
        .unwrap_or_default()
        .split(|b| *b == 0)
        .filter(|arg| !arg.is_empty())
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect()
}

/// The session a `claude --fork-session --resume <id or transcript>` process
/// continues from.
fn fork_parent(args: &[String]) -> Option<String> {
    if !args.iter().any(|a| a == "--fork-session") {
        return None;
    }
    let i = args.iter().position(|a| a == "--resume" || a == "-r")?;
    let value = args.get(i + 1)?;
    let id = value.rsplit('/').next()?.trim_end_matches(".jsonl");
    let uuid = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    uuid.then(|| id.to_owned())
}

/// A readable program name: the Nix package of the program (or of the
/// script an interpreter runs), else the executable without wrapper noise.
fn display_name(comm: &str, args: &[String]) -> String {
    let base = |arg: &str| arg.rsplit('/').next().unwrap_or(arg).to_owned();
    let interpreter = args.first().is_some_and(|arg| {
        let name = base(arg);
        [
            "node", "python", "bash", "sh", "bun", "deno", "ruby", "perl",
        ]
        .iter()
        .any(|i| name.starts_with(i))
    });
    let program = if interpreter {
        args.get(1)
    } else {
        args.first()
    };
    if let Some(name) = program.and_then(|arg| nix_package(arg)) {
        return name;
    }
    let name = program.map_or_else(|| comm.to_owned(), |arg| base(arg));
    let name = name.trim_start_matches('.');
    name.strip_suffix("-wrapped").unwrap_or(name).to_owned()
}

/// `/nix/store/<hash>-playwright-mcp-0.0.82/lib/…` → `playwright-mcp`.
fn nix_package(path: &str) -> Option<String> {
    let entry = path.strip_prefix("/nix/store/")?.split('/').next()?;
    let (_, name) = entry.split_once('-')?;
    let parts: Vec<&str> = name
        .split('-')
        .take_while(|part| !part.starts_with(|c: char| c.is_ascii_digit()))
        .collect();
    (!parts.is_empty()).then(|| parts.join("-"))
}

/// Hides values of `--token=x`, `--api-key x` and similar arguments.
fn mask_secrets(args: &[String]) -> Vec<String> {
    let secret = |arg: &str| {
        let lower = arg.to_ascii_lowercase();
        SECRET_WORDS.iter().any(|word| lower.contains(word))
    };
    let mut out = Vec::with_capacity(args.len());
    let mut mask_next = false;
    for arg in args {
        if mask_next {
            out.push("…".to_owned());
            mask_next = false;
        } else if let Some((key, _)) = arg.split_once('=').filter(|(key, _)| secret(key)) {
            out.push(format!("{key}=…"));
        } else {
            mask_next = arg.starts_with('-') && secret(arg);
            out.push(arg.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stat_with_odd_comm() {
        let raw = concat!(
            "4242 (tmux: server (1)) S 1 4242 4242 0 -1 4194560 100 0 0 0 ",
            "250 50 7 3 20 0 1 0 1234567 10000 300 18446744073709551615",
        );
        let stat = parse_stat(raw).unwrap();
        assert_eq!(stat.comm, "tmux: server (1)");
        assert_eq!((stat.ppid, stat.ticks, stat.start), (1, 310, 1_234_567));
        assert_eq!(stat.pgrp, 4242);
    }

    #[test]
    fn only_the_running_call_s_shell_is_ended() {
        const SHELL: &str = "bash -c source ~/.claude/shell-snapshots/snapshot-bash-1.sh && eval x";
        let child = |pid, pgrp, started_ms, args: &str| Child {
            pid,
            pgrp,
            start: u64::from(pid),
            started_ms,
            args: args.to_owned(),
        };
        let since = 100_000;
        let picked = running_shells(
            &[
                child(10, 10, since + 50, SHELL),
                child(11, 11, since - 500, SHELL),
                child(20, 5, since + 50, "ccdash channel"),
                child(21, 21, since + 50, "node mcp-server.js"),
                child(22, 5, since + 50, SHELL),
                child(30, 30, since - 60_000, SHELL),
            ],
            since,
        );
        assert_eq!(
            picked,
            [(10, 10), (11, 11)],
            "not an MCP server, a shell in Claude's group, or an older background shell"
        );
    }

    #[test]
    fn subtree_walks_all_descendants() {
        let children = HashMap::from([(1, vec![2, 3]), (2, vec![4]), (9, vec![10])]);
        let mut members = subtree(1, &children);
        members.sort_unstable();
        assert_eq!(members, [1, 2, 3, 4]);
    }

    #[test]
    fn masks_secret_arguments() {
        let args: Vec<String> = ["mcp", "--api-key", "abc", "--token=xyz", "--port", "80"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            mask_secrets(&args).join(" "),
            "mcp --api-key … --token=… --port 80"
        );
    }

    #[test]
    fn readable_names() {
        let args = |list: &[&str]| list.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        assert_eq!(
            display_name(
                "MainThread",
                &args(&[
                    "/nix/store/v6w-nodejs-24.20.0/bin/node",
                    "/nix/store/wg3-playwright-mcp-0.0.82/lib/node_modules/cli.js"
                ])
            ),
            "playwright-mcp"
        );
        assert_eq!(
            display_name(
                ".claude-wrapped",
                &args(&["/nix/store/ayk-claude-code-2.1.296/bin/.claude-wrapped"])
            ),
            "claude-code"
        );
        assert_eq!(display_name("cargo", &args(&["cargo", "build"])), "cargo");
        assert_eq!(display_name(".foo-wrapped", &[]), "foo");
    }

    #[test]
    fn finds_the_forked_session() {
        let args = |list: &[&str]| list.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        assert_eq!(
            fork_parent(&args(&[
                ".claude-wrapped",
                "--session-id",
                "5175e184-971a-4662-b207-6bf37c73bd66",
                "--fork-session",
                "--resume",
                "/home/u/.claude/projects/x/bd459acf-f32e-4e9b-9a8e-ef2ee7758fca.jsonl",
            ])),
            Some("bd459acf-f32e-4e9b-9a8e-ef2ee7758fca".into())
        );
        assert_eq!(fork_parent(&args(&["claude", "--resume", "abc"])), None);
    }

    #[test]
    fn measures_this_process() {
        let mut poller = Poller::new();
        let roots = [("s".to_owned(), std::process::id())];
        let first = poller.poll(&roots, now_ms());
        assert_eq!(first.len(), 1);
        assert!(first[0].mem_bytes > 0);
        assert!(first[0].top.iter().any(|p| p.pid == std::process::id()));
    }
}
