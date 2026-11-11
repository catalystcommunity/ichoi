mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use ichoi::{
    db::{models, store},
    scan,
};
use libichoi::csil::{services::LibraryService, types::*};
use tower::ServiceExt;

#[tokio::test]
async fn m4b_metadata_embedded_art_mp4_delivery_and_seeking_work_together() {
    check_m4b(include_bytes!("fixtures/m4b/TwoChapters.M4B"), "aac").await;
    check_m4b(include_bytes!("fixtures/m4b/TwoChaptersALAC.m4b"), "alac").await;
}

async fn check_m4b(bytes: &[u8], codec: &str) {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("TwoChapters.M4B"), bytes).unwrap();
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        store::upsert_library(
            &mut conn,
            &models::Library {
                id: "lib:audiobook".into(),
                kind: "audiobook".into(),
                path: temp.path().to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        let stats = scan::scan_library(
            &mut conn,
            "lib:audiobook",
            temp.path(),
            None,
            false,
            true,
            &["cd".into()],
        )
        .unwrap();
        assert_eq!(stats.tracks, 1);
        assert_eq!(stats.errors, 0);
    }
    let tracks = store::tracks_for_library(&mut pool.get().unwrap(), "lib:audiobook").unwrap();
    let track = &tracks[0];
    assert_eq!(track.codec, codec);
    assert_eq!(track.title, "Two Chapters");
    assert!((1900..2200).contains(&track.duration_ms));
    let detail = app
        .get_album(
            &common::ctx_anon(),
            AlbumRequest {
                album_id: track.album_id.clone().unwrap(),
            },
        )
        .unwrap();
    assert_eq!(detail.album.title, "The Test Audiobook");
    assert!(detail.album.has_cover_art);
    let art = app
        .get_cover_art(
            &common::ctx_anon(),
            CoverArtRequest {
                album_id: detail.album.id,
                max_size: Some(128),
            },
        )
        .unwrap();
    assert!(image::load_from_memory(&art.data).is_ok());
    let router = ichoi::server::http::router(app, temp.path().into());
    let uri = format!("/media/{}", track.id);
    let full = router
        .clone()
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(full.headers()["content-type"], "audio/mp4");
    assert_eq!(
        full.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
        bytes
    );
    for (range, start, end) in [
        ("bytes=0-31", 0, 31),
        ("bytes=128-255", 128, 255),
        ("bytes=-16", bytes.len() - 16, bytes.len() - 1),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&uri)
                    .header("Range", range)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .as_ref(),
            &bytes[start..=end]
        );
    }
    let response = router
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("Range", "bytes=999999999-")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
}
