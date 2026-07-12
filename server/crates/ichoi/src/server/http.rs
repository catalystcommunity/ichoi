//! The HTTP surface (§4): serve the SPA assets and accept the WebSocket upgrade that carries
//! the CSIL surface. Nothing else — no REST, no media over HTTP. Plain HTTP by default;
//! browser TLS is proxy-fronted (§4.2).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
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
