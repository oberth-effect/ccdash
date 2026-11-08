//! The demo's machines while its hub runs: two stand-in collectors with the
//! sessions open now, the numbers of their machines and the hub's, all
//! made up. They speak to the hub through its own commands, as a link would.

use std::collections::BTreeMap;

use tokio::sync::mpsc;

use super::{
    script::{LIVE, Now, PROJECTS},
    timeline::{self, Built, Limits, POLL_MS},
};
use crate::{
    hub::core::{Cmd, Hub, Input},
    machine::{Battery, Cpu, Disk, MachineReport, Memory, Nic},
    model::{HostFeatures, ScopedLimit},
    util::now_ms,
    wire::Source,
};

const HOSTS: &[&str] = &["desk", "laptop"];
const GIB: u64 = 1 << 30;
const MACHINES_EVERY_MS: u64 = 30_000;

/// Starts the stand-ins on `hub`; `limits` goes on from the seeded week.
pub fn spawn(hub: Hub, limits: Limits) {
    tokio::spawn(async move {
        if let Err(e) = run(hub, limits).await {
            tracing::warn!("demo machines: {e:#}");
        }
    });
}

async fn run(hub: Hub, mut limits: Limits) -> anyhow::Result<()> {
    let tx = hub.sender();
    let now = now_ms();
    for (conn, host) in HOSTS.iter().enumerate() {
        let (link, mut rx) = mpsc::channel(256);
        // NOTE: what the hub asks of a machine (launches, answers) goes nowhere
        tokio::spawn(async move { while rx.recv().await.is_some() {} });
        tx.send(Cmd::CollectorOpen {
            host: (*host).to_owned(),
            conn: 1_000 + conn as u64,
            terminals: vec!["com.mitchellh.ghostty".into()],
            features: HostFeatures {
                edit: true,
                attach: true,
                launch: true,
                channel: true,
                sync: false,
                workspace: false,
            },
            addr: None,
            tx: link,
        })
        .await?;
        let dirs: BTreeMap<String, Vec<String>> = PROJECTS
            .iter()
            .map(|(ws, projects)| {
                let projects = projects.iter().map(|p| (*p).to_owned()).collect();
                ((*ws).to_owned(), projects)
            })
            .collect();
        tx.send(Cmd::Dirs {
            host: (*host).to_owned(),
            dirs,
        })
        .await?;
    }

    let mut open: Vec<(&'static super::script::Session, Built)> = Vec::new();
    for (n, live) in LIVE.iter().enumerate() {
        let s = &live.session;
        let mut built = timeline::build(s, 0, Some(&live.now));
        let lag = match live.now {
            Now::Working => 25_000,
            Now::Asking => 70_000,
            Now::Done => 4 * 60_000,
        };
        let by = now - lag - built.end;
        timeline::shift(&mut built, by);
        if matches!(live.now, Now::Working) {
            let shell = built.busy.iter().any(|&(_, z, sh)| sh && z >= built.end);
            built.busy.push((built.end, i64::MAX, shell));
        }
        // NOTE: a process for the session, with the ccdash channel open, so the panel replies
        let pid = 41_000 + n as u32;
        for mut env in built.envelopes.drain(..) {
            if env.source == Source::Statusline {
                limits.fill(&mut env);
            }
            env.pid = Some(pid);
            tx.send(Cmd::Input(s.host.to_owned(), Input::Envelope(env)))
                .await?;
        }
        tx.send(Cmd::ChannelOpen {
            host: s.host.to_owned(),
            pid,
        })
        .await?;
        if let Some(mut ask) = built.ask.take() {
            ask.pid = Some(pid);
            tx.send(Cmd::Ask {
                host: s.host.to_owned(),
                id: format!("demo-ask-{n}"),
                envelope: Box::new(ask),
            })
            .await?;
        }
        open.push((s, built));
    }
    let (_, (_, week_resets)) = limits.spend(now, 0.0);
    for host in HOSTS {
        tx.send(Cmd::Input(
            (*host).to_owned(),
            Input::Usage(
                vec![ScopedLimit {
                    label: "Fable".into(),
                    used_pct: 41.0,
                    resets_at: week_resets,
                    severity: "normal".into(),
                    read_at: now,
                }],
                now,
            ),
        ))
        .await?;
    }

    let mut polls = tokio::time::interval(std::time::Duration::from_millis(POLL_MS as u64));
    let mut machines = tokio::time::interval(std::time::Duration::from_millis(MACHINES_EVERY_MS));
    loop {
        tokio::select! {
            _ = polls.tick() => {
                let now = now_ms();
                for host in HOSTS {
                    let here = open
                        .iter()
                        .filter(|(s, _)| s.host == *host)
                        .map(|(s, b)| timeline::poll(s, b, now))
                        .collect();
                    tx.send(Cmd::Input((*host).to_owned(), Input::Procs(here, now))).await?;
                }
            }
            _ = machines.tick() => {
                let now = now_ms();
                for host in HOSTS {
                    tx.send(Cmd::Machine {
                        host: (*host).to_owned(),
                        report: Box::new(machine(host, now)),
                    })
                    .await?;
                }
                tx.send(Cmd::HubMachine(Box::new(machine("homelab", now)))).await?;
            }
        }
    }
}

/// A made-up machine's numbers at `now`, moving a little between reads.
fn machine(host: &str, now: i64) -> MachineReport {
    let wobble = ((now / 30_000) % 7) as f64;
    let (cores, mem_total, mem_used, disk_total, disk_used, up_days, battery, nic) = match host {
        "desk" => (
            16,
            64 * GIB,
            21 * GIB,
            2_000 * GIB,
            1_140 * GIB,
            4,
            None,
            ("enp5s0", "02:00:5e:10:00:01", "192.168.1.20/24", true),
        ),
        "laptop" => (
            8,
            32 * GIB,
            12 * GIB,
            1_000 * GIB,
            610 * GIB,
            1,
            Some(Battery {
                pct: 64.0,
                status: "Discharging".into(),
                ac: Some(false),
            }),
            ("wlp1s0", "02:00:5e:10:00:02", "192.168.1.31/24", false),
        ),
        _ => (
            4,
            16 * GIB,
            5 * GIB,
            500 * GIB,
            212 * GIB,
            23,
            None,
            ("eno1", "02:00:5e:10:00:03", "192.168.1.2/24", true),
        ),
    };
    MachineReport {
        at: now,
        boot_at: now - up_days * 86_400_000 - 3 * 3_600_000,
        slept_ms: if host == "laptop" { 9 * 3_600_000 } else { 0 },
        resumed_at: None,
        os: Some("NixOS 26.05".into()),
        kernel: "6.18.9".into(),
        reboot_pending: Some(false),
        cpu: Cpu {
            cores,
            usage_pct: Some(14.0 + 3.0 * wobble),
            load: [1.2 + 0.1 * wobble, 1.1, 0.9],
            temp_c: Some(48.0 + wobble),
        },
        memory: Memory {
            total: mem_total,
            used: mem_used + (wobble as u64) * (GIB / 8),
            swap_total: 8 * GIB,
            swap_used: 0,
        },
        disks: vec![Disk {
            path: "/".into(),
            total: disk_total,
            used: disk_used,
            avail: disk_total - disk_used,
        }],
        battery,
        nics: vec![Nic {
            name: nic.0.into(),
            mac: nic.1.into(),
            ipv4: vec![nic.2.into()],
            wake: nic.3,
        }],
        can_suspend: host != "homelab",
        can_lock: host != "homelab",
        locked: (host != "homelab").then_some(false),
    }
}
