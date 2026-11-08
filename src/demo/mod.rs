//! `ccdash demo`: a hub on a fictional workspace, for screenshots and for
//! trying the panel without a setup. A fresh database each run holds a week
//! of history written through the hub's own reducer; two stand-in machines
//! keep sessions open while it runs. Nothing in it is real.

mod live;
mod script;
mod seed;
mod timeline;

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use tracing::info;

use crate::{
    config::HubConfig, hub::reports::Reports, store::Store, util::now_ms, workspace::Workspaces,
};

/// The bootstrap token: the demo is fictional and listens on loopback.
const TOKEN: &str = "demo";
/// Marks a directory the demo made, the only kind it ever empties.
const MARKER: &str = ".ccdash-demo";

/// Runs the demo hub in `dir` (made, or emptied when it is the demo's own)
/// on `listen` until a signal.
pub fn run(dir: Option<PathBuf>, listen: SocketAddr) -> Result<()> {
    let dir = dir.unwrap_or_else(|| std::env::temp_dir().join("ccdash-demo"));
    fresh(&dir)?;
    let cfg = Arc::new(config(&dir, listen)?);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let now = now_ms();
            let workspaces = Workspaces::new(&cfg.workspaces.dirs, "");
            let mut limits = timeline::Limits::new(now);
            {
                let store = Store::open(&cfg.store_path(), cfg.store.retention_days)?;
                let reports = Reports::open(&cfg.reports_dir(), cfg.reports.max_mb)?;
                seed::seed(&store, &reports, &workspaces, now, &mut limits).await?;
            }
            info!("demo history written to {}", cfg.store_path().display());
            println!("ccdash demo: http://{listen}/#token={TOKEN}");
            crate::hub::serve(
                cfg.clone(),
                Some(Box::new(move |hub| live::spawn(hub, limits))),
            )
            .await
        })
}

/// Makes `dir`, or empties it when an earlier demo made it.
fn fresh(dir: &Path) -> Result<()> {
    if dir.exists() {
        if !dir.join(MARKER).is_file() {
            bail!(
                "{} exists and is not a ccdash demo's: pick another --dir",
                dir.display()
            );
        }
        std::fs::remove_dir_all(dir).with_context(|| format!("emptying {}", dir.display()))?;
    }
    std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    std::fs::write(dir.join(MARKER), "")?;
    std::fs::write(dir.join("token"), TOKEN)?;
    Ok(())
}

/// The demo hub's configuration, also left in `dir/hub.toml` to read.
fn config(dir: &Path, listen: SocketAddr) -> Result<HubConfig> {
    // NOTE: a JSON string is a valid TOML basic string
    let q = |s: String| serde_json::to_string(&s).unwrap_or_default();
    let path = |name: &str| q(dir.join(name).display().to_string());
    let dirs: Vec<String> = script::WORKSPACES
        .iter()
        .map(|ws| q(format!("{}/{ws}", script::ROOT)))
        .collect();
    let toml = format!(
        "host = \"homelab\"\n\
        [http]\nlisten = [{listen}]\ntoken_file = {token}\npublic_url = {url}\n\
        [store]\npath = {db}\n\
        [push]\nenabled = false\n\
        [knowledge]\nreflect = false\n\
        [workspaces]\ndirs = [{dirs}]\n\
        [sync]\ndir = {sync}\n\
        [reports]\ndir = {reports}\n",
        listen = q(listen.to_string()),
        token = path("token"),
        url = q(format!("http://{listen}")),
        db = path("ccdash.db"),
        dirs = dirs.join(", "),
        sync = path("claude"),
        reports = path("reports"),
    );
    std::fs::write(dir.join("hub.toml"), &toml)?;
    toml::from_str(&toml).context("the demo's hub configuration")
}
