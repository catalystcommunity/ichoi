mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use ichoi::db::{models, store};
use tower::ServiceExt;

fn app_with_music() -> (
    ichoi::handlers::App,
    ichoi::db::SqlitePool,
    tempfile::TempDir,
) {
    let (mut app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    let mut config = common::test_config();
    config.music_dir = Some(music.path().to_owned());
    app.config = Arc::new(config);
    {
        let mut conn = pool.get().unwrap();
        common::create_artist(&mut conn, &common::DataMap::new());
        common::create_album(&mut conn, &common::DataMap::new());
        common::create_track(&mut conn, &common::DataMap::new());
    }
    (app, pool, music)
}

fn seed_account(pool: &ichoi::db::SqlitePool, id: &str, role: &str, token: Option<&str>) {
    let mut conn = pool.get().unwrap();
    store::upsert_account(
        &mut conn,
        &models::Account {
            id: id.into(),
            handle: id.split('@').next().unwrap().into(),
            display_name: None,
            role: role.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        },
    )
    .unwrap();
    if let Some(token) = token {
        store::create_session(
            &mut conn,
            &ichoi::auth::sha256_hex(token),
            id,
            "2099-01-01T00:00:00Z",
        )
        .unwrap();
    }
}

async fn save(
    app: ichoi::handlers::App,
    cookie: Option<&str>,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/playlists/from-queue")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = ichoi::server::http::router(app, ".".into())
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, body)
}

#[tokio::test]
async fn signed_in_member_owns_private_playlist_and_false_owner_is_ignored() {
    let (app, pool, _music) = app_with_music();
    seed_account(&pool, "member@example.com", "member", Some("valid-token"));

    let (status, body) = save(
        app,
        Some("__Host-ichoi_session=valid-token"),
        serde_json::json!({
            "name": "Private queue",
            "visibility": "private",
            "owner": "attacker@example.com",
            "track_ids": ["track-1"]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let mut conn = pool.get().unwrap();
    let playlist = store::get_playlist(&mut conn, body["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(playlist.owner.as_deref(), Some("member@example.com"));
    assert_eq!(playlist.visibility, "private");
}

#[tokio::test]
async fn zero_account_guest_can_save_only_a_public_playlist() {
    let (app, pool, _music) = app_with_music();
    let (status, body) = save(
        app,
        None,
        serde_json::json!({
            "name": "Guest queue",
            "visibility": "private",
            "track_ids": ["track-1"]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let mut conn = pool.get().unwrap();
    let playlist = store::get_playlist(&mut conn, body["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert!(playlist.owner.is_none());
    assert_eq!(playlist.visibility, "public");
}

#[tokio::test]
async fn guest_and_invalid_session_cannot_save_after_first_account() {
    for cookie in [None, Some("__Host-ichoi_session=invalid-token")] {
        let (app, pool, _music) = app_with_music();
        seed_account(&pool, "admin@example.com", "admin", None);
        let (status, _) = save(
            app,
            cookie,
            serde_json::json!({"name": "Denied", "track_ids": ["track-1"]}),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(store::list_playlists(&mut pool.get().unwrap())
            .unwrap()
            .is_empty());
    }
}
