mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use ichoi::config::AccessMode;
use ichoi::db::store;
use libichoi::csil::services::AdminService;
use libichoi::csil::types::{Page, RevokeTrustedIdentityRequest, TrustIdentityRequest};
use tower::ServiceExt;

#[test]
fn configured_and_admin_trust_are_reconciled_as_a_union() {
    let (_app, pool) = common::test_app();
    let mut config = common::test_config();
    config.linkkeys_rp = true;
    config.linkkeys_trusted_identities = vec!["old@example.com".into()];
    ichoi::auth::local_rp::initialize_database(&pool, &config).unwrap();
    {
        let mut conn = pool.get().unwrap();
        store::add_linkkeys_trust(&mut conn, "friends.example", Some("alice"), "admin").unwrap();
    }

    config.linkkeys_trusted_identities = vec!["new@example.com".into()];
    ichoi::auth::local_rp::initialize_database(&pool, &config).unwrap();

    let mut conn = pool.get().unwrap();
    let trust = store::list_linkkeys_trust(&mut conn).unwrap();
    assert!(trust.iter().any(|entry| {
        entry.domain == "example.com" && entry.handle == "new" && entry.source == "config"
    }));
    assert!(trust.iter().any(|entry| {
        entry.domain == "friends.example" && entry.handle == "alice" && entry.source == "admin"
    }));
    assert!(!trust.iter().any(|entry| entry.handle == "old"));
}

#[test]
fn admins_manage_exact_identity_trust_but_not_configured_entries() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        store::add_linkkeys_trust(&mut conn, "configured.example", None, "config").unwrap();
    }
    let ctx = common::ctx_admin("admin@example.com");
    app.trust_identity(
        &ctx,
        TrustIdentityRequest {
            identity: "alice@friends.example".into(),
        },
    )
    .unwrap();
    let listed = app
        .list_trusted_identities(
            &ctx,
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(listed.identities.len(), 2);
    app.revoke_trusted_identity(
        &ctx,
        RevokeTrustedIdentityRequest {
            identity: "alice@friends.example".into(),
        },
    )
    .unwrap();
    assert!(app
        .revoke_trusted_identity(
            &ctx,
            RevokeTrustedIdentityRequest {
                identity: "configured.example".into(),
            },
        )
        .is_err());
}

#[tokio::test]
async fn login_required_blocks_media_until_the_browser_session_cookie_is_set() {
    let (mut app, pool) = common::test_app();
    let mut config = common::test_config();
    config.access_mode = AccessMode::LoginRequired;
    app.config = Arc::new(config);
    let router = ichoi::server::http::router(app.clone(), ".".into());

    let denied = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/media/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    {
        let mut conn = pool.get().unwrap();
        store::upsert_account(
            &mut conn,
            &ichoi::db::models::Account {
                id: "user@example.com".into(),
                handle: "user".into(),
                display_name: None,
                role: "member".into(),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .unwrap();
        store::create_session(
            &mut conn,
            &ichoi::auth::sha256_hex("browser-secret"),
            "user@example.com",
            "2099-01-01T00:00:00Z",
        )
        .unwrap();
    }
    let cookie = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/session")
                .header(header::AUTHORIZATION, "Bearer browser-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cookie.status(), StatusCode::NO_CONTENT);
    let set_cookie = cookie
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    let cookie_pair = set_cookie.split(';').next().unwrap();

    let allowed = router
        .oneshot(
            Request::builder()
                .uri("/media/missing")
                .header(header::COOKIE, cookie_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn lan_guests_uses_forwarded_address_only_from_a_trusted_proxy() {
    let (mut app, _pool) = common::test_app();
    let mut config = common::test_config();
    config.access_mode = AccessMode::LanGuests;
    config.trusted_proxy_cidrs = vec!["10.19.81.5/32".parse().unwrap()];
    app.config = Arc::new(config);
    let router = ichoi::server::http::router(app, ".".into());

    for (forwarded, expected) in [("192.168.1.20", true), ("203.0.113.20", false)] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/auth")
                    .header("x-forwarded-for", forwarded)
                    .extension(ConnectInfo(
                        "10.19.81.5:54321".parse::<std::net::SocketAddr>().unwrap(),
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(status["guest_allowed"], expected);
    }

    let appended_spoof = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/auth")
                .header("x-forwarded-for", "192.168.1.20, 203.0.113.20")
                .extension(ConnectInfo(
                    "10.19.81.5:54321".parse::<std::net::SocketAddr>().unwrap(),
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = appended_spoof
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status["guest_allowed"], false);

    let spoofed = router
        .oneshot(
            Request::builder()
                .uri("/api/auth")
                .header("x-forwarded-for", "192.168.1.20")
                .extension(ConnectInfo(
                    "203.0.113.10:54321"
                        .parse::<std::net::SocketAddr>()
                        .unwrap(),
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = spoofed.into_body().collect().await.unwrap().to_bytes();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status["guest_allowed"], false);
}
