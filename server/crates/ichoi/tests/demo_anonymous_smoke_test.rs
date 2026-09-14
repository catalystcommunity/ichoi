mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ichoi::db::{models, store};
use libichoi::csil::services::{AdminService, LibraryService, MediaService, PlayerService};
use libichoi::csil::types::*;
use tower::ServiceExt;

#[tokio::test]
async fn demo_guest_can_browse_and_stream_but_cannot_write_after_bootstrap() {
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
        store::upsert_account(
            &mut conn,
            &models::Account {
                id: "demo-admin@example.invalid".into(),
                handle: "demo-admin".into(),
                display_name: None,
                role: "admin".into(),
                created_at: "2026-09-12T00:00:00Z".into(),
            },
        )
        .unwrap();
    }

    let guest = common::ctx_anon();
    let albums = app
        .list_albums(
            &guest,
            BrowseRequest {
                library: None,
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(albums.total, 1);
    app.stream(
        &guest,
        MediaControl::Variant0(MediaOpen {
            kind: "open".into(),
            stream_id: "smoke-stream".into(),
            track_id: "track-1".into(),
            pref: StreamPref {
                max_bitrate_kbps: None,
                prefer_original: Some(true),
                transcode_codec: None,
            },
        }),
    )
    .unwrap();

    let playlist_response = ichoi::server::http::router(app.clone(), ".".into())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/playlists/from-queue")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"name": "Denied", "track_ids": ["track-1"]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(playlist_response.status(), StatusCode::UNAUTHORIZED);

    let export = app.export_manifest(
        &guest,
        ExportManifestRequest {
            track_id: "track-1".into(),
        },
    );
    assert_eq!(export.unwrap_err().code, 401);

    let import = app.import_track(
        &guest,
        ImportTrackRequest {
            library: None,
            root_relative_path: "denied.mp3".into(),
            content_type: "audio/mpeg".into(),
            content_hash: None,
            data: vec![0],
        },
    );
    assert_eq!(import.unwrap_err().code, 401);

    let share = app.enable_share(&guest, EnableShareRequest { suffix: None });
    assert_eq!(share.unwrap_err().code, 401);

    let progress = app.update_audiobook_progress(
        &guest,
        UpdateAudiobookProgressRequest {
            track_id: "track-1".into(),
            position_ms: 1,
            completed: false,
        },
    );
    assert_eq!(progress.unwrap_err().code, 401);
}
