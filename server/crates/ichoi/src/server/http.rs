//! The HTTP surface (§4): serve the SPA assets and accept the WebSocket upgrade that carries
//! the CSIL surface. Nothing else — no REST, no media over HTTP. Plain HTTP by default;
//! browser TLS is proxy-fronted (§4.2).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Query, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tower_http::services::{ServeDir, ServeFile};

use crate::handlers::{App, Ctx, Identity};
use crate::server::media_http;
use crate::transport;

static CONN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct AppState {
    pub app: App,
    pub release_version: String,
}

pub fn router(app: App, web_dir: PathBuf) -> Router {
    let local_rp_enabled = app.config.linkkeys_local_rp;
    let regular_rp_enabled = app.config.linkkeys_rp;
    let state = AppState {
        app,
        release_version: release_version(&web_dir),
    };
    // Serve static files; for any unmatched path (a client-side route like /jukebox), fall
    // back to index.html so the SPA router handles it instead of 404ing (deep links, refresh).
    let index = web_dir.join("index.html");
    let assets = ServeDir::new(web_dir)
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(index));
    let router = Router::new()
        .route("/healthz", get(healthz))
        .route("/status", get(status))
        .route("/api/auth", get(auth_status))
        .route(
            "/api/session",
            post(set_session_cookie).delete(clear_session_cookie),
        )
        .route("/ws", get(ws_upgrade))
        .route("/media/:track_id", get(media_http::stream_media))
        .route("/api/playlists/from-queue", post(save_queue_playlist))
        .fallback_service(assets);
    let router = if local_rp_enabled {
        router
            .route("/auth/linkkeys/local/start", post(local_rp_start))
            .route("/auth/linkkeys/local/callback", get(local_rp_callback))
    } else {
        router
    };
    let router = if regular_rp_enabled {
        router
            .route("/auth/linkkeys/start", post(regular_rp_start))
            .route("/auth/linkkeys/callback", get(regular_rp_callback))
    } else {
        router
    };
    router.with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

fn release_version(web_dir: &std::path::Path) -> String {
    let package = env!("CARGO_PKG_VERSION");
    let mut files = walkdir::WalkDir::new(web_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() {
        return package.to_owned();
    }
    let mut hasher = Sha256::new();
    for path in files {
        if let Ok(relative) = path.strip_prefix(web_dir) {
            hasher.update(relative.as_os_str().as_encoded_bytes());
        }
        let Ok(contents) = std::fs::read(path) else {
            return package.to_owned();
        };
        hasher.update(contents);
    }
    let digest = hasher.finalize();
    format!("{package}+web.{}", hex::encode(&digest[..8]))
}

async fn status(State(s): State<AppState>) -> impl IntoResponse {
    let release_version = s.release_version.clone();
    let fallback_version = release_version.clone();
    let value = tokio::task::spawn_blocking(move || {
        let mut conn = match s.app.pool.get() {
            Ok(c) => c,
            Err(_) => return serde_json::json!({
                "service": "ichoi",
                "status": "degraded",
                "version": release_version,
            }),
        };
        serde_json::json!({
            "service": "ichoi",
            "status": "ok",
            "version": release_version,
            "role": if matches!(s.app.config.role, crate::config::Role::Core) { "core" } else { "satellite" },
            "tracks": crate::db::store::count_tracks(&mut conn).unwrap_or(0),
            "albums": crate::db::store::count_all_albums(&mut conn).unwrap_or(0),
            "audio_outputs": crate::audio::state_label(),
        })
    })
    .await
    .unwrap_or_else(|_| serde_json::json!({
        "service": "ichoi",
        "status": "degraded",
        "version": fallback_version,
    }));
    ([(header::CACHE_CONTROL, "no-store")], Json(value))
}

fn client_address(
    app: &App,
    headers: &HeaderMap,
    connect: Option<ConnectInfo<SocketAddr>>,
) -> IpAddr {
    let peer = connect
        .map(|ConnectInfo(address)| address.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    if !app.config.trusts_proxy(peer) {
        return peer;
    }
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        // The trusted immediate proxy appends the address it observed. Reading
        // from the right prevents a client-supplied leading value from turning
        // a public request into a LAN guest.
        .and_then(|value| value.split(',').next_back())
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(peer)
}

fn access_mode_name(mode: crate::config::AccessMode) -> &'static str {
    match mode {
        crate::config::AccessMode::Open => "open",
        crate::config::AccessMode::LanGuests => "lan-guests",
        crate::config::AccessMode::LoginRequired => "login-required",
    }
}

async fn auth_status(
    State(s): State<AppState>,
    headers: HeaderMap,
    connect: Option<ConnectInfo<SocketAddr>>,
) -> impl IntoResponse {
    let guest_allowed = s
        .app
        .config
        .guest_allowed_from(client_address(&s.app, &headers, connect));
    Json(serde_json::json!({
        "access_mode": access_mode_name(s.app.config.access_mode),
        "guest_allowed": guest_allowed,
        "local_rp": s.app.local_rp.as_ref().map(|backend| serde_json::json!({
            "name": s.app.config.linkkeys_local_rp_name,
            "fingerprint": backend.fingerprint(),
            "start_url": "/auth/linkkeys/local/start",
        })),
        "regular_rp": s.app.regular_rp.as_ref().map(|_| serde_json::json!({
            "domain": s.app.config.linkkeys_rp_domain,
            "start_url": "/auth/linkkeys/start",
        })),
    }))
}

async fn set_session_cookie(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let Some(token) = token else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let mut conn = match s.app.pool.get() {
        Ok(conn) => conn,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match crate::db::store::account_for_token(&mut conn, &crate::auth::sha256_hex(token)) {
        Ok(Some(_)) => (
            StatusCode::NO_CONTENT,
            [(
                header::SET_COOKIE,
                "__Host-ichoi_session=".to_string()
                    + token
                    + "; Path=/; Max-Age=2592000; HttpOnly; Secure; SameSite=Strict",
            )],
        )
            .into_response(),
        Ok(None) => StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn clear_session_cookie() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [(
            header::SET_COOKIE,
            "__Host-ichoi_session=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict",
        )],
    )
}

pub(crate) fn request_has_session(app: &App, headers: &HeaderMap) -> bool {
    let token = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                cookie
                    .trim()
                    .strip_prefix("__Host-ichoi_session=")
                    .filter(|token| !token.is_empty())
            })
        });
    let Some(token) = token else {
        return false;
    };
    app.pool
        .get()
        .ok()
        .and_then(|mut conn| {
            crate::db::store::account_for_token(&mut conn, &crate::auth::sha256_hex(token))
                .ok()
                .flatten()
        })
        .is_some()
}

pub(crate) fn request_allowed(
    app: &App,
    headers: &HeaderMap,
    connect: Option<ConnectInfo<SocketAddr>>,
) -> bool {
    request_has_session(app, headers)
        || app
            .config
            .guest_allowed_from(client_address(app, headers, connect))
}

#[derive(Debug, Deserialize)]
struct LocalRpStartRequest {
    identity: String,
}

#[derive(Debug, Serialize)]
struct LocalRpStartResponse {
    redirect_url: String,
}

async fn local_rp_start(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LocalRpStartRequest>,
) -> impl IntoResponse {
    let Some(backend) = s.app.local_rp.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let selector = match crate::auth::local_rp::parse_selector(&req.identity) {
        Ok(value) => value,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let origin = match validated_origin(&headers) {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let mut conn = match s.app.pool.get() {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match crate::db::store::linkkeys_identity_is_trusted(
        &mut conn,
        &selector.domain,
        selector.handle.as_deref(),
    ) {
        Ok(true) => {}
        Ok(false) => {
            return (StatusCode::FORBIDDEN, "LinkKeys identity is not trusted").into_response()
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    drop(conn);

    let attempt = crate::auth::mint_token();
    let callback_url = format!(
        "{origin}/auth/linkkeys/local/callback?attempt={}",
        attempt.token
    );
    let domain = selector.domain.clone();
    let callback = callback_url.clone();
    let begun = tokio::task::spawn_blocking(move || backend.begin(&domain, &callback)).await;
    let (redirect_url, pending_login) = match begun {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let now = chrono::Utc::now();
    let row = crate::db::models::LinkkeysLoginAttempt {
        attempt_sha256: attempt.sha256_hex,
        pending_login,
        expected_handle: selector.handle,
        created_at: now.to_rfc3339(),
        expires_at: (now + chrono::Duration::minutes(5)).to_rfc3339(),
    };
    let mut conn = match s.app.pool.get() {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if crate::db::store::purge_expired_linkkeys_state(&mut conn, &now.to_rfc3339()).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if crate::db::store::create_linkkeys_attempt(&mut conn, &row).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    (StatusCode::OK, Json(LocalRpStartResponse { redirect_url })).into_response()
}

#[derive(Debug, Deserialize)]
struct LocalRpCallbackQuery {
    attempt: String,
    encrypted_token: String,
}

async fn local_rp_callback(
    State(s): State<AppState>,
    Query(query): Query<LocalRpCallbackQuery>,
) -> impl IntoResponse {
    let Some(backend) = s.app.local_rp.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut conn = match s.app.pool.get() {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let attempt = match crate::db::store::consume_linkkeys_attempt(
        &mut conn,
        &crate::auth::sha256_hex(&query.attempt),
    ) {
        Ok(Some(value)) if !crate::auth::local_rp::is_expired(&value.expires_at) => value,
        Ok(_) => {
            return (StatusCode::UNAUTHORIZED, "invalid or expired login attempt").into_response()
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    drop(conn);
    let callback_url =
        serde_json::from_str::<linkkeys_local_rp::PendingLogin>(&attempt.pending_login)
            .map(|pending| pending.callback_url);
    let callback_url = match callback_url {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let arrived_url = format!("{callback_url}&encrypted_token={}", query.encrypted_token);
    let pending = attempt.pending_login.clone();
    let encrypted = query.encrypted_token.clone();
    let completed =
        tokio::task::spawn_blocking(move || backend.complete(&pending, &encrypted, &arrived_url))
            .await;
    let verified = match completed {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => return (StatusCode::UNAUTHORIZED, e.to_string()).into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    finish_linkkeys_login(&s.app, verified)
}

async fn regular_rp_start(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LocalRpStartRequest>,
) -> Response {
    let Some(backend) = s.app.regular_rp.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let selector = match crate::auth::local_rp::parse_selector(&req.identity) {
        Ok(value) => value,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    if let Err(error) = validated_origin(&headers) {
        return error.into_response();
    }
    let mut conn = match s.app.pool.get() {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match crate::db::store::linkkeys_identity_is_trusted(
        &mut conn,
        &selector.domain,
        selector.handle.as_deref(),
    ) {
        Ok(true) => {}
        Ok(false) => {
            return (StatusCode::FORBIDDEN, "LinkKeys identity is not trusted").into_response()
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    drop(conn);

    let attempt = crate::auth::mint_token();
    let public_url = s
        .app
        .config
        .public_url
        .as_deref()
        .expect("validated regular RP public URL")
        .trim_end_matches('/');
    let callback_url = format!(
        "{public_url}/auth/linkkeys/callback?attempt={}",
        attempt.token
    );
    let domain = selector.domain.clone();
    let handle = selector.handle.clone();
    let callback = callback_url.clone();
    let begun =
        tokio::task::spawn_blocking(move || backend.begin(&domain, handle.as_deref(), &callback))
            .await;
    let (redirect_url, pending_login) = match begun {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let now = chrono::Utc::now();
    let row = crate::db::models::LinkkeysLoginAttempt {
        attempt_sha256: attempt.sha256_hex,
        pending_login,
        expected_handle: selector.handle,
        created_at: now.to_rfc3339(),
        expires_at: (now + chrono::Duration::minutes(5)).to_rfc3339(),
    };
    let mut conn = match s.app.pool.get() {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if crate::db::store::purge_expired_linkkeys_state(&mut conn, &now.to_rfc3339()).is_err()
        || crate::db::store::create_linkkeys_attempt(&mut conn, &row).is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    (StatusCode::OK, Json(LocalRpStartResponse { redirect_url })).into_response()
}

async fn regular_rp_callback(
    State(s): State<AppState>,
    Query(query): Query<LocalRpCallbackQuery>,
) -> Response {
    let Some(backend) = s.app.regular_rp.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut conn = match s.app.pool.get() {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let attempt = match crate::db::store::consume_linkkeys_attempt(
        &mut conn,
        &crate::auth::sha256_hex(&query.attempt),
    ) {
        Ok(Some(value)) if !crate::auth::local_rp::is_expired(&value.expires_at) => value,
        Ok(_) => {
            return (StatusCode::UNAUTHORIZED, "invalid or expired login attempt").into_response()
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    drop(conn);

    let pending = attempt.pending_login.clone();
    let encrypted = query.encrypted_token.clone();
    let completed =
        tokio::task::spawn_blocking(move || backend.complete(&pending, &encrypted)).await;
    let verified = match completed {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => return (StatusCode::UNAUTHORIZED, e.to_string()).into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    finish_linkkeys_login(&s.app, verified)
}

fn finish_linkkeys_login(app: &App, verified: crate::auth::local_rp::VerifiedIdentity) -> Response {
    let mut conn = match app.pool.get() {
        Ok(value) => value,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    // The prefix entered at login is an IDP user hint, not an authorization
    // fact. A LinkKeys username may legitimately differ from its public
    // `handle` claim (for example `tod` versus `todpunk`). Admission below is
    // therefore based only on the cryptographically verified domain/handle.
    match crate::db::store::linkkeys_identity_is_trusted(
        &mut conn,
        &verified.domain,
        Some(&verified.handle),
    ) {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::FORBIDDEN,
                "verified LinkKeys identity is not trusted",
            )
                .into_response()
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let account_id = format!("{}@{}", verified.user_id, verified.domain);
    let first_account = match crate::db::store::count_accounts(&mut conn) {
        Ok(count) => count == 0,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if crate::db::store::upsert_linkkeys_account(
        &mut conn,
        &account_id,
        &verified.handle,
        verified.display_name.as_deref(),
    )
    .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if first_account && crate::db::store::set_role(&mut conn, &account_id, "admin").is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let exchange = crate::auth::mint_token();
    let now = chrono::Utc::now();
    let row = crate::db::models::LinkkeysLoginExchange {
        code_sha256: exchange.sha256_hex,
        account_id,
        created_at: now.to_rfc3339(),
        expires_at: (now + chrono::Duration::minutes(2)).to_rfc3339(),
    };
    if crate::db::store::create_linkkeys_exchange(&mut conn, &row).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    Redirect::to(&format!("/#linkkeys_exchange={}", exchange.token)).into_response()
}

fn validated_origin(headers: &HeaderMap) -> Result<String, (StatusCode, &'static str)> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .ok_or((StatusCode::FORBIDDEN, "same-origin login required"))?;
    let uri: Uri = origin
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid Origin header"))?;
    if !matches!(uri.scheme_str(), Some("http") | Some("https")) || uri.path() != "/" {
        return Err((StatusCode::BAD_REQUEST, "invalid Origin header"));
    }
    let origin_authority = uri
        .authority()
        .map(|value| value.as_str())
        .ok_or((StatusCode::BAD_REQUEST, "invalid Origin header"))?;
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "missing Host header"))?;
    if origin_authority != host {
        return Err((StatusCode::FORBIDDEN, "Origin does not match Host"));
    }
    Ok(origin.trim_end_matches('/').to_string())
}

#[derive(Debug, Deserialize)]
struct SaveQueuePlaylistRequest {
    name: String,
    track_ids: Vec<String>,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    owner: Option<String>,
}

#[derive(Debug, Serialize)]
struct SaveQueuePlaylistResponse {
    id: String,
    name: String,
    root_relative_path: String,
    visibility: String,
    entry_count: usize,
}

async fn save_queue_playlist(
    State(s): State<AppState>,
    headers: HeaderMap,
    connect: Option<ConnectInfo<SocketAddr>>,
    Json(req): Json<SaveQueuePlaylistRequest>,
) -> impl IntoResponse {
    if !request_allowed(&s.app, &headers, connect) {
        return (StatusCode::UNAUTHORIZED, "sign in required").into_response();
    }
    let result = tokio::task::spawn_blocking(move || save_queue_playlist_sync(&s.app, req)).await;
    match result {
        Ok(Ok(saved)) => (StatusCode::OK, Json(saved)).into_response(),
        Ok(Err((status, message))) => (status, message).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("internal: {e}")).into_response(),
    }
}

fn save_queue_playlist_sync(
    app: &App,
    req: SaveQueuePlaylistRequest,
) -> Result<SaveQueuePlaylistResponse, (StatusCode, String)> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "playlist name is required".to_string(),
        ));
    }
    if req.track_ids.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "queue is empty".to_string()));
    }
    let root = app.config.music_dir.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "no music directory configured".to_string(),
        )
    })?;
    let owner = req.owner.filter(|s| !s.trim().is_empty());
    let visibility = if req.visibility.as_deref() == Some("private") && owner.is_some() {
        "private"
    } else {
        "public"
    }
    .to_string();

    let mut conn = app
        .pool
        .get()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("internal: {e}")))?;
    let mut entries = Vec::new();
    for id in &req.track_ids {
        let track = crate::db::store::get_track(&mut conn, id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("internal: {e}")))?
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("track not found: {id}")))?;
        entries.push(track.root_relative_path);
    }

    let dir = root.join("playlists");
    std::fs::create_dir_all(&dir).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("creating playlists dir: {e}"),
        )
    })?;
    let filename = unique_playlist_filename(&dir, name);
    let root_relative_path = format!("playlists/{filename}");
    let full_path = root.join(&root_relative_path);
    std::fs::write(&full_path, libichoi::m3u::write(&entries)).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("writing playlist: {e}"),
        )
    })?;

    let id = format!("playlist:{}", root_relative_path);
    crate::db::store::upsert_playlist(
        &mut conn,
        &crate::db::models::Playlist {
            id: id.clone(),
            name: name.to_string(),
            owner,
            root_relative_path: root_relative_path.clone(),
            visibility: visibility.clone(),
        },
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("internal: {e}")))?;

    Ok(SaveQueuePlaylistResponse {
        id,
        name: name.to_string(),
        root_relative_path,
        visibility,
        entry_count: entries.len(),
    })
}

fn unique_playlist_filename(dir: &std::path::Path, name: &str) -> String {
    let slug = playlist_slug(name);
    let mut candidate = format!("{slug}.m3u");
    let mut n = 2;
    while dir.join(&candidate).exists() {
        candidate = format!("{slug}-{n}.m3u");
        n += 1;
    }
    candidate
}

fn playlist_slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == '-' || c == '_' || c.is_whitespace() {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect();
    let collapsed = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "queue".to_string()
    } else {
        collapsed
    }
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(s): State<AppState>,
    headers: HeaderMap,
    connect: Option<ConnectInfo<SocketAddr>>,
) -> impl IntoResponse {
    let allow_guest = s
        .app
        .config
        .guest_allowed_from(client_address(&s.app, &headers, connect));
    ws.on_upgrade(move |socket| ws_conn(socket, s.app, allow_guest))
}

async fn ws_conn(mut socket: WebSocket, app: App, allow_guest: bool) {
    // Connection identity, resolved from the `$hello` auth token (login-less → guest).
    let mut ident = Identity::Anonymous;
    let conn_id = CONN_ID.fetch_add(1, Ordering::Relaxed);
    // Outbound channel for server-pushed frames (live player-state fan-out).
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    // Periodic ping keeps the socket alive through NAT/idle timeouts (the browser answers
    // automatically); without it a backgrounded phone would silently drop the connection.
    let mut keepalive = tokio::time::interval(Duration::from_secs(20));
    keepalive.tick().await;

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(msg)) = incoming else { break };
                match msg {
                    Message::Binary(bytes) => {
                        let app2 = app.clone();
                        let id2 = ident.clone();
                        let (new_ident, reply, effects) = tokio::task::spawn_blocking(move || {
                            transport::handle_events_frame(&app2, id2, allow_guest, &bytes)
                        })
                        .await
                        .unwrap_or((Identity::Anonymous, None, transport::FrameEffects::default()));
                        ident = new_ident;
                        if let Some(player_id) = effects.subscribe {
                            app.subs.subscribe(player_id, conn_id, tx.clone());
                        }
                        if let Some(player_id) = effects.attach {
                            // This connection is now the device's speaker; it shows up as a live
                            // device and, via subscribe, drives its audio.
                            app.presence.attach(player_id, conn_id);
                        }
                        if let Some(player_id) = effects.node_session {
                            app.nodes.subscribe(player_id, conn_id, tx.clone());
                        }
                        if let Some(frame) = reply {
                            if socket.send(Message::Binary(frame)).await.is_err() {
                                break;
                            }
                        }
                    }
                    // Debugging convenience: text JSON envelopes (guest identity).
                    Message::Text(text) => {
                        let app2 = app.clone();
                        let ctx = Ctx {
                            identity: ident.clone(),
                            allow_guest,
                        };
                        let reply = tokio::task::spawn_blocking(move || {
                            transport::handle_json(&app2, &ctx, &text)
                        })
                        .await
                        .unwrap_or_else(|_| "{\"id\":0,\"status\":500}".to_string());
                        if socket.send(Message::Text(reply)).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            Some(frame) = rx.recv() => {
                if socket.send(Message::Binary(frame)).await.is_err() {
                    break;
                }
            }
            _ = keepalive.tick() => {
                if socket.send(Message::Ping(Vec::new())).await.is_err() {
                    break;
                }
            }
        }
    }
    app.subs.unsubscribe_conn(conn_id);
    app.presence.detach_conn(conn_id);
    app.nodes.unsubscribe_conn(conn_id);
}
