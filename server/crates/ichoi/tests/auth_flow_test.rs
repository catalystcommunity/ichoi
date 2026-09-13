//! Authentication flow: first-admin bootstrap and its one-shot nature (§7.4).

mod common;

use std::sync::Arc;

use diesel::prelude::*;
use libichoi::csil::services::SessionService;
use libichoi::csil::types::*;

#[test]
fn bootstrap_creates_admin_and_mints_token() {
    let (app, _pool) = common::test_app();

    let info = app
        .authenticate(
            &common::ctx_anon(),
            AuthRequest {
                linkkeys_assertion: None,
                linkkeys_exchange_code: None,
                bootstrap_token: Some("test-admin-token".to_string()),
            },
        )
        .expect("bootstrap");

    assert!(matches!(info.role, Role::Admin));
    assert!(info.token.is_some(), "a session token is minted");
    assert_eq!(info.handle, "admin");
}

#[test]
fn bootstrap_rejects_wrong_token() {
    let (app, _pool) = common::test_app();

    let result = app.authenticate(
        &common::ctx_anon(),
        AuthRequest {
            linkkeys_assertion: None,
            linkkeys_exchange_code: None,
            bootstrap_token: Some("wrong".to_string()),
        },
    );

    let err = result.expect_err("wrong token rejected");
    assert_eq!(err.code, 403);
}

#[test]
fn bootstrap_is_one_shot() {
    let (app, _pool) = common::test_app();
    // First bootstrap succeeds and creates an account.
    app.authenticate(
        &common::ctx_anon(),
        AuthRequest {
            linkkeys_assertion: None,
            linkkeys_exchange_code: None,
            bootstrap_token: Some("test-admin-token".to_string()),
        },
    )
    .expect("first bootstrap");

    // Once an account exists, the bootstrap token is inert: with no LinkKeys assertion the
    // request is treated as an anonymous (guest) attempt and refused.
    let result = app.authenticate(
        &common::ctx_anon(),
        AuthRequest {
            linkkeys_assertion: None,
            linkkeys_exchange_code: None,
            bootstrap_token: Some("test-admin-token".to_string()),
        },
    );
    assert!(
        result.is_err(),
        "bootstrap no longer applies once accounts exist"
    );
}

#[test]
fn new_session_uses_the_configured_lifetime() {
    let (mut app, pool) = common::test_app();
    let mut config = common::test_config();
    config.session_lifetime_hours = 24;
    app.config = Arc::new(config);
    let before = chrono::Utc::now();
    let info = app
        .authenticate(
            &common::ctx_anon(),
            AuthRequest {
                linkkeys_assertion: None,
                linkkeys_exchange_code: None,
                bootstrap_token: Some("test-admin-token".into()),
            },
        )
        .unwrap();
    let token_hash = ichoi::auth::sha256_hex(info.token.as_deref().unwrap());
    let expires: String = ichoi::db::schema::sessions::table
        .find(token_hash)
        .select(ichoi::db::schema::sessions::expires_at)
        .first(&mut pool.get().unwrap())
        .unwrap();
    let expires = chrono::DateTime::parse_from_rfc3339(&expires)
        .unwrap()
        .with_timezone(&chrono::Utc);
    let lifetime = expires - before;
    assert!(lifetime >= chrono::Duration::hours(24));
    assert!(lifetime < chrono::Duration::hours(24) + chrono::Duration::seconds(2));
}

#[test]
fn expired_session_is_not_authenticated() {
    let (_app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    ichoi::db::store::upsert_account(
        &mut conn,
        &ichoi::db::models::Account {
            id: "expired@example.com".into(),
            handle: "expired".into(),
            display_name: None,
            role: "member".into(),
            created_at: "2026-09-12T00:00:00Z".into(),
        },
    )
    .unwrap();
    let token_hash = ichoi::auth::sha256_hex("expired-token");
    ichoi::db::store::create_session(
        &mut conn,
        &token_hash,
        "expired@example.com",
        "2000-01-01T00:00:00Z",
    )
    .unwrap();

    assert!(ichoi::db::store::account_for_token(&mut conn, &token_hash)
        .unwrap()
        .is_none());
}
