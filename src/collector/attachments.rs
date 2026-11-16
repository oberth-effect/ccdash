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
//!
//! Claude cannot hear or watch: a recording gets its words in a file
//! (`[transcribe] command`, which the panel's dictation uses too) and a
//! video still frames cut evenly across it (`[video] ffmpeg`) plus its
//! words, under `<id>/derived/`, made once and named in lines after the
//! file's own.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use tracing::{info, warn};

use crate::{
    attachments::{self as shared, Derived, Kind},
    config::CollectorConfig,
    fsx::{self, ModeExt},
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
    format!("Read(/{}/**)", rule_path(&dir(cfg).to_string_lossy()))
}

/// `dir` as Claude Code's permission rules match it: a Windows path in
/// POSIX form (`C:\Users\me` is `/c/Users/me`); any other as it is.
///
/// <https://code.claude.com/docs/en/permissions>
fn rule_path(dir: &str) -> String {
    let slashed = crate::workspace::slashed(dir);
    match slashed.as_bytes() {
        [drive, b':', b'/', ..] if drive.is_ascii_alphabetic() => {
            format!(
                "/{}{}",
                char::from(drive.to_ascii_lowercase()),
                &slashed[2..]
            )
        }
        _ => slashed.into_owned(),
    }
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
        Ok((kind, path)) => {
            let mut line = shared::delivered(kind, &path.to_string_lossy());
            if matches!(kind, Kind::Audio | Kind::Video) {
                line.push_str(&shared::derived(id, kind, &derive(cfg, kind, &path)));
            }
            line
        }
        Err(e) => {
            warn!("attachment {id}: {e:#}");
            shared::undelivered(id, &format!("{e:#}"))
        }
    })
}

/// What is said in attachment `id` (a recording), for the panel's
/// dictation. Blocking: it may fetch, and it runs the command.
pub fn transcribe(cfg: &CollectorConfig, id: &str) -> Result<String, String> {
    if !shared::valid_id(id) {
        return Err(format!("{id:?} is not an attachment"));
    }
    if cfg.transcribe.command.is_empty() {
        return Err("this machine puts no recordings into words (`[transcribe] command`)".into());
    }
    let (_, path) = local(cfg, &dir(cfg), id).map_err(|e| format!("{e:#}"))?;
    let text = words(cfg, &path)?;
    if text.is_empty() {
        return Err("nothing was heard in the recording".into());
    }
    Ok(text)
}

/// Runs `[transcribe] command` on `file`; its standard output, tidied.
fn words(cfg: &CollectorConfig, file: &Path) -> Result<String, String> {
    let (program, args) = cfg
        .transcribe
        .command
        .split_first()
        .ok_or("`[transcribe] command` is empty")?;
    let path = file.to_string_lossy();
    let args: Vec<String> = args.iter().map(|a| a.replace("{file}", &path)).collect();
    let out = run(
        program,
        &args,
        Duration::from_secs(cfg.transcribe.timeout_secs.max(1)),
    )?;
    // NOTE: whisper prints a line per stretch of speech, each led by a space
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Runs a command to its end or `timeout`, whichever is first; its
/// standard output, or why it failed (the end of its standard error).
fn run(program: &str, args: &[String], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(crate::util::program(program))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program} did not start: {e}"))?;
    // NOTE: read off this thread, or a full pipe would stall the child
    let pipe = |mut from: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut all = Vec::new();
            let _ = from.read_to_end(&mut all);
            String::from_utf8_lossy(&all).into_owned()
        })
    };
    let stdout = pipe(Box::new(child.stdout.take().expect("piped")));
    let stderr = pipe(Box::new(child.stderr.take().expect("piped")));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                // NOTE: a `.cmd` shim is `cmd.exe` starting the program
                #[cfg(windows)]
                crate::procs::end_group(child.id(), true);
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{program} was stopped after {} s",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("waiting for {program}: {e}")),
        }
    };
    let (out, err) = (
        stdout.join().unwrap_or_default(),
        stderr.join().unwrap_or_default(),
    );
    if status.success() {
        return Ok(out);
    }
    let last = err.lines().rev().find(|l| !l.trim().is_empty());
    Err(format!(
        "{program} failed ({status}){}",
        last.map(|l| format!(": {}", crate::util::truncate(l.trim(), 200)))
            .unwrap_or_default()
    ))
}

/// What this machine makes of a recording or a video for Claude: made on
/// the first delivery, read back after.
fn derive(cfg: &CollectorConfig, kind: Kind, file: &Path) -> Derived {
    let mut made = Derived::default();
    let Some(folder) = file.parent().map(|f| f.join("derived")) else {
        return made;
    };
    if kind == Kind::Video && !cfg.video.ffmpeg.is_empty() {
        match frames(cfg, file, &folder.join("frames")) {
            Ok(found) => made.frames = Some(found),
            Err(e) => {
                warn!("frames of {}: {e}", file.display());
                made.notes
                    .push(format!("no frames could be cut from it ({e})"));
            }
        }
    }
    if cfg.transcribe.attachments && !cfg.transcribe.command.is_empty() {
        let path = folder.join("transcript.txt");
        let known = fs::read_to_string(&path).ok();
        let text = match known {
            Some(text) => Ok(text),
            None => words(cfg, file).inspect(|text| {
                let _ = fs::create_dir_all(&folder);
                let _ = write_private(&path, text.as_bytes());
            }),
        };
        match text {
            Ok(text) if text.trim().is_empty() => {
                made.notes.push("nothing is said in it".into());
            }
            Ok(_) => made.transcript = Some(path.to_string_lossy().into_owned()),
            Err(e) => {
                warn!("words of {}: {e}", file.display());
                made.notes
                    .push(format!("what is said in it could not be made out ({e})"));
            }
        }
    }
    made
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?
        .write_all(bytes)
}

/// Still frames of `video` in `into`, spread evenly over it: the
/// directory, how many, the seconds between two. Kept once cut (`info`
/// beside them holds the count and the step).
fn frames(cfg: &CollectorConfig, video: &Path, into: &Path) -> Result<(String, u32, f64), String> {
    let shown = into.to_string_lossy().into_owned();
    let info = into.join("info");
    if let Some((count, step)) = fs::read_to_string(&info).ok().and_then(|text| {
        let (count, step) = text.trim().split_once(' ')?;
        Some((count.parse().ok()?, step.parse().ok()?))
    }) {
        return Ok((shown, count, step));
    }
    let ffmpeg = &cfg.video.ffmpeg;
    let ffprobe = Path::new(ffmpeg)
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map_or_else(|| "ffprobe".into(), |dir| dir.join("ffprobe"))
        .to_string_lossy()
        .into_owned();
    let timeout = Duration::from_secs(cfg.video.timeout_secs.max(1));
    let file = video.to_string_lossy().into_owned();
    let probe: Vec<String> = [
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
        &file,
    ]
    .map(str::to_owned)
    .to_vec();
    let seconds: f64 = run(&ffprobe, &probe, timeout)?
        .trim()
        .parse()
        .ok()
        .filter(|s: &f64| s.is_finite() && *s > 0.0)
        .ok_or("its length is unknown")?;
    // NOTE: a clip of a few seconds has no dozen frames worth reading
    let count = cfg
        .video
        .frames
        .clamp(1, 60)
        .min((seconds * 2.0).ceil().max(1.0) as u32);
    let step = seconds / f64::from(count);
    let edge = cfg.video.edge.max(64);
    let _ = fs::remove_dir_all(into);
    fs::create_dir_all(into).map_err(|e| format!("creating {}: {e}", into.display()))?;
    let args: Vec<String> = [
        "-v".to_owned(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        file,
        "-vf".into(),
        format!(
            "fps={count}/{seconds},scale=w={edge}:h={edge}:force_original_aspect_ratio=decrease:force_divisible_by=2"
        ),
        "-frames:v".into(),
        count.to_string(),
        "-q:v".into(),
        "4".into(),
        into.join("%03d.jpg").to_string_lossy().into_owned(),
    ]
    .to_vec();
    if let Err(e) = run(ffmpeg, &args, timeout) {
        let _ = fs::remove_dir_all(into);
        return Err(e);
    }
    let cut = fs::read_dir(into)
        .map(|d| {
            d.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "jpg"))
                .count() as u32
        })
        .unwrap_or(0);
    if cut == 0 {
        let _ = fs::remove_dir_all(into);
        return Err("it has no picture".into());
    }
    let _ = write_private(&info, format!("{cut} {step}").as_bytes());
    Ok((shown, cut, step))
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
        fsx::set_mode(root, 0o700)?;
    }
    fsx::set_mode(folder, 0o700)?;
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
        #[cfg(unix)]
        assert_eq!(read_rule(&cfg), format!("Read(/{}/**)", root.display()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn the_read_rule_names_a_windows_folder_as_claude_code_matches_it() {
        assert_eq!(
            rule_path(r"C:\Users\me\AppData\Local\ccdash\state\attachments"),
            "/c/Users/me/AppData/Local/ccdash/state/attachments"
        );
        assert_eq!(rule_path("d:/files"), "/d/files");
        assert_eq!(
            rule_path("/home/me/.local/state/ccdash"),
            "/home/me/.local/state/ccdash"
        );
        assert_eq!(rule_path(r"/home/me/a\b"), r"/home/me/a\b");
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(root.join(ID)).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
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

    #[cfg(unix)]
    fn sh(script: &str) -> Vec<String> {
        ["sh", "-c", script, "sh", "{file}"]
            .map(str::to_owned)
            .to_vec()
    }

    /// Unix: `sh`.
    #[cfg(unix)]
    #[test]
    fn commands_end_with_their_output_their_error_or_the_clock() {
        let args = |script: &str| vec!["-c".to_owned(), script.to_owned()];
        let second = Duration::from_secs(5);
        assert_eq!(
            run("sh", &args("printf hi; echo noise >&2"), second).unwrap(),
            "hi"
        );
        let failed = run(
            "sh",
            &args("echo first >&2; echo 'no model' >&2; exit 3"),
            second,
        );
        assert!(
            failed.as_ref().unwrap_err().ends_with(": no model"),
            "{failed:?}"
        );
        let slow = run("sh", &args("sleep 30"), Duration::from_millis(200)).unwrap_err();
        assert!(slow.contains("was stopped after"), "{slow}");
        let missing = run("/nonexistent/whisper", &[], second).unwrap_err();
        assert!(missing.contains("did not start"), "{missing}");
        // NOTE: more than a pipe holds: the reader threads keep the child going
        let big = run("sh", &args("head -c 300000 /dev/zero | tr '\\0' x"), second).unwrap();
        assert_eq!(big.len(), 300_000);
    }

    #[cfg(windows)]
    #[test]
    fn a_stopped_batch_file_ends_with_what_it_started() {
        let dir = std::env::temp_dir().join(format!("ccdash-catt-{}-cmd", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let shim = dir.join("slow.cmd");
        // NOTE: cmd.exe runs the shim and waits for ping, as for an npm-installed tool
        fs::write(&shim, "@ping -n 33 127.0.0.4 >NUL\r\n").unwrap();
        let slow = run(&shim.to_string_lossy(), &[], Duration::from_millis(500)).unwrap_err();
        assert!(slow.contains("was stopped after"), "{slow}");
        let pinging = || {
            crate::win::processes().into_iter().any(|p| {
                crate::win::command_line(p.pid).is_some_and(|line| line.contains("127.0.0.4"))
            })
        };
        let gone = (0..50).any(|_| {
            std::thread::sleep(Duration::from_millis(40));
            !pinging()
        });
        assert!(gone, "ping outlived the timeout");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Unix: `sh`.
    #[cfg(unix)]
    #[test]
    fn a_recording_is_put_into_words_once_and_named_in_the_prompt() {
        let (mut cfg, root) = cfg("words");
        let folder = root.join(ID);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("memo.wav"), b"RIFF\0\0\0\0WAVEfmt ").unwrap();
        let count = root.join("runs");
        cfg.transcribe.command = sh(&format!(
            "echo x >> {}; printf '\\n Check the second board.\\n  It reads too high. \\n' ; test -f \"$1\"",
            count.display()
        ));
        // dictation: the words alone
        assert_eq!(
            transcribe(&cfg, ID).unwrap(),
            "Check the second board.\nIt reads too high."
        );
        // as an attachment: the words in a file beside it, made once
        let text = format!("listen\n{}", marker(ID));
        let out = materialize(&cfg, &text);
        let transcript = folder.join("derived/transcript.txt");
        assert_eq!(
            out,
            format!(
                "listen\n[Attached audio recording, which you cannot hear: {}]\n[Transcript of \
                {ID}: what is said in it is in {}]",
                folder.join("memo.wav").display(),
                transcript.display()
            )
        );
        assert_eq!(
            fs::read_to_string(&transcript).unwrap(),
            "Check the second board.\nIt reads too high."
        );
        assert_eq!(materialize(&cfg, &text), out);
        assert_eq!(fs::read_to_string(&count).unwrap().lines().count(), 2);
        // NOTE: the derived files do not hide the recording from the cache
        assert_eq!(cached(&folder).unwrap(), folder.join("memo.wav"));

        cfg.transcribe.attachments = false;
        assert!(materialize(&cfg, &text).contains("no transcript was made"));
        let _ = fs::remove_dir_all(root);
    }

    /// Unix: `sh`.
    #[cfg(unix)]
    #[test]
    fn what_cannot_be_heard_is_said_not_hidden() {
        let (mut cfg, root) = cfg("unheard");
        let folder = root.join(ID);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("memo.wav"), b"RIFF\0\0\0\0WAVEfmt ").unwrap();
        assert!(
            transcribe(&cfg, ID)
                .unwrap_err()
                .contains("[transcribe] command")
        );
        assert!(
            transcribe(&cfg, "../x")
                .unwrap_err()
                .contains("not an attachment")
        );
        cfg.transcribe.command = sh("echo 'model missing' >&2; exit 1");
        assert!(
            transcribe(&cfg, ID)
                .unwrap_err()
                .ends_with(": model missing")
        );
        let out = materialize(&cfg, &marker(ID));
        assert!(
            out.contains("what is said in it could not be made out"),
            "{out}"
        );
        assert!(out.contains("model missing"), "{out}");
        cfg.transcribe.command = sh("true");
        assert_eq!(
            transcribe(&cfg, ID).unwrap_err(),
            "nothing was heard in the recording"
        );
        assert!(materialize(&cfg, &marker(ID)).contains("nothing is said in it"));
        // a video with no ffmpeg named: said as well
        fs::remove_dir_all(&folder).unwrap();
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("clip.mp4"), b"\0\0\0\x18ftypmp42").unwrap();
        cfg.transcribe.command = Vec::new();
        let out = materialize(&cfg, &marker(ID));
        assert!(out.contains("which you cannot watch"), "{out}");
        assert!(out.contains("no frames or transcript were made"), "{out}");
        cfg.video.ffmpeg = "/nonexistent/ffmpeg".into();
        assert!(materialize(&cfg, &marker(ID)).contains("no frames could be cut from it"));
        let _ = fs::remove_dir_all(root);
    }

    /// With real tools: `CCDASH_TEST_FFMPEG=/…/bin/ffmpeg CCDASH_TEST_VIDEO=clip.mp4
    /// cargo test real_frames -- --ignored`.
    #[test]
    #[ignore = "needs ffmpeg and a video named in the environment"]
    fn real_frames_are_cut_evenly_and_kept() {
        let (mut cfg, root) = cfg("frames");
        cfg.video.ffmpeg = std::env::var("CCDASH_TEST_FFMPEG").unwrap();
        cfg.video.frames = 5;
        let folder = root.join(ID);
        fs::create_dir_all(&folder).unwrap();
        fs::copy(
            std::env::var("CCDASH_TEST_VIDEO").unwrap(),
            folder.join("clip.mp4"),
        )
        .unwrap();
        let out = materialize(&cfg, &marker(ID));
        println!("{out}");
        let frames = folder.join("derived/frames");
        assert!(out.contains(&format!("[Frames of {ID}: 5 stills")), "{out}");
        for n in 1..=5 {
            assert!(frames.join(format!("{n:03}.jpg")).is_file());
        }
        assert!(!frames.join("006.jpg").exists());
        let stamp = fs::metadata(frames.join("001.jpg"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(materialize(&cfg, &marker(ID)), out);
        assert_eq!(
            fs::metadata(frames.join("001.jpg"))
                .unwrap()
                .modified()
                .unwrap(),
            stamp
        );
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
