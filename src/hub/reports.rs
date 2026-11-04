//! Reports: the HTML files behind `report` notes. A collector publishes
//! one at `PUT /reports/-/upload` (its machine's token) with the note's
//! metadata; the hub checks it is a self-contained document, keeps it as a
//! plain file under `reports.dir/<workspace>/<slug>.html` (outside the
//! database and its nightly copies) and upserts the note. The panel shows
//! a report through a short-lived view link (`/reports/-/view/<key>`, an
//! HMAC over workspace, slug and expiry with a key random per hub start)
//! in a sandboxed frame: the response's policy makes the document an
//! opaque origin that can run its own inline scripts and load nothing
//! from the network, so a report never sees the panel's token even when
//! opened on its own. The API downloads the file with the bearer token.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path as AxPath, State},
    http::{HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::put,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use ring::{
    digest, hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::info;

use super::{
    collect::Tokens,
    http::{ApiError, App},
};
use crate::{
    knowledge::{self, NoteKind, Status, Upsert, slugify, valid_slug},
    protocol::Who,
    report,
    sync::hub::{Host, collector_auth},
    util::now_ms,
};

/// How long a view link works.
pub const VIEW_MS: i64 = 3_600_000;
/// The frame's policy: its own inline scripts and styles, embedded images
/// and fonts, nothing fetched, and an opaque origin (`sandbox`) whether it
/// is framed or opened on its own.
const CSP: &str = "sandbox allow-scripts allow-popups allow-popups-to-escape-sandbox allow-downloads; \
    default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
    img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; \
    form-action 'none'; base-uri 'none'";

pub struct Reports {
    dir: PathBuf,
    max_bytes: usize,
    key: hmac::Key,
}

/// A report written beside its place, moved there once the note is in.
pub struct Pending {
    tmp: PathBuf,
    path: PathBuf,
}

impl Pending {
    pub fn commit(self) -> Result<()> {
        fs::rename(&self.tmp, &self.path)
            .with_context(|| format!("placing {}", self.path.display()))
    }

    pub fn discard(self) {
        let _ = fs::remove_file(&self.tmp);
    }
}

impl Reports {
    /// Opens `dir` (created private) and draws the view links' key.
    pub fn open(dir: &Path, max_mb: u32) -> Result<Self> {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        let mut seed = [0u8; 32];
        SystemRandom::new()
            .fill(&mut seed)
            .map_err(|_| anyhow!("no randomness for the view links"))?;
        Ok(Self {
            dir: dir.to_owned(),
            max_bytes: max_mb as usize * 1024 * 1024,
            key: hmac::Key::new(hmac::HMAC_SHA256, &seed),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The largest report taken (`reports.max_mb`).
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// The report's file; None for a workspace or slug that could leave
    /// the directory.
    pub fn path(&self, workspace: &str, slug: &str) -> Option<PathBuf> {
        (plain(workspace) && valid_slug(slug))
            .then(|| self.dir.join(workspace).join(format!("{slug}.html")))
    }

    pub fn read(&self, workspace: &str, slug: &str) -> Result<Option<Vec<u8>>> {
        let Some(path) = self.path(workspace, slug) else {
            return Ok(None);
        };
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Writes `html` beside the report's place; `commit` moves it there.
    pub fn stage(&self, workspace: &str, slug: &str, html: &[u8]) -> Result<Pending> {
        let path = self
            .path(workspace, slug)
            .ok_or_else(|| anyhow!("bad report name {workspace}/{slug}"))?;
        let folder = path.parent().context("no folder")?;
        fs::create_dir_all(folder)?;
        let tmp = folder.join(format!(".{slug}.{}.tmp", std::process::id()));
        fs::write(&tmp, html).with_context(|| format!("writing {}", tmp.display()))?;
        Ok(Pending { tmp, path })
    }

    /// A view link's key for the report, and when it expires.
    pub fn mint(&self, workspace: &str, slug: &str, now: i64) -> (String, i64) {
        let expires = now + VIEW_MS;
        let payload = format!("{expires}\n{workspace}\n{slug}");
        let tag = hmac::sign(&self.key, payload.as_bytes());
        (
            format!("{}.{}", B64.encode(payload), B64.encode(tag.as_ref())),
            expires,
        )
    }

    /// The report a key names, while the key is good.
    pub fn verify(&self, key: &str, now: i64) -> Option<(String, String)> {
        let (payload, tag) = key.split_once('.')?;
        let payload = B64.decode(payload).ok()?;
        let tag = B64.decode(tag).ok()?;
        hmac::verify(&self.key, &payload, &tag).ok()?;
        let payload = String::from_utf8(payload).ok()?;
        let mut parts = payload.splitn(3, '\n');
        let expires: i64 = parts.next()?.parse().ok()?;
        let workspace = parts.next()?;
        let slug = parts.next()?;
        (expires > now && plain(workspace) && valid_slug(slug))
            .then(|| (workspace.to_owned(), slug.to_owned()))
    }
}

/// A workspace name as a directory name: nothing that walks.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
        && name.len() <= 200
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `PUT /reports/-/upload`, behind the collectors' tokens; the body may be
/// about twice `reports.max_mb` (JSON around the HTML).
pub fn upload_routes(tokens: Tokens, max_bytes: usize) -> Router<App> {
    Router::new()
        .route("/reports/-/upload", put(upload))
        .layer(DefaultBodyLimit::max(max_bytes.saturating_mul(2) + 65_536))
        .layer(middleware::from_fn_with_state(tokens, collector_auth))
}

/// What a collector publishes: the session asking (`Who`, flattened), the
/// note and the document.
#[derive(Deserialize)]
pub(crate) struct UploadBody {
    #[serde(flatten)]
    who: Who,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    /// The abstract, markdown.
    body: String,
    #[serde(default)]
    project: Option<String>,
    /// Where the file was on the session's machine.
    #[serde(default)]
    path: Option<String>,
    html: String,
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// Takes a report from a collector: checks, the file, the note.
pub(crate) async fn upload(
    State(app): State<App>,
    Extension(Host(host)): Extension<Host>,
    Json(body): Json<UploadBody>,
) -> Result<Json<Value>, ApiError> {
    let refuse = |text: String| ApiError(StatusCode::UNPROCESSABLE_ENTITY, text);
    let now = now_ms();
    let bytes = body.html.len();
    if bytes > app.reports.max_bytes() {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "the report is {:.1} MB; the hub takes up to {} MB (reports.max_mb)",
                bytes as f64 / 1_048_576.0,
                app.reports.max_bytes() / 1_048_576
            ),
        ));
    }
    if !report::is_html(body.html.as_bytes()) {
        return Err(refuse(
            "not an HTML document: build the report with `ccdash report build`".into(),
        ));
    }
    let problems: Vec<String> = report::check(&body.html)
        .into_iter()
        .filter(|p| p.fatal)
        .map(|p| p.text)
        .collect();
    if !problems.is_empty() {
        return Err(refuse(format!(
            "the report is not self-contained, fix it and publish again:\n- {}",
            problems.join("\n- ")
        )));
    }
    let abstract_ = body.body.trim().to_owned();
    if abstract_.is_empty() {
        return Err(refuse(
            "give the abstract (body): what the report answers and found".into(),
        ));
    }
    let title = clean(body.title)
        .or_else(|| report::title_of(&body.html))
        .ok_or_else(|| refuse("give a title: the file has no <title>".into()))?;
    let located = app.hub.locate(&host, &body.who).await?.map_err(refuse)?;
    let workspace = located.workspace;
    let project = match body.project.as_deref().map(str::trim) {
        Some("") => None,
        Some(p) => {
            if !known_project(&app, &workspace, p).await? {
                return Err(refuse(format!(
                    "{p:?} is not a project of {workspace}: no such directory under the \
                        workspace on any host, and no notes under it"
                )));
            }
            Some(p.to_owned())
        }
        None => located.project,
    };
    let slug = match clean(body.slug) {
        Some(s) if !valid_slug(&s) => {
            return Err(refuse(format!(
                "{s:?} is not a slug (lowercase words joined with '-')"
            )));
        }
        Some(s) => s,
        None => slugify(&title),
    };
    let sha256 = hex(digest::digest(&digest::SHA256, body.html.as_bytes()).as_ref());
    let reports = app.reports.clone();
    let (ws, s, html) = (workspace.clone(), slug.clone(), body.html);
    let pending = tokio::task::spawn_blocking(move || reports.stage(&ws, &s, html.as_bytes()))
        .await
        .map_err(anyhow::Error::from)??;
    let up = Upsert {
        slug: slug.clone(),
        kind: NoteKind::Report,
        project,
        title,
        summary: clean(body.summary),
        status: Status::Active,
        extra: json!({
            "bytes": bytes,
            "sha256": sha256,
            "host": host,
            "path": body.path,
            "published_at": now,
        }),
        body: abstract_,
        links: Vec::new(),
        created_at: None,
        updated_at: None,
        verified: true,
    };
    let by = located.session;
    let ws = workspace.clone();
    let result = app
        .store
        .exec(
            move |conn| -> Result<(knowledge::Written, knowledge::Note)> {
                let written = knowledge::upsert(conn, &ws, &up, by.as_deref(), now)?;
                let note = knowledge::get(conn, &ws, &up.slug)?.context("note vanished")?;
                Ok((written, note))
            },
        )
        .await?;
    let (written, note) = match result {
        Ok(ok) => ok,
        Err(e) => {
            pending.discard();
            return Err(refuse(format!("{e:#}")));
        }
    };
    pending.commit()?;
    info!(
        "report {workspace}/{slug} from {host}: {} ({bytes} bytes)",
        if written.created { "new" } else { "replaced" }
    );
    app.hub.notes_changed(vec![note]).await?;
    Ok(Json(json!({
        "written": written,
        "workspace": workspace,
        "slug": slug,
        "bytes": bytes,
        "url": format!("{}/reports/{workspace}/{slug}", app.public_url),
    })))
}

/// Whether `project` is a directory under the workspace on some host, or
/// already has notes (what the knowledge tools accept).
async fn known_project(app: &App, workspace: &str, project: &str) -> Result<bool> {
    if app
        .hub
        .host_dirs()
        .await?
        .iter()
        .filter_map(|(_, dirs)| dirs.get(workspace))
        .any(|dirs| dirs.iter().any(|d| d == project))
    {
        return Ok(true);
    }
    let store = app.store.clone();
    let (ws, p) = (workspace.to_owned(), project.to_owned());
    tokio::task::spawn_blocking(move || knowledge::has_project(&store.reader()?, &ws, &p))
        .await
        .map_err(anyhow::Error::from)?
}

async fn report_note(app: &App, workspace: &str, slug: &str) -> Result<knowledge::Note, ApiError> {
    let store = app.store.clone();
    let (ws, s) = (workspace.to_owned(), slug.to_owned());
    let note = tokio::task::spawn_blocking(move || knowledge::get(&store.reader()?, &ws, &s))
        .await
        .map_err(anyhow::Error::from)??
        .ok_or(ApiError(StatusCode::NOT_FOUND, "no such report".into()))?;
    if note.kind != NoteKind::Report {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            format!("{slug} is a {}, not a report", note.kind),
        ));
    }
    Ok(note)
}

/// `POST /api/v1/reports/{ws}/{slug}/view`: a link the panel frames.
pub(crate) async fn mint(
    State(app): State<App>,
    AxPath((workspace, slug)): AxPath<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    report_note(&app, &workspace, &slug).await?;
    let (key, expires_at) = app.reports.mint(&workspace, &slug, now_ms());
    Ok(Json(json!({
        "url": format!("/reports/-/view/{key}"),
        "expires_at": expires_at,
    })))
}

async fn read(app: &App, workspace: String, slug: String) -> Result<Option<Vec<u8>>> {
    let reports = app.reports.clone();
    tokio::task::spawn_blocking(move || reports.read(&workspace, &slug))
        .await
        .map_err(anyhow::Error::from)?
}

/// `GET /reports/-/view/{key}`: the document, sandboxed.
pub(crate) async fn view(State(app): State<App>, AxPath(key): AxPath<String>) -> Response {
    let Some((workspace, slug)) = app.reports.verify(&key, now_ms()) else {
        return (
            StatusCode::NOT_FOUND,
            "this link has expired; open the report in ccdash again",
        )
            .into_response();
    };
    match read(&app, workspace, slug).await {
        Ok(Some(bytes)) => (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("private, no-store"),
                ),
                (
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static(CSP),
                ),
                (
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ),
                (
                    header::REFERRER_POLICY,
                    HeaderValue::from_static("no-referrer"),
                ),
            ],
            bytes,
        )
            .into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "the report's file is gone").into_response(),
        Err(e) => ApiError::from(e).into_response(),
    }
}

/// `GET /api/v1/reports/{ws}/{slug}/file`: the document to keep or send on.
pub(crate) async fn file(
    State(app): State<App>,
    AxPath((workspace, slug)): AxPath<(String, String)>,
) -> Result<Response, ApiError> {
    report_note(&app, &workspace, &slug).await?;
    let bytes = read(&app, workspace, slug.clone()).await?.ok_or(ApiError(
        StatusCode::NOT_FOUND,
        "the report's file is gone".into(),
    ))?;
    let name = format!("attachment; filename=\"{slug}.html\"");
    Ok((
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&name).map_err(anyhow::Error::from)?,
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            ),
        ],
        bytes,
    )
        .into_response())
}

/// `ccdash export`: the HTML next to the note's markdown, when it exists.
pub fn export_file(dir: &Path, workspace: &str, slug: &str) -> Option<Vec<u8>> {
    if !plain(workspace) || !valid_slug(slug) {
        return None;
    }
    fs::read(dir.join(workspace).join(format!("{slug}.html"))).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A private directory per call: the tests run in parallel and each
    /// removes its own at the end.
    fn store() -> (Reports, PathBuf) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ccdash-reports-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        (Reports::open(&dir, 1).unwrap(), dir)
    }

    #[test]
    fn files_stay_under_the_directory() {
        let (r, dir) = store();
        assert_eq!(
            r.path("tsandrini", "a-b.c"),
            Some(dir.join("tsandrini").join("a-b.c.html"))
        );
        assert!(r.path("../x", "a").is_none());
        assert!(r.path("ws", "../a").is_none());
        assert!(r.path("ws", "A").is_none());
        assert!(r.path(".hidden", "a").is_none());
        assert!(bail_on_bad(&r));
        let pending = r.stage("ws", "a", b"<html>1</html>").unwrap();
        assert!(r.read("ws", "a").unwrap().is_none());
        pending.commit().unwrap();
        assert_eq!(r.read("ws", "a").unwrap().unwrap(), b"<html>1</html>");
        let pending = r.stage("ws", "a", b"<html>2</html>").unwrap();
        pending.discard();
        assert_eq!(r.read("ws", "a").unwrap().unwrap(), b"<html>1</html>");
        assert_eq!(
            export_file(&dir, "ws", "a").unwrap(),
            b"<html>1</html>".to_vec()
        );
        assert!(export_file(&dir, "ws", "../a").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    fn bail_on_bad(r: &Reports) -> bool {
        r.stage("ws", "../a", b"x").is_err()
    }

    #[test]
    fn view_keys_name_one_report_for_an_hour() {
        let (r, dir) = store();
        let now = now_ms();
        let (key, expires) = r.mint("ws", "slug-1", now);
        assert_eq!(expires, now + VIEW_MS);
        assert!(!key.contains('/'));
        assert_eq!(
            r.verify(&key, now + VIEW_MS - 1),
            Some(("ws".into(), "slug-1".into()))
        );
        assert_eq!(r.verify(&key, now + VIEW_MS), None);
        // tampered, truncated, another hub's key
        let mut forged = key.clone();
        forged.replace_range(0..1, if key.starts_with('A') { "B" } else { "A" });
        assert_eq!(r.verify(&forged, now), None);
        assert_eq!(r.verify(&key[..key.len() - 2], now), None);
        assert_eq!(r.verify("nope", now), None);
        let (other, _) = store();
        assert_eq!(other.verify(&key, now), None);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(other.dir());
    }
}
