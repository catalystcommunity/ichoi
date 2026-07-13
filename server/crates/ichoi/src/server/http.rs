//! The HTTP surface (§4): serve the SPA assets and accept the WebSocket upgrade that carries
//! the CSIL surface. Nothing else — no REST, no media over HTTP. Plain HTTP by default;
//! browser TLS is proxy-fronted (§4.2).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tower_http::services::{ServeDir, ServeFile};

use crate::handlers::{App, Ctx, Identity};
use crate::server::media_http;
use crate::transport;

static CONN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct AppState {
    pub app: App,
}

pub fn router(app: App, web_dir: PathBuf) -> Router {
    let state = AppState { app };
    // Serve static files; for any unmatched path (a client-side route like /jukebox), fall
    // back to index.html so the SPA router handles it instead of 404ing (deep links, refresh).
    let index = web_dir.join("index.html");
    let assets = ServeDir::new(web_dir)
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(index));
    Router::new()
        .route("/healthz", get(healthz))
        .route("/status", get(status))
        .route("/ws", get(ws_upgrade))
        .route("/media/:track_id", get(media_http::stream_media))
        .route("/api/playlists/from-queue", post(save_queue_playlist))
        .fallback_service(assets)
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn status(State(s): State<AppState>) -> impl IntoResponse {
    let value = tokio::task::spawn_blocking(move || {
        let mut conn = match s.app.pool.get() {
            Ok(c) => c,
            Err(_) => return serde_json::json!({ "service": "ichoi", "status": "degraded" }),
        };
        serde_json::json!({
            "service": "ichoi",
            "status": "ok",
            "role": if matches!(s.app.config.role, crate::config::Role::Core) { "core" } else { "satellite" },
            "tracks": crate::db::store::count_tracks(&mut conn).unwrap_or(0),
            "albums": crate::db::store::count_albums(&mut conn).unwrap_or(0),
            "audio_outputs": crate::audio::state_label(),
        })
    })
    .await
    .unwrap_or_else(|_| serde_json::json!({ "service": "ichoi", "status": "degraded" }));
    Json(value)
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
    Json(req): Json<SaveQueuePlaylistRequest>,
) -> impl IntoResponse {
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

async fn ws_upgrade(ws: WebSocketUpgrade, State(s): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_conn(socket, s.app))
}

async fn ws_conn(mut socket: WebSocket, app: App) {
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
                            transport::handle_events_frame(&app2, id2, &bytes)
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
                        if let Some(frame) = reply {
                            if socket.send(Message::Binary(frame)).await.is_err() {
                                break;
                            }
                        }
                    }
                    // Debugging convenience: text JSON envelopes (guest identity).
                    Message::Text(text) => {
                        let app2 = app.clone();
                        let ctx = Ctx { identity: ident.clone() };
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
}
