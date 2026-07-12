//! Authentication flow: first-admin bootstrap and its one-shot nature (§7.4).

mod common;

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
            bootstrap_token: Some("test-admin-token".to_string()),
        },
    );
    assert!(
        result.is_err(),
        "bootstrap no longer applies once accounts exist"
    );
}
