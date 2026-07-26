mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn reported_version(web_dir: std::path::PathBuf) -> String {
    let (app, _pool) = common::test_app();
    let response = ichoi::server::http::router(app, web_dir)
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status["service"], "ichoi");
    status["version"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn status_exposes_release_version_without_cache() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    std::fs::write(first.path().join("index.html"), "first bundle").unwrap();
    std::fs::write(second.path().join("index.html"), "second bundle").unwrap();

    let first_version = reported_version(first.path().to_owned()).await;
    let second_version = reported_version(second.path().to_owned()).await;
    assert!(first_version.starts_with(&format!("{}+web.", ichoi::version())));
    assert_ne!(first_version, second_version);
}
