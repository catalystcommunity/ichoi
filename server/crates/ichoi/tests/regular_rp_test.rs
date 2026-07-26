mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use ichoi::auth::local_rp::VerifiedIdentity;
use ichoi::auth::regular_rp::{Backend, PendingLogin};
use ichoi::handlers::{App, Ctx, Identity};
use libichoi::csil::services::SessionService;
use libichoi::csil::types::{AuthRequest, Role};
use tower::ServiceExt;

struct FakeBackend;

impl Backend for FakeBackend {
    fn begin(
        &self,
        domain: &str,
        user_hint: Option<&str>,
        callback_url: &str,
    ) -> anyhow::Result<(String, String)> {
        let pending = PendingLogin {
            nonce: "fake-nonce".to_string(),
            user_domain: domain.to_string(),
            callback_url: callback_url.to_string(),
            api_base: format!("https://idp.{domain}"),
        };
        Ok((
            format!(
                "https://idp.{domain}/auth/authorize?user_hint={}&callback_url={callback_url}",
                user_hint.unwrap_or("")
            ),
            serde_json::to_string(&pending)?,
        ))
    }

    fn complete(
        &self,
        pending_json: &str,
        encrypted_token: &str,
    ) -> anyhow::Result<VerifiedIdentity> {
        let pending: PendingLogin = serde_json::from_str(pending_json)?;
        anyhow::ensure!(encrypted_token == "fake-token");
        Ok(VerifiedIdentity {
            user_id: "regular-user-uuid".to_string(),
            domain: pending.user_domain,
            handle: "alice-public".to_string(),
            display_name: Some("Alice Regular".to_string()),
        })
    }
}

fn enabled_app() -> App {
    let mut config = common::test_config();
    config.linkkeys_rp = true;
    config.linkkeys_rp_addr = Some("rp.example:4987".to_string());
    config.linkkeys_rp_fingerprints = vec!["a".repeat(64)];
    config.linkkeys_rp_api_key = Some("test-api-key".to_string());
    config.linkkeys_rp_domain = Some("ichoi.example".to_string());
    config.public_url = Some("https://ichoi.example".to_string());
    config.linkkeys_trusted_identities = vec!["family.example".to_string()];
    let pool = ichoi::db::test_pool();
    ichoi::auth::local_rp::initialize_database(&pool, &config).unwrap();
    App::new(pool, Arc::new(config)).with_regular_rp_backend(Arc::new(FakeBackend))
}

#[test]
fn regular_rp_requires_complete_configuration_with_optional_initial_trust() {
    let mut config = common::test_config();
    config.linkkeys_rp = true;
    assert!(ichoi::app::validate_runtime_config(&config).is_err());

    config.linkkeys_rp_addr = Some("rp.example:4987".to_string());
    config.linkkeys_rp_fingerprints = vec!["a".repeat(64)];
    config.linkkeys_rp_api_key = Some("test-api-key".to_string());
    config.linkkeys_rp_domain = Some("ichoi.example".to_string());
    config.public_url = Some("http://ichoi.example".to_string());
    assert!(ichoi::app::validate_runtime_config(&config)
        .unwrap_err()
        .to_string()
        .contains("HTTPS origin"));

    config.public_url = Some("https://ichoi.example".to_string());
    assert!(ichoi::app::validate_runtime_config(&config).is_ok());

    config.linkkeys_local_rp = true;
    assert!(ichoi::app::validate_runtime_config(&config)
        .unwrap_err()
        .to_string()
        .contains("mutually exclusive"));
}

#[tokio::test]
async fn regular_rp_browser_flow_is_mocked_single_use_and_mints_session() {
    let app = enabled_app();
    let router = ichoi::server::http::router(app.clone(), ".".into());

    let status = router
        .clone()
        .oneshot(Request::get("/api/auth").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = status.into_body().collect().await.unwrap().to_bytes();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(status["local_rp"].is_null());
    assert_eq!(status["regular_rp"]["start_url"], "/auth/linkkeys/start");

    let local_route = router
        .clone()
        .oneshot(
            Request::post("/auth/linkkeys/local/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(local_route.status(), StatusCode::METHOD_NOT_ALLOWED);

    let start = router
        .clone()
        .oneshot(
            Request::post("/auth/linkkeys/start")
                .header(header::HOST, "ichoi.example")
                .header(header::ORIGIN, "https://ichoi.example")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"identity":"alice@family.example"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::OK);
    let body = start.into_body().collect().await.unwrap().to_bytes();
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let redirect = response["redirect_url"].as_str().unwrap();
    assert!(redirect.contains("user_hint=alice"));
    let callback = redirect.split("callback_url=").nth(1).unwrap();
    let attempt = callback.split("attempt=").nth(1).unwrap();

    let callback_uri =
        format!("/auth/linkkeys/callback?attempt={attempt}&encrypted_token=fake-token");
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
                allow_guest: false,
            },
            AuthRequest {
                linkkeys_assertion: None,
                linkkeys_exchange_code: Some(code.to_string()),
                bootstrap_token: None,
            },
        )
        .unwrap();
    assert_eq!(info.account_id, "regular-user-uuid@family.example");
    assert_eq!(info.handle, "alice-public");
    assert_eq!(info.role, Role::Admin);
    assert!(info.can_admin);
    assert!(info.token.is_some());

    let replay = router
        .oneshot(Request::get(&callback_uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}
