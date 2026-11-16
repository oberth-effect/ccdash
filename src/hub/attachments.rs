//! Attachments on the hub: the files behind the markers in prompts (see
//! `crate::attachments`). The panel uploads one at `POST
//! /api/v1/attachments` (the body is the file, streamed to disk; its kind
//! comes from its first bytes); it is kept as a plain file under
//! `attachments.dir/<id>` (outside the database and its nightly copies)
//! with a row beside it. A collector fetches it with its machine's token
//! (`GET /attachments/-/fetch/<id>`) when a prompt naming it is delivered;
//! the panel shows or plays it through a short-lived link
//! (`/attachments/-/view/<key>`, an HMAC over the id and the expiry with a
//! key random per hub start), served so that nothing in a file can act in
//! the panel's origin. Caps per kind, a quota and a retention keep the
//! store bounded (`[attachments]`).

use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use axum::{
    Extension, Json, Router,
    body::{Body, Bytes},
    extract::{Path as AxPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use futures_util::StreamExt;
use ring::{
    digest, hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, warn};

use super::{
    collect::Tokens,
    http::{ApiError, App},
};
use crate::{
    attachments::{self as rows, Attachment, ID_LEN, Kind},
    config::HubAttachments,
    store::Store,
    sync::hub::{Host, collector_auth},
    util::now_ms,
};

/// How long a view link works.
const VIEW_MS: i64 = 6 * 3_600_000;
/// How often the retention looks.
const SWEEP_EVERY: std::time::Duration = std::time::Duration::from_secs(3600);
const DAY_MS: i64 = 86_400_000;
const MIB: u64 = 1024 * 1024;
/// Bytes read from a file per piece of a response.
const PIECE: usize = 256 * 1024;
/// Bytes of a file its kind is read from.
const HEAD: usize = 4096;
/// NOTE: `sandbox` makes whatever is served an opaque origin, so a file
/// opened on its own cannot reach the panel's storage or its token.
const CSP: &str = "sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:; \
    media-src 'self'";

pub struct Attachments {
    dir: PathBuf,
    cfg: HubAttachments,
    key: hmac::Key,
}

impl Attachments {
    /// Opens `dir` (created private) and draws the view links' key.
    pub fn open(dir: &Path, cfg: HubAttachments) -> Result<Self> {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        let mut seed = [0u8; 32];
        SystemRandom::new()
            .fill(&mut seed)
            .map_err(|_| anyhow!("no randomness for the view links"))?;
        Ok(Self {
            dir: dir.to_owned(),
            cfg,
            key: hmac::Key::new(hmac::HMAC_SHA256, &seed),
        })
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        rows::valid_id(id).then(|| self.dir.join(id))
    }

    /// The largest file of `kind` taken, in bytes.
    fn cap(&self, kind: Kind) -> u64 {
        u64::from(match kind {
            Kind::Image => self.cfg.max_image_mb,
            Kind::Audio => self.cfg.max_audio_mb,
            Kind::Video => self.cfg.max_video_mb,
            Kind::File => self.cfg.max_file_mb,
        }) * MIB
    }

    fn quota(&self) -> u64 {
        self.cfg.quota_mb.saturating_mul(MIB)
    }

    fn retention_ms(&self) -> Option<i64> {
        (self.cfg.retention_days > 0).then(|| i64::from(self.cfg.retention_days) * DAY_MS)
    }

    /// What the panel is told about the store's bounds.
    fn limits(&self) -> Value {
        json!({
            "max_image_mb": self.cfg.max_image_mb,
            "max_audio_mb": self.cfg.max_audio_mb,
            "max_video_mb": self.cfg.max_video_mb,
            "max_file_mb": self.cfg.max_file_mb,
            "quota_mb": self.cfg.quota_mb,
            "retention_days": self.cfg.retention_days,
        })
    }

    /// The row as the panel gets it: with its view link and its end.
    fn shown(&self, mut a: Attachment, now: i64) -> Attachment {
        a.expires_at = match (a.pinned, self.retention_ms()) {
            (false, Some(keep)) => Some(a.created_at + keep),
            _ => None,
        };
        a.url = Some(format!("/attachments/-/view/{}", self.mint(&a.id, now)));
        a
    }

    fn mint(&self, id: &str, now: i64) -> String {
        let payload = format!("{}\n{id}", now + VIEW_MS);
        let tag = hmac::sign(&self.key, payload.as_bytes());
        format!("{}.{}", B64.encode(payload), B64.encode(tag.as_ref()))
    }

    /// The attachment a key names, while the key is good.
    fn verify(&self, key: &str, now: i64) -> Option<String> {
        let (payload, tag) = key.split_once('.')?;
        let payload = B64.decode(payload).ok()?;
        let tag = B64.decode(tag).ok()?;
        hmac::verify(&self.key, &payload, &tag).ok()?;
        let payload = String::from_utf8(payload).ok()?;
        let (expires, id) = payload.split_once('\n')?;
        let expires: i64 = expires.parse().ok()?;
        (expires > now && rows::valid_id(id)).then(|| id.to_owned())
    }

    fn remove_file(&self, id: &str) {
        if let Some(path) = self.path(id)
            && let Err(e) = fs::remove_file(&path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            warn!("attachments: removing {}: {e}", path.display());
        }
    }
}

fn new_id() -> Result<String> {
    let mut raw = [0u8; ID_LEN / 2];
    SystemRandom::new()
        .fill(&mut raw)
        .map_err(|_| anyhow!("no randomness for an attachment id"))?;
    Ok(hex(&raw))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / MIB as f64)
}

/// The store, or 404 when `[attachments] enable` is off.
fn store_of(app: &App) -> Result<&Attachments, ApiError> {
    app.attachments.as_deref().ok_or(ApiError(
        StatusCode::NOT_FOUND,
        "attachments are off on this hub ([attachments] enable)".into(),
    ))
}

fn gone() -> ApiError {
    ApiError(
        StatusCode::NOT_FOUND,
        "no such attachment: it was removed, or its time on the hub ran out".into(),
    )
}

/// Drops rows and files: what is too old, and what the quota (with
/// `incoming` more bytes) has no room for. Returns how many went.
async fn evict(store: &Store, files: &Attachments, incoming: u64, now: i64) -> Result<usize> {
    let before = files.retention_ms().map(|keep| now - keep);
    let quota = files.quota();
    let ids = store
        .exec(move |conn| {
            let ids = rows::to_evict(conn, before, quota, incoming)?;
            for id in &ids {
                rows::remove(conn, id)?;
            }
            anyhow::Ok(ids)
        })
        .await??;
    for id in &ids {
        files.remove_file(id);
    }
    Ok(ids.len())
}

/// The hourly look at the retention; files with no row (a crash between
/// the two) go too.
pub fn spawn_sweep(store: Store, files: std::sync::Arc<Attachments>) {
    tokio::spawn(async move {
        loop {
            match evict(&store, &files, 0, now_ms()).await {
                Ok(0) => {}
                Ok(n) => info!("attachments: dropped {n} past their time or the quota"),
                Err(e) => warn!("attachments: retention: {e:#}"),
            }
            if let Err(e) = orphans(&store, &files).await {
                warn!("attachments: looking for stray files: {e:#}");
            }
            tokio::time::sleep(SWEEP_EVERY).await;
        }
    });
}

async fn orphans(store: &Store, files: &Attachments) -> Result<()> {
    let reader = store.clone();
    let known: std::collections::HashSet<String> =
        tokio::task::spawn_blocking(move || -> Result<_> {
            Ok(rows::list(&reader.reader()?, None)?
                .into_iter()
                .map(|a| a.id)
                .collect())
        })
        .await??;
    let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for entry in fs::read_dir(&files.dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let id = name.strip_suffix(".part").unwrap_or(&name);
        // NOTE: an upload in progress is a young `.part` with no row yet
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|t| t < hour_ago);
        if old && !known.contains(id) {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct UploadQuery {
    /// The file's name on the device.
    #[serde(default)]
    name: String,
    /// The session whose composer it was attached in.
    #[serde(default)]
    session: Option<String>,
}

/// What the writer thread found: size, digest, first bytes.
struct Written {
    bytes: u64,
    sha256: String,
    head: Vec<u8>,
}

/// Writes the pieces to `tmp` off the runtime, hashing as it goes.
fn writer(tmp: PathBuf, mut rx: mpsc::Receiver<Bytes>) -> Result<Written> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("creating {}", tmp.display()))?;
    let mut ctx = digest::Context::new(&digest::SHA256);
    let mut head = Vec::with_capacity(HEAD);
    let mut bytes = 0u64;
    while let Some(piece) = rx.blocking_recv() {
        if head.len() < HEAD {
            let take = (HEAD - head.len()).min(piece.len());
            head.extend_from_slice(&piece[..take]);
        }
        ctx.update(&piece);
        file.write_all(&piece)?;
        bytes += piece.len() as u64;
    }
    file.sync_all()?;
    Ok(Written {
        bytes,
        sha256: hex(ctx.finish().as_ref()),
        head,
    })
}

/// `POST /attachments?name=&session=`: the body is the file.
pub(crate) async fn upload(
    State(app): State<App>,
    Query(query): Query<UploadQuery>,
    body: Body,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let files = store_of(&app)?;
    let now = now_ms();
    let id = new_id()?;
    let path = files.path(&id).context("a fresh id is valid")?;
    let tmp = files.dir.join(format!("{id}.part"));
    let limit = u64::from(files.cfg.max_mb()) * MIB;

    let (tx, rx) = mpsc::channel::<Bytes>(8);
    let write = tokio::task::spawn_blocking({
        let tmp = tmp.clone();
        move || writer(tmp, rx)
    });
    let mut stream = body.into_data_stream();
    let mut taken = 0u64;
    let mut failed: Option<ApiError> = None;
    while let Some(piece) = stream.next().await {
        let piece = match piece {
            Ok(piece) => piece,
            Err(e) => {
                failed = Some(ApiError(
                    StatusCode::BAD_REQUEST,
                    format!("the upload broke off: {e}"),
                ));
                break;
            }
        };
        taken += piece.len() as u64;
        if taken > limit {
            failed = Some(ApiError(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "the file is over {} MB, the most this hub takes ([attachments] max_*_mb)",
                    files.cfg.max_mb()
                ),
            ));
            break;
        }
        if tx.send(piece).await.is_err() {
            break;
        }
    }
    drop(tx);
    let written = write.await.map_err(anyhow::Error::from)?;
    let discard = |e: ApiError| {
        let _ = fs::remove_file(&tmp);
        e
    };
    if let Some(e) = failed {
        return Err(discard(e));
    }
    let written = written.map_err(|e| discard(e.into()))?;
    if written.bytes == 0 {
        return Err(discard(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "the file is empty".into(),
        )));
    }
    let (kind, mime) = rows::sniff(&written.head);
    if written.bytes > files.cap(kind) {
        return Err(discard(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "the {} is {}; this hub takes up to {} MB of that kind ([attachments] max_{}_mb)",
                kind.as_str(),
                mb(written.bytes),
                files.cap(kind) / MIB,
                kind.as_str()
            ),
        )));
    }
    let quota = files.quota();
    if quota > 0 && written.bytes > quota {
        return Err(discard(ApiError(
            StatusCode::INSUFFICIENT_STORAGE,
            format!(
                "the file is larger than the whole store ([attachments] quota_mb = {})",
                files.cfg.quota_mb
            ),
        )));
    }
    let session = match query.session.as_deref().filter(|s| !s.is_empty()) {
        Some(key) => app.hub.resolve(key).await.map_err(|e| discard(e.into()))?,
        None => None,
    };
    evict(&app.store, files, written.bytes, now)
        .await
        .map_err(|e| discard(e.into()))?;
    let row = Attachment {
        id: id.clone(),
        name: rows::safe_name(&query.name),
        kind,
        mime: mime.to_owned(),
        bytes: written.bytes,
        sha256: written.sha256,
        workspace: session.as_ref().and_then(|s| s.workspace.clone()),
        session: session.map(|s| s.id),
        created_at: now,
        fetched_at: None,
        pinned: false,
        expires_at: None,
        url: None,
    };
    let (kept, total) = app
        .store
        .exec({
            let row = row.clone();
            move |conn| {
                rows::insert(conn, &row)?;
                anyhow::Ok((row, rows::total_bytes(conn)?))
            }
        })
        .await
        .map_err(|e| discard(e.into()))?
        .map_err(|e: anyhow::Error| discard(e.into()))?;
    if quota > 0 && total > quota {
        // NOTE: everything else is pinned, so nothing could make room
        let _ = app
            .store
            .exec({
                let id = id.clone();
                move |conn| rows::remove(conn, &id)
            })
            .await;
        return Err(discard(ApiError(
            StatusCode::INSUFFICIENT_STORAGE,
            format!(
                "the store is full of pinned attachments ({} of {} MB): remove or unpin some in Files",
                mb(total - kept.bytes),
                files.cfg.quota_mb
            ),
        )));
    }
    fs::rename(&tmp, &path)
        .with_context(|| format!("placing {}", path.display()))
        .map_err(|e| discard(e.into()))?;
    info!(
        "attachments: {} ({}, {}) kept as {id}",
        kept.name,
        kept.mime,
        mb(kept.bytes)
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({ "attachment": files.shown(kept, now) })),
    ))
}

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    #[serde(default)]
    session: Option<String>,
}

/// `GET /attachments[?session=]`: the rows, the bytes kept and the bounds.
pub(crate) async fn list(
    State(app): State<App>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let files = store_of(&app)?;
    let session = match query.session.filter(|s| !s.is_empty()) {
        Some(key) => Some(app.hub.resolve(&key).await?.map_or(key, |s| s.id)),
        None => None,
    };
    let store = app.store.clone();
    let (all, total) = tokio::task::spawn_blocking(move || -> Result<_> {
        let conn = store.reader()?;
        Ok((
            rows::list(&conn, session.as_deref())?,
            rows::total_bytes(&conn)?,
        ))
    })
    .await
    .map_err(anyhow::Error::from)??;
    let now = now_ms();
    let shown: Vec<Attachment> = all.into_iter().map(|a| files.shown(a, now)).collect();
    Ok(Json(json!({
        "attachments": shown,
        "total_bytes": total,
        "limits": files.limits(),
    })))
}

async fn row_of(app: &App, id: &str) -> Result<Attachment, ApiError> {
    if !rows::valid_id(id) {
        return Err(gone());
    }
    let (store, id) = (app.store.clone(), id.to_owned());
    tokio::task::spawn_blocking(move || rows::get(&store.reader()?, &id))
        .await
        .map_err(anyhow::Error::from)??
        .ok_or_else(gone)
}

/// `GET /attachments/<id>`: one row, with a fresh view link.
pub(crate) async fn one(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, ApiError> {
    let files = store_of(&app)?;
    let row = row_of(&app, &id).await?;
    Ok(Json(json!({ "attachment": files.shown(row, now_ms()) })))
}

#[derive(Deserialize)]
pub(crate) struct Patch {
    pinned: bool,
}

/// `PATCH /attachments/<id>`: pin it, or let the retention have it again.
pub(crate) async fn patch(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
    Json(body): Json<Patch>,
) -> Result<Json<Value>, ApiError> {
    let files = store_of(&app)?;
    row_of(&app, &id).await?;
    let key = id.clone();
    app.store
        .exec(move |conn| rows::set_pinned(conn, &key, body.pinned))
        .await??;
    let row = row_of(&app, &id).await?;
    Ok(Json(json!({ "attachment": files.shown(row, now_ms()) })))
}

/// `DELETE /attachments/<id>`: the row and the file.
pub(crate) async fn remove(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, ApiError> {
    let files = store_of(&app)?;
    row_of(&app, &id).await?;
    let key = id.clone();
    app.store
        .exec(move |conn| rows::remove(conn, &key))
        .await??;
    files.remove_file(&id);
    Ok(Json(json!({ "removed": id })))
}

/// `first-last` of a `Range: bytes=…` header within `len` bytes; None
/// for no header or one this does not serve (several ranges).
fn range_of(headers: &HeaderMap, len: u64) -> Option<(u64, u64)> {
    let spec = headers
        .get(header::RANGE)?
        .to_str()
        .ok()?
        .strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (first, last) = spec.split_once('-')?;
    let (first, last) = match (first.trim(), last.trim()) {
        ("", suffix) => (len.saturating_sub(suffix.parse::<u64>().ok()?), len - 1),
        (first, "") => (first.parse().ok()?, len - 1),
        (first, last) => (first.parse().ok()?, last.parse::<u64>().ok()?.min(len - 1)),
    };
    (first <= last && first < len).then_some((first, last))
}

/// The file's bytes `first..=last` as a streamed body, read off the runtime.
fn stream(path: PathBuf, first: u64, last: u64) -> Body {
    let (tx, rx) = mpsc::channel::<std::io::Result<Bytes>>(4);
    tokio::task::spawn_blocking(move || {
        let mut left = last + 1 - first;
        let opened = fs::File::open(&path).and_then(|mut f| {
            f.seek(SeekFrom::Start(first))?;
            Ok(f)
        });
        let mut file = match opened {
            Ok(file) => file,
            Err(e) => {
                let _ = tx.blocking_send(Err(e));
                return;
            }
        };
        while left > 0 {
            let mut piece = vec![0u8; PIECE.min(left as usize)];
            match file.read(&mut piece) {
                Ok(0) => break,
                Ok(n) => {
                    piece.truncate(n);
                    left -= n as u64;
                    if tx.blocking_send(Ok(Bytes::from(piece))).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.blocking_send(Err(e));
                    break;
                }
            }
        }
    });
    Body::from_stream(ReceiverStream::new(rx))
}

/// The file as a response: a range when asked for one, shown in place
/// only when it is a picture, a recording or a PDF, a download otherwise.
fn serve(files: &Attachments, row: &Attachment, headers: &HeaderMap) -> Response {
    let Some(path) = files.path(&row.id).filter(|p| p.is_file()) else {
        return gone().into_response();
    };
    let inline = matches!(row.kind, Kind::Image | Kind::Audio | Kind::Video)
        || row.mime == "application/pdf";
    let disposition = format!(
        "{}; filename=\"{}\"",
        if inline { "inline" } else { "attachment" },
        row.name
            .chars()
            .map(|c| if c.is_ascii() && c != '"' { c } else { '_' })
            .collect::<String>()
    );
    let (status, first, last) = match range_of(headers, row.bytes) {
        Some((first, last)) => (StatusCode::PARTIAL_CONTENT, first, last),
        None => (StatusCode::OK, 0, row.bytes.saturating_sub(1)),
    };
    let mut out = Response::new(stream(path, first, last));
    *out.status_mut() = status;
    let h = out.headers_mut();
    let set = |h: &mut HeaderMap, name, value: &str| {
        if let Ok(value) = HeaderValue::from_str(value) {
            h.insert(name, value);
        }
    };
    set(h, header::CONTENT_TYPE, &row.mime);
    set(h, header::CONTENT_LENGTH, &(last + 1 - first).to_string());
    set(h, header::CONTENT_DISPOSITION, &disposition);
    set(h, header::ACCEPT_RANGES, "bytes");
    set(h, header::CACHE_CONTROL, "private, max-age=3600");
    set(h, header::CONTENT_SECURITY_POLICY, CSP);
    set(h, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(h, header::REFERRER_POLICY, "no-referrer");
    if status == StatusCode::PARTIAL_CONTENT {
        set(
            h,
            header::CONTENT_RANGE,
            &format!("bytes {first}-{last}/{}", row.bytes),
        );
    }
    out
}

/// `GET /attachments/-/view/<key>`: the file through a view link.
pub(crate) async fn view(
    State(app): State<App>,
    AxPath(key): AxPath<String>,
    headers: HeaderMap,
) -> Response {
    let Ok(files) = store_of(&app) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(id) = files.verify(&key, now_ms()) else {
        return (
            StatusCode::NOT_FOUND,
            "this link has expired; open the attachment in ccdash again",
        )
            .into_response();
    };
    match row_of(&app, &id).await {
        Ok(row) => serve(files, &row, &headers),
        Err(e) => e.into_response(),
    }
}

/// `GET /attachments/-/fetch/<id>`, behind the collectors' tokens: the
/// file for a session on that machine, its name and kind in headers.
pub(crate) async fn fetch(
    State(app): State<App>,
    Extension(Host(host)): Extension<Host>,
    AxPath(id): AxPath<String>,
    headers: HeaderMap,
) -> Response {
    let Ok(files) = store_of(&app) else {
        return gone().into_response();
    };
    let row = match row_of(&app, &id).await {
        Ok(row) => row,
        Err(e) => return e.into_response(),
    };
    let key = id.clone();
    if let Err(e) = app
        .store
        .exec(move |conn| rows::mark_fetched(conn, &key, now_ms()))
        .await
    {
        warn!("attachments: noting the fetch of {id}: {e:#}");
    }
    info!("attachments: {id} ({}) fetched by {host}", row.name);
    let mut out = serve(files, &row, &headers);
    let h = out.headers_mut();
    if let Ok(name) = HeaderValue::from_str(&B64.encode(&row.name)) {
        h.insert("x-ccdash-name", name);
    }
    h.insert("x-ccdash-kind", HeaderValue::from_static(row.kind.as_str()));
    out
}

/// The routes outside `/api/v1`: view links (the key is the credential)
/// and the collectors' fetch.
pub fn routes(tokens: Tokens) -> Router<App> {
    Router::new()
        .route("/attachments/-/fetch/{id}", get(fetch))
        .layer(middleware::from_fn_with_state(tokens, collector_auth))
        .route("/attachments/-/view/{key}", get(view))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(cfg: HubAttachments) -> (Attachments, PathBuf) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ccdash-att-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        (Attachments::open(&dir, cfg).unwrap(), dir)
    }

    #[test]
    fn view_links_name_one_attachment_for_a_while() {
        let (files, dir) = store(HubAttachments::default());
        let id = "0123456789abcdef01234567";
        let key = files.mint(id, 1000);
        assert_eq!(files.verify(&key, 1000).as_deref(), Some(id));
        assert_eq!(files.verify(&key, 1000 + VIEW_MS), None);
        let (payload, tag) = key.split_once('.').unwrap();
        let other = B64.encode(format!("{}\n{}", 1000 + VIEW_MS, "f".repeat(ID_LEN)));
        assert_eq!(files.verify(&format!("{other}.{tag}"), 1000), None);
        assert_eq!(files.verify(payload, 1000), None);
        let (again, dir2) = store(HubAttachments::default());
        assert_eq!(again.verify(&key, 1000), None);
        assert_eq!(files.path("../../etc/passwd"), None);
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir_all(dir2);
    }

    #[test]
    fn ranges_are_single_and_inside_the_file() {
        let h = |v: &str| {
            let mut h = HeaderMap::new();
            h.insert(header::RANGE, HeaderValue::from_str(v).unwrap());
            h
        };
        assert_eq!(range_of(&HeaderMap::new(), 100), None);
        assert_eq!(range_of(&h("bytes=0-"), 100), Some((0, 99)));
        assert_eq!(range_of(&h("bytes=10-19"), 100), Some((10, 19)));
        assert_eq!(range_of(&h("bytes=90-500"), 100), Some((90, 99)));
        assert_eq!(range_of(&h("bytes=-10"), 100), Some((90, 99)));
        assert_eq!(range_of(&h("bytes=100-"), 100), None);
        assert_eq!(range_of(&h("bytes=5-2"), 100), None);
        assert_eq!(range_of(&h("bytes=0-1,5-6"), 100), None);
        assert_eq!(range_of(&h("items=0-1"), 100), None);
        assert_eq!(range_of(&h("bytes=0-"), 0), None);
    }

    #[test]
    fn caps_follow_the_kind_and_the_writer_hashes() {
        let (files, dir) = store(HubAttachments {
            max_image_mb: 1,
            max_video_mb: 9,
            ..HubAttachments::default()
        });
        assert_eq!(files.cap(Kind::Image), MIB);
        assert_eq!(files.cap(Kind::Video), 9 * MIB);
        assert_eq!(files.cfg.max_mb(), 50);

        let (tx, rx) = mpsc::channel(4);
        tx.blocking_send(Bytes::from_static(b"\xff\xd8\xff"))
            .unwrap();
        tx.blocking_send(Bytes::from_static(b"rest")).unwrap();
        drop(tx);
        let tmp = dir.join("x.part");
        let written = writer(tmp.clone(), rx).unwrap();
        assert_eq!(written.bytes, 7);
        assert_eq!(written.head, b"\xff\xd8\xffrest");
        assert_eq!(
            written.sha256,
            hex(digest::digest(&digest::SHA256, b"\xff\xd8\xffrest").as_ref())
        );
        assert_eq!(
            fs::metadata(&tmp).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // NOTE: an id is drawn fresh, so a second upload never lands on the first
        let (_tx, rx) = mpsc::channel(1);
        assert!(writer(tmp, rx).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
