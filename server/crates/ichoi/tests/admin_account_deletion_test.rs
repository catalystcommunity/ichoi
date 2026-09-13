mod common;

use std::sync::Arc;

use ichoi::db::{models, store};
use libichoi::csil::services::AdminService;
use libichoi::csil::types::AdminDeleteAccountRequest;

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
fn admin_deletes_a_member_and_another_admin_with_exact_confirmation() {
    let (app, pool, _music) = configured_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin-one@example.com", "admin");
    seed_account(&mut conn, "admin-two@example.com", "admin");
    seed_account(&mut conn, "member@example.com", "member");
    drop(conn);

    app.delete_account(
        &common::ctx_admin("admin-one@example.com"),
        AdminDeleteAccountRequest {
            account_id: "member@example.com".into(),
            confirmation_handle: "member".into(),
        },
    )
    .unwrap();
    app.delete_account(
        &common::ctx_admin("admin-one@example.com"),
        AdminDeleteAccountRequest {
            account_id: "admin-two@example.com".into(),
            confirmation_handle: "admin-two".into(),
        },
    )
    .unwrap();
    let mut conn = pool.get().unwrap();
    assert!(store::get_account(&mut conn, "member@example.com")
        .unwrap()
        .is_none());
    assert!(store::get_account(&mut conn, "admin-two@example.com")
        .unwrap()
        .is_none());
}

#[test]
fn admin_deletion_rejects_access_missing_wrong_confirmation_and_last_admin() {
    let (app, pool, _music) = configured_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    seed_account(&mut conn, "member@example.com", "member");
    drop(conn);
    let request = || AdminDeleteAccountRequest {
        account_id: "member@example.com".into(),
        confirmation_handle: "member".into(),
    };
    assert_eq!(
        app.delete_account(&common::ctx_user("member@example.com"), request())
            .unwrap_err()
            .code,
        403
    );
    assert_eq!(
        app.delete_account(&common::ctx_anon(), request())
            .unwrap_err()
            .code,
        401
    );
    assert_eq!(
        app.delete_account(
            &common::ctx_admin("admin@example.com"),
            AdminDeleteAccountRequest {
                account_id: "missing@example.com".into(),
                confirmation_handle: "missing".into(),
            },
        )
        .unwrap_err()
        .code,
        404
    );
    assert_eq!(
        app.delete_account(
            &common::ctx_admin("admin@example.com"),
            AdminDeleteAccountRequest {
                account_id: "member@example.com".into(),
                confirmation_handle: "Member".into(),
            },
        )
        .unwrap_err()
        .code,
        400
    );
    assert_eq!(
        app.delete_account(
            &common::ctx_admin("admin@example.com"),
            AdminDeleteAccountRequest {
                account_id: "admin@example.com".into(),
                confirmation_handle: "admin".into(),
            },
        )
        .unwrap_err()
        .code,
        409
    );
}
