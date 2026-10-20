//! Who may see and control an account-owned browser share.

mod common;

use ichoi::db::{models, store};
use libichoi::csil::services::{AdminService, LibraryService, PlayerService};
use libichoi::csil::types::*;

fn seed_account(conn: &mut diesel::SqliteConnection, id: &str, role: &str) {
    store::upsert_account(
        conn,
        &models::Account {
            id: id.to_string(),
            handle: id.split('@').next().unwrap_or(id).to_string(),
            display_name: None,
            role: role.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        },
    )
    .unwrap();
}

fn pause(player_id: &str) -> CommandRequest {
    CommandRequest {
        player_id: player_id.into(),
        command: PlayerCommand::Variant9(CmdPause { op: "pause".into() }),
    }
}

fn visible_to(app: &ichoi::handlers::App, ctx: &ichoi::handlers::Ctx, player_id: &str) -> bool {
    app.list_players(ctx, ListPlayersRequest { kind: None })
        .unwrap()
        .players
        .iter()
        .any(|player| player.id == player_id)
}

/// A server with an admin, a share owner (alice), and two other members.
fn setup() -> (ichoi::handlers::App, ichoi::db::SqlitePool) {
    let (app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    seed_account(&mut conn, "alice@example.com", "member");
    seed_account(&mut conn, "bob@example.com", "member");
    seed_account(&mut conn, "carol@example.com", "member");
    drop(conn);
    (app, pool)
}

fn share(app: &ichoi::handlers::App) -> String {
    let id = app
        .enable_share(
            &common::ctx_user("alice@example.com"),
            EnableShareRequest {
                suffix: Some("Laptop".into()),
            },
        )
        .unwrap()
        .player
        .id;
    app.presence.attach(id.clone(), 7);
    id
}

#[test]
fn default_access_admits_every_signed_in_account_but_not_guests() {
    let (app, _pool) = setup();
    let id = share(&app);

    for member in ["bob@example.com", "carol@example.com"] {
        assert!(visible_to(&app, &common::ctx_user(member), &id));
        app.control(&common::ctx_user(member), pause(&id))
            .expect("signed-in account controls a share open to everyone");
    }
    assert!(!visible_to(&app, &common::ctx_anon(), &id));
    assert_eq!(
        app.control(&common::ctx_anon(), pause(&id))
            .unwrap_err()
            .code,
        401
    );
}

#[test]
fn private_server_default_admits_only_the_owner_and_selected_accounts() {
    let (app, _pool) = setup();
    app.set_setting(
        &common::ctx_admin("admin@example.com"),
        SetSettingRequest {
            key: "share_default_access".into(),
            value: "private".into(),
        },
    )
    .unwrap();
    let id = share(&app);
    let alice = common::ctx_user("alice@example.com");
    let bob = common::ctx_user("bob@example.com");
    let carol = common::ctx_user("carol@example.com");

    app.control(&alice, pause(&id))
        .expect("owner always has access");
    assert!(!visible_to(&app, &bob, &id));
    assert_eq!(app.control(&bob, pause(&id)).unwrap_err().code, 403);
    assert_eq!(
        app.get_state(
            &bob,
            SubscribeRequest {
                player_id: id.clone(),
                active: None,
            },
        )
        .unwrap_err()
        .code,
        403
    );

    let info = app
        .get_share_access(
            &alice,
            ShareAccessRequest {
                player_id: id.clone(),
            },
        )
        .unwrap();
    assert!(matches!(info.access, ShareAccess::Selected));
    let mut candidates = info
        .accounts
        .iter()
        .map(|account| (account.account_id.as_str(), account.selected))
        .collect::<Vec<_>>();
    candidates.sort();
    assert_eq!(
        candidates,
        vec![
            ("admin@example.com", false),
            ("bob@example.com", false),
            ("carol@example.com", false),
        ],
        "the owner is never a candidate"
    );

    let info = app
        .set_share_access(
            &alice,
            SetShareAccessRequest {
                player_id: id.clone(),
                access: ShareAccess::Selected,
                account_ids: vec!["bob@example.com".into()],
            },
        )
        .unwrap();
    assert!(info
        .accounts
        .iter()
        .any(|account| account.account_id == "bob@example.com" && account.selected));
    assert!(visible_to(&app, &bob, &id));
    app.control(&bob, pause(&id)).expect("selected account");
    assert_eq!(app.control(&carol, pause(&id)).unwrap_err().code, 403);

    // Switching to everyone keeps the selection for a later switch back.
    let info = app
        .set_share_access(
            &alice,
            SetShareAccessRequest {
                player_id: id.clone(),
                access: ShareAccess::Everyone,
                account_ids: vec!["bob@example.com".into()],
            },
        )
        .unwrap();
    assert!(matches!(info.access, ShareAccess::Everyone));
    app.control(&carol, pause(&id))
        .expect("open to everyone again");
}

#[test]
fn only_the_owner_manages_access_and_the_owner_cannot_be_listed() {
    let (app, _pool) = setup();
    let id = share(&app);
    let request = || ShareAccessRequest {
        player_id: id.clone(),
    };
    assert_eq!(
        app.get_share_access(&common::ctx_user("bob@example.com"), request())
            .unwrap_err()
            .code,
        403
    );
    assert_eq!(
        app.get_share_access(&common::ctx_anon(), request())
            .unwrap_err()
            .code,
        401
    );
    let set = |account_ids: Vec<String>| SetShareAccessRequest {
        player_id: id.clone(),
        access: ShareAccess::Selected,
        account_ids,
    };
    let alice = common::ctx_user("alice@example.com");
    assert_eq!(
        app.set_share_access(&alice, set(vec!["alice@example.com".into()]))
            .unwrap_err()
            .code,
        400
    );
    assert_eq!(
        app.set_share_access(&alice, set(vec!["nobody@example.com".into()]))
            .unwrap_err()
            .code,
        404
    );
}

#[test]
fn blocking_an_account_removes_it_from_the_owners_shares() {
    let (app, _pool) = setup();
    let id = share(&app);
    let alice = common::ctx_user("alice@example.com");
    let bob = common::ctx_user("bob@example.com");
    app.set_share_access(
        &alice,
        SetShareAccessRequest {
            player_id: id.clone(),
            access: ShareAccess::Selected,
            account_ids: vec!["bob@example.com".into()],
        },
    )
    .unwrap();
    app.block_account(
        &alice,
        BlockAccountRequest {
            account_id: "bob@example.com".into(),
        },
    )
    .unwrap();

    assert_eq!(app.control(&bob, pause(&id)).unwrap_err().code, 403);
    let info = app
        .get_share_access(
            &alice,
            ShareAccessRequest {
                player_id: id.clone(),
            },
        )
        .unwrap();
    assert!(
        info.accounts
            .iter()
            .all(|account| account.account_id != "bob@example.com"),
        "a blocked account is not a candidate"
    );

    // A blocked account stays out even when the share is open to everyone.
    app.set_share_access(
        &alice,
        SetShareAccessRequest {
            player_id: id.clone(),
            access: ShareAccess::Everyone,
            account_ids: vec![],
        },
    )
    .unwrap();
    assert!(!visible_to(&app, &bob, &id));
    assert_eq!(app.control(&bob, pause(&id)).unwrap_err().code, 403);
    app.control(&common::ctx_user("carol@example.com"), pause(&id))
        .expect("other accounts still have access");

    // Unblocking does not restore the old selection.
    app.unblock_account(
        &alice,
        BlockAccountRequest {
            account_id: "bob@example.com".into(),
        },
    )
    .unwrap();
    app.set_share_access(
        &alice,
        SetShareAccessRequest {
            player_id: id.clone(),
            access: ShareAccess::Selected,
            account_ids: vec![],
        },
    )
    .unwrap();
    assert_eq!(app.control(&bob, pause(&id)).unwrap_err().code, 403);
}
