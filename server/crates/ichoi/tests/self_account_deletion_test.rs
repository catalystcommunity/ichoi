mod common;

use std::sync::Arc;

use ichoi::db::{models, store};
use libichoi::csil::services::SessionService;
use libichoi::csil::types::DeleteAccountRequest;

fn seed_account(conn: &mut diesel::SqliteConnection, id: &str, role: &str) {
    store::upsert_account(
        conn,
        &models::Account {
            id: id.into(),
            handle: id.split('@').next().unwrap().into(),
            display_name: None,
            role: role.into(),
            created_at: "2026-09-12T00:00:00Z".into(),
        },
    )
    .unwrap();
}

fn configured_app() -> (
    ichoi::handlers::App,
    ichoi::db::SqlitePool,
    tempfile::TempDir,
) {
    let (mut app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    let mut config = common::test_config();
    config.music_dir = Some(music.path().to_owned());
    app.config = Arc::new(config);
    (app, pool, music)
}

#[test]
fn self_deletion_requires_exact_handle_and_invalidates_all_sessions() {
    let (app, pool, _music) = configured_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    seed_account(&mut conn, "member@example.com", "member");
    for token in ["one", "two"] {
        store::create_session(
            &mut conn,
            &ichoi::auth::sha256_hex(token),
            "member@example.com",
            "2099-01-01T00:00:00Z",
        )
        .unwrap();
    }
    drop(conn);

    assert_eq!(
        app.delete_account(
            &common::ctx_user("member@example.com"),
            DeleteAccountRequest {
                confirmation_handle: "Member".into(),
            },
        )
        .unwrap_err()
        .code,
        400
    );
    app.delete_account(
        &common::ctx_user("member@example.com"),
        DeleteAccountRequest {
            confirmation_handle: "member".into(),
        },
    )
    .unwrap();
    let mut conn = pool.get().unwrap();
    for token in ["one", "two"] {
        assert!(
            store::account_for_token(&mut conn, &ichoi::auth::sha256_hex(token))
                .unwrap()
                .is_none()
        );
    }
    drop(conn);
    assert_eq!(
        app.delete_account(
            &common::ctx_user("member@example.com"),
            DeleteAccountRequest {
                confirmation_handle: "member".into(),
            },
        )
        .unwrap_err()
        .code,
        404
    );
}

#[test]
fn anonymous_and_last_admin_deletion_are_rejected() {
    let (app, pool, _music) = configured_app();
    seed_account(&mut pool.get().unwrap(), "admin@example.com", "admin");
    assert_eq!(
        app.delete_account(
            &common::ctx_anon(),
            DeleteAccountRequest {
                confirmation_handle: "admin".into(),
            },
        )
        .unwrap_err()
        .code,
        401
    );
    assert_eq!(
        app.delete_account(
            &common::ctx_admin("admin@example.com"),
            DeleteAccountRequest {
                confirmation_handle: "admin".into(),
            },
        )
        .unwrap_err()
        .code,
        409
    );
}
