//! Attachments on a session's machine: a prompt on its way to Claude
//! (through the channel, as a launch's prompt, as a note with the next
//! prompt) may carry markers (`crate::attachments`); each one's file is
//! fetched from the hub with this machine's token into
//! `$XDG_STATE_HOME/ccdash/attachments/<id>/<name>` and the marker
//! becomes the line naming that path, which Claude reads with its own
//! tools. The directory is a cache: a file fetched once is reused, and
//! what was not touched for `[attachments] keep_days` is dropped. A file
//! that cannot be fetched becomes a line saying so, so the prompt still
//! goes out and Claude can tell the owner.

use std::{
    fs,
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use tracing::{info, warn};

use crate::{
    attachments::{self as shared, Kind},
    config::CollectorConfig,
    paths,
    sync::collector::{base_url, hub_http_config},
    util::expand_env,
};

/// One fetch may take this long: a video over a slow link.
const FETCH_TIMEOUT: Duration = Duration::from_secs(600);
const DAY: Duration = Duration::from_secs(86_400);

/// Where this machine keeps fetched attachments: `[attachments] dir`,
/// else `attachments/` in ccdash's state directory.
pub fn dir(cfg: &CollectorConfig) -> PathBuf {
    match &cfg.attachments.dir {
        Some(dir) => PathBuf::from(expand_env(&dir.to_string_lossy())),
        None => paths::state_dir().join("attachments"),
    }
}

/// The permission rule that lets a session read the fetched files without
/// asking, in Claude Code's syntax (`//` starts an absolute path).
pub fn read_rule(cfg: &CollectorConfig) -> String {
    format!("Read(/{}/**)", dir(cfg).display())
}

/// `text` with every attachment marker replaced by its local file's line.
/// Blocking: it may fetch. Text without markers comes back as it is.
pub fn materialize(cfg: &CollectorConfig, text: &str) -> String {
    if shared::marked(text).is_empty() {
        return text.to_owned();
    }
    if !cfg.attachments.enable {
        return shared::resolve(text, |id| {
            shared::undelivered(
                id,
                "attachments are off on this machine ([attachments] enable)",
            )
        });
    }
    let root = dir(cfg);
    shared::resolve(text, |id| match local(cfg, &root, id) {
        Ok((kind, path)) => shared::delivered(kind, &path.to_string_lossy()),
        Err(e) => {
            warn!("attachment {id}: {e:#}");
            shared::undelivered(id, &format!("{e:#}"))
        }
    })
}

/// The attachment's file here, fetched unless it already is.
fn local(cfg: &CollectorConfig, root: &Path, id: &str) -> Result<(Kind, PathBuf)> {
    let folder = root.join(id);
    if let Some(path) = cached(&folder) {
        // NOTE: the sweep goes by the folder's time, so a reused file stays
        let _ = fs::File::open(&folder).and_then(|f| f.set_modified(SystemTime::now()));
        return Ok((kind_of(&path), path));
    }
    fetch(cfg, &folder, id)
}

/// The one finished file in an attachment's folder.
fn cached(folder: &Path) -> Option<PathBuf> {
    fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.is_file() && p.extension().is_none_or(|ext| ext != "part"))
}

fn kind_of(path: &Path) -> Kind {
    let mut head = vec![0u8; 4096];
    let read = fs::File::open(path)
        .and_then(|mut f| f.read(&mut head))
        .unwrap_or(0);
    shared::sniff(&head[..read]).0
}

fn fetch(cfg: &CollectorConfig, folder: &Path, id: &str) -> Result<(Kind, PathBuf)> {
    let Some(token_file) = &cfg.hub.token_file else {
        bail!("this machine's collector has no hub.token_file");
    };
    let token_file = PathBuf::from(expand_env(&token_file.to_string_lossy()));
    let token = fs::read_to_string(&token_file)
        .with_context(|| format!("reading {}", token_file.display()))?;
    let url = format!("{}/attachments/-/fetch/{id}", base_url(&cfg.hub.url));
    let agent: ureq::Agent = hub_http_config(FETCH_TIMEOUT).into();
    let response = agent
        .get(&url)
        .header("Authorization", &format!("Bearer {}", token.trim()))
        .call()
        .map_err(|e| anyhow!("the hub is unreachable ({e})"))?;
    let status = response.status();
    if status.as_u16() == 404 {
        bail!("the hub no longer has it (removed, or its time there ran out)");
    }
    // NOTE: a proxy answers for a hub that is down
    if matches!(status.as_u16(), 502..=504) {
        bail!("the hub is unreachable ({status} from the proxy)");
    }
    if !status.is_success() {
        bail!("the hub refused it ({status})");
    }
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let name = header("x-ccdash-name")
        .and_then(|n| B64.decode(n).ok())
        .and_then(|n| String::from_utf8(n).ok())
        .map(|n| shared::safe_name(&n))
        .unwrap_or_else(|| "file".into());
    let kind = match header("x-ccdash-kind").as_deref() {
        Some("image") => Kind::Image,
        Some("audio") => Kind::Audio,
        Some("video") => Kind::Video,
        _ => Kind::File,
    };
    fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
    if let Some(root) = folder.parent() {
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    }
    fs::set_permissions(folder, fs::Permissions::from_mode(0o700))?;
    let path = folder.join(&name);
    let tmp = folder.join(format!("{name}.part"));
    let _ = fs::remove_file(&tmp);
    let written = (|| -> Result<u64> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .with_context(|| format!("creating {}", tmp.display()))?;
        let mut body = response.into_body();
        let bytes =
            std::io::copy(&mut body.as_reader(), &mut file).context("the download broke off")?;
        file.sync_all()?;
        Ok(bytes)
    })();
    let bytes = match written {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    };
    fs::rename(&tmp, &path).with_context(|| format!("placing {}", path.display()))?;
    info!(
        "attachment {id}: {} ({bytes} bytes) fetched",
        path.display()
    );
    Ok((kind, path))
}

/// Drops the fetched attachments nobody used for `[attachments]
/// keep_days` (0 keeps them). Returns how many went.
pub fn sweep(cfg: &CollectorConfig) -> usize {
    let keep = cfg.attachments.keep_days;
    if keep == 0 {
        return 0;
    }
    let Some(before) = SystemTime::now().checked_sub(DAY * keep) else {
        return 0;
    };
    let Ok(entries) = fs::read_dir(dir(cfg)) else {
        return 0;
    };
    let mut gone = 0;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|t| t < before);
        // NOTE: only folders this module made (an id) are ever removed
        if old
            && shared::valid_id(&name.to_string_lossy())
            && fs::remove_dir_all(entry.path()).is_ok()
        {
            gone += 1;
        }
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attachments::marker;

    const ID: &str = "0123456789abcdef01234567";

    fn cfg(tag: &str) -> (CollectorConfig, PathBuf) {
        let root = std::env::temp_dir().join(format!("ccdash-catt-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut cfg = CollectorConfig::default();
        cfg.attachments.dir = Some(root.clone());
        (cfg, root)
    }

    #[test]
    fn a_cached_file_needs_no_hub_and_keeps_its_kind() {
        let (cfg, root) = cfg("cached");
        fs::create_dir_all(root.join(ID)).unwrap();
        fs::write(root.join(ID).join("board.jpg"), b"\xff\xd8\xff\xe0....").unwrap();
        fs::write(root.join(ID).join("other.part"), b"half").unwrap();
        let text = format!("what is this?\n{}", marker(ID));
        let out = materialize(&cfg, &text);
        assert_eq!(
            out,
            format!(
                "what is this?\n[Attached image: {}]",
                root.join(ID).join("board.jpg").display()
            )
        );
        assert_eq!(materialize(&cfg, "no markers"), "no markers");
        assert_eq!(read_rule(&cfg), format!("Read(/{}/**)", root.display()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn what_cannot_be_fetched_is_said_in_the_prompt() {
        let (mut cfg, root) = cfg("missing");
        let text = format!("look {}", marker(ID));
        // no token file: nothing to fetch with
        let out = materialize(&cfg, &text);
        assert!(out.starts_with("look [An attachment (0123456789abcdef01234567) did not reach"));
        assert!(out.contains("hub.token_file"), "{out}");
        // a hub nobody listens on
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("token"), "t\n").unwrap();
        cfg.hub.token_file = Some(root.join("token"));
        cfg.hub.url = "ws://127.0.0.1:9/collect".into();
        let out = materialize(&cfg, &text);
        assert!(out.contains("the hub is unreachable"), "{out}");
        assert!(!root.join(ID).join("file").exists());
        cfg.attachments.enable = false;
        assert!(materialize(&cfg, &text).contains("attachments are off on this machine"));
        let _ = fs::remove_dir_all(root);
    }

    /// A hub of one answer: reads a request, checks the token, replies.
    fn hub(
        status: &str,
        headers: &str,
        body: &'static [u8],
    ) -> (String, std::thread::JoinHandle<String>) {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}/collect", listener.local_addr().unwrap());
        let reply = format!(
            "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n{headers}\r\n",
            body.len()
        );
        let served = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut piece = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = conn.read(&mut piece).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&piece[..n]);
            }
            conn.write_all(reply.as_bytes()).unwrap();
            conn.write_all(body).unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });
        (url, served)
    }

    #[test]
    fn a_file_is_fetched_once_with_the_machines_token_and_kept_private() {
        let (mut cfg, root) = cfg("fetch");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("token"), "machine-token\n").unwrap();
        cfg.hub.token_file = Some(root.join("token"));
        let name = B64.encode("../deska č. 2.jpg");
        let (url, served) = hub(
            "200 OK",
            &format!("x-ccdash-name: {name}\r\nx-ccdash-kind: image\r\n"),
            b"\xff\xd8\xff\xe0 the photo",
        );
        cfg.hub.url = url;
        let text = format!("what is this?\n{}", marker(ID));
        let out = materialize(&cfg, &text);
        let path = root.join(ID).join("deska_č._2.jpg");
        assert_eq!(
            out,
            format!("what is this?\n[Attached image: {}]", path.display())
        );
        assert_eq!(fs::read(&path).unwrap(), b"\xff\xd8\xff\xe0 the photo");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(root.join(ID)).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let request = served.join().unwrap();
        assert!(
            request.starts_with(&format!("GET /attachments/-/fetch/{ID} ")),
            "{request}"
        );
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer machine-token")
        );
        // NOTE: the one-answer hub is gone: a second prompt must not need it
        assert_eq!(materialize(&cfg, &text), out);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_file_the_hub_dropped_is_said_and_leaves_nothing_behind() {
        let (mut cfg, root) = cfg("dropped");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("token"), "t").unwrap();
        cfg.hub.token_file = Some(root.join("token"));
        let (url, served) = hub("404 Not Found", "", b"{\"error\":\"gone\"}");
        cfg.hub.url = url;
        let out = materialize(&cfg, &marker(ID));
        served.join().unwrap();
        assert!(out.contains("the hub no longer has it"), "{out}");
        assert!(!root.join(ID).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn the_sweep_takes_old_folders_it_made_and_nothing_else() {
        let (mut cfg, root) = cfg("sweep");
        let old = root.join(ID);
        let young = root.join("89abcdef0123456789abcdef");
        let foreign = root.join("notes");
        for d in [&old, &young, &foreign] {
            fs::create_dir_all(d).unwrap();
            fs::write(d.join("f"), b"x").unwrap();
        }
        let long_ago = SystemTime::now() - DAY * 30;
        for d in [&old, &foreign] {
            fs::File::open(d).unwrap().set_modified(long_ago).unwrap();
        }
        cfg.attachments.keep_days = 0;
        assert_eq!(sweep(&cfg), 0);
        cfg.attachments.keep_days = 7;
        assert_eq!(sweep(&cfg), 1);
        assert!(!old.exists() && young.exists() && foreign.exists());
        let _ = fs::remove_dir_all(root);
    }
}
