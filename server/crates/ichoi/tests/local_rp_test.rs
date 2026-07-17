mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use ichoi::auth::local_rp::{Backend, VerifiedIdentity};
use ichoi::db::store;
use ichoi::handlers::{App, Ctx, Identity};
use libichoi::csil::services::SessionService;
use libichoi::csil::types::AuthRequest;
use tower::ServiceExt;

struct FakeBackend;

impl Backend for FakeBackend {
    fn fingerprint(&self) -> &str {
        "fake-fingerprint"
    }

    fn begin(&self, domain: &str, callback_url: &str) -> anyhow::Result<(String, String)> {
        let pending = linkkeys_local_rp::PendingLogin {
            nonce: vec![1; 32],
            state: vec![2; 32],
            user_domain: domain.to_string(),
            callback_url: callback_url.to_string(),
            required_claims: vec!["handle".to_string()],
        };
        Ok((callback_url.to_string(), serde_json::to_string(&pending)?))
    }

    fn complete(
        &self,
        _pending_json: &str,
        encrypted_token: &str,
        _arrived_url: &str,
    ) -> anyhow::Result<VerifiedIdentity> {
        anyhow::ensure!(encrypted_token == "fake-token");
        Ok(VerifiedIdentity {
            user_id: "user-uuid".to_string(),
            domain: "family.example".to_string(),
            handle: "alice".to_string(),
            display_name: Some("Alice".to_string()),
        })
    }
}

fn enabled_app() -> (App, ichoi::db::SqlitePool) {
    let mut config = common::test_config();
    config.linkkeys_local_rp = true;
    config.linkkeys_local_rp_name = Some("Test Ichoi".to_string());
    config.linkkeys_trusted_identities = vec!["alice@family.example".to_string()];
    let pool = ichoi::db::test_pool();
    ichoi::auth::local_rp::initialize_database(&pool, &config).unwrap();
    let app = App::new(pool.clone(), Arc::new(config)).with_local_rp_backend(Arc::new(FakeBackend));
    (app, pool)
}

#[test]
fn enabled_configuration_requires_name_and_valid_trust_selectors() {
    let pool = ichoi::db::test_pool();
    let mut config = common::test_config();
    config.linkkeys_local_rp = true;
    assert!(ichoi::auth::local_rp::initialize_database(&pool, &config).is_err());

    config.linkkeys_local_rp_name = Some("Test Ichoi".to_string());
    assert!(ichoi::auth::local_rp::initialize_database(&pool, &config).is_err());

    config.linkkeys_trusted_identities = vec!["https://family.example".to_string()];
    assert!(ichoi::auth::local_rp::initialize_database(&pool, &config).is_err());
}

#[test]
fn identity_is_stable_and_configured_trust_is_seeded() {
    let mut config = common::test_config();
    config.linkkeys_local_rp = true;
    config.linkkeys_local_rp_name = Some("Test Ichoi".to_string());
    config.linkkeys_trusted_identities = vec![
        "family.example".to_string(),
        "bob@friends.example".to_string(),
    ];
    let pool = ichoi::db::test_pool();
    ichoi::auth::local_rp::initialize_database(&pool, &config).unwrap();
    let first = store::active_local_rp_identity(&mut pool.get().unwrap())
        .unwrap()
        .unwrap();
    ichoi::auth::local_rp::initialize_database(&pool, &config).unwrap();
    let second = store::active_local_rp_identity(&mut pool.get().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(first.fingerprint, second.fingerprint);
    assert_eq!(first.identity_bundle, second.identity_bundle);
    let sdk = ichoi::auth::local_rp::SdkBackend::load(&pool).unwrap();
    let (redirect, pending) = sdk
        .begin(
            "family.example",
            "http://ichoi-box:4042/auth/linkkeys/local/callback?attempt=test",
        )
        .unwrap();
    assert!(redirect.starts_with("https://family.example/auth/local-rp?signed_request="));
    let pending: linkkeys_local_rp::PendingLogin = serde_json::from_str(&pending).unwrap();
    assert_eq!(pending.user_domain, "family.example");
    let mut conn = pool.get().unwrap();
    assert!(
        store::linkkeys_identity_is_trusted(&mut conn, "family.example", Some("anyone")).unwrap()
    );
    assert!(
        store::linkkeys_identity_is_trusted(&mut conn, "friends.example", Some("bob")).unwrap()
    );
    assert!(
        !store::linkkeys_identity_is_trusted(&mut conn, "friends.example", Some("alice")).unwrap()
    );

    store::upsert_account(
        &mut conn,
        &ichoi::db::models::Account {
            id: "existing@family.example".to_string(),
            handle: "old".to_string(),
            display_name: None,
            role: "admin".to_string(),
            created_at: "2020-01-01T00:00:00Z".to_string(),
        },
    )
    .unwrap();
    let refreshed = store::upsert_linkkeys_account(
        &mut conn,
        "existing@family.example",
        "new",
        Some("New Name"),
    )
    .unwrap();
    assert_eq!(refreshed.role, "admin");
    assert_eq!(refreshed.created_at, "2020-01-01T00:00:00Z");
    assert_eq!(refreshed.handle, "new");
}

#[tokio::test]
async fn browser_flow_is_offline_single_use_and_mints_normal_session() {
    let (app, _pool) = enabled_app();
    let router = ichoi::server::http::router(app.clone(), ".".into());
    let untrusted = router
        .clone()
        .oneshot(
            Request::post("/auth/linkkeys/local/start")
                .header(header::HOST, "ichoi-box:4042")
                .header(header::ORIGIN, "http://ichoi-box:4042")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"identity":"bob@family.example"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(untrusted.status(), StatusCode::FORBIDDEN);

    let cross_origin = router
        .clone()
        .oneshot(
            Request::post("/auth/linkkeys/local/start")
                .header(header::HOST, "ichoi-box:4042")
                .header(header::ORIGIN, "http://attacker.example")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"identity":"alice@family.example"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);

    let start = router
        .clone()
        .oneshot(
            Request::post("/auth/linkkeys/local/start")
                .header(header::HOST, "ichoi-box:4042")
                .header(header::ORIGIN, "http://ichoi-box:4042")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"identity":"alice@family.example"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::OK);
    let body = start.into_body().collect().await.unwrap().to_bytes();
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let callback = response["redirect_url"].as_str().unwrap();
    let attempt = callback.split("attempt=").nth(1).unwrap();
    let callback_uri =
        format!("/auth/linkkeys/local/callback?attempt={attempt}&encrypted_token=fake-token");
    let completed = router
        .clone()
        .oneshot(Request::get(&callback_uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(completed.status(), StatusCode::SEE_OTHER);
    let location = completed.headers()[header::LOCATION].to_str().unwrap();
    let code = location.split("#linkkeys_exchange=").nth(1).unwrap();

    let info = app
        .authenticate(
            &Ctx {
                identity: Identity::Anonymous,
            },
            AuthRequest {
                linkkeys_assertion: None,
                linkkeys_exchange_code: Some(code.to_string()),
                bootstrap_token: None,
            },
        )
        .unwrap();
    assert_eq!(info.account_id, "user-uuid@family.example");
    assert_eq!(info.handle, "alice");
    assert!(info.token.is_some());

    let replay = router
        .oneshot(Request::get(&callback_uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}
