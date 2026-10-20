mod common;

use ichoi::db::{models, store};
use ichoi::transport::dispatch;
use libichoi::csil::codec::{
    decode_blocked_accounts_response, decode_ok, encode_block_account_request, encode_page,
};
use libichoi::csil::services::LibraryService;
use libichoi::csil::types::{BlockAccountRequest, BrowseRequest, Page, PlaylistRequest};

fn seed_account(conn: &mut diesel::SqliteConnection, id: &str, handle: &str, role: &str) {
    store::upsert_account(
        conn,
        &models::Account {
            id: id.into(),
            handle: handle.into(),
            display_name: Some(format!("{handle} name")),
            role: role.into(),
            created_at: "2026-09-16T00:00:00Z".into(),
        },
    )
    .unwrap();
}

fn request(account_id: &str) -> BlockAccountRequest {
    BlockAccountRequest {
        account_id: account_id.into(),
    }
}

fn page(offset: u64, limit: u64) -> Page {
    Page {
        offset: Some(offset),
        limit: Some(limit),
    }
}

#[test]
fn block_list_and_unblock_round_trip_through_the_request_dispatcher() {
    let (app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "one@example.com", "one", "member");
    seed_account(&mut conn, "two@example.com", "two", "member");
    drop(conn);
    let ctx = common::ctx_user("one@example.com");
    let request = request("two@example.com");

    let blocked = dispatch(
        &app,
        &ctx,
        "library",
        "block-account",
        &encode_block_account_request(&request),
    )
    .unwrap();
    decode_ok(&blocked).unwrap();

    let listed = dispatch(
        &app,
        &ctx,
        "library",
        "list-blocked-accounts",
        &encode_page(&page(0, 10)),
    )
    .unwrap();
    let listed = decode_blocked_accounts_response(&listed).unwrap();
    assert_eq!(listed.total, 1);
    assert_eq!(listed.accounts[0].account_id, "two@example.com");

    let unblocked = dispatch(
        &app,
        &ctx,
        "library",
        "unblock-account",
        &encode_block_account_request(&request),
    )
    .unwrap();
    decode_ok(&unblocked).unwrap();
}

#[test]
fn block_list_and_unblock_are_authenticated_and_idempotent() {
    let (app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "one@example.com", "one", "member");
    seed_account(&mut conn, "two@example.com", "two", "member");
    seed_account(&mut conn, "three@example.com", "three", "member");
    drop(conn);
    let ctx = common::ctx_user("one@example.com");

    assert_eq!(
        app.block_account(&common::ctx_anon(), request("two@example.com"))
            .unwrap_err()
            .code,
        401
    );
    assert_eq!(
        app.list_blocked_accounts(&common::ctx_anon(), page(0, 10))
            .unwrap_err()
            .code,
        401
    );
    assert_eq!(
        app.block_account(&ctx, request("one@example.com"))
            .unwrap_err()
            .code,
        400
    );
    assert_eq!(
        app.block_account(&ctx, request("missing@example.com"))
            .unwrap_err()
            .code,
        404
    );

    app.block_account(&ctx, request("two@example.com")).unwrap();
    app.block_account(&ctx, request("two@example.com")).unwrap();
    app.block_account(&ctx, request("three@example.com"))
        .unwrap();
    let first = app.list_blocked_accounts(&ctx, page(0, 1)).unwrap();
    let second = app.list_blocked_accounts(&ctx, page(1, 1)).unwrap();
    assert_eq!(first.total, 2);
    assert_eq!(first.accounts.len(), 1);
    assert_eq!(second.total, 2);
    assert_eq!(second.accounts.len(), 1);
    assert_ne!(first.accounts[0].account_id, second.accounts[0].account_id);

    app.unblock_account(&ctx, request("two@example.com"))
        .unwrap();
    app.unblock_account(&ctx, request("two@example.com"))
        .unwrap();
    let remaining = app.list_blocked_accounts(&ctx, page(0, 10)).unwrap();
    assert_eq!(remaining.total, 1);
    assert_eq!(remaining.accounts[0].account_id, "three@example.com");
}

#[test]
fn blocking_hides_public_playlists_and_private_playlists_stay_owner_only() {
    let (app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "viewer@example.com", "viewer", "member");
    seed_account(&mut conn, "blocked@example.com", "blocked", "member");
    seed_account(&mut conn, "other@example.com", "other", "member");
    for (id, owner, visibility) in [
        ("playlist:blocked-public", "blocked@example.com", "public"),
        ("playlist:blocked-private", "blocked@example.com", "private"),
        ("playlist:other-public", "other@example.com", "public"),
    ] {
        store::upsert_playlist(
            &mut conn,
            &models::Playlist {
                id: id.into(),
                name: id.into(),
                owner: Some(owner.into()),
                root_relative_path: format!("playlists/{id}.m3u"),
                visibility: visibility.into(),
            },
        )
        .unwrap();
    }
    drop(conn);
    let ctx = common::ctx_user("viewer@example.com");
    app.block_account(&ctx, request("blocked@example.com"))
        .unwrap();

    let listed = app
        .list_playlists(
            &ctx,
            BrowseRequest {
                library: None,
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    let ids = listed
        .playlists
        .iter()
        .map(|playlist| playlist.id.as_str())
        .collect::<Vec<_>>();
    assert!(!ids.contains(&"playlist:blocked-public"));
    assert!(!ids.contains(&"playlist:blocked-private"));
    assert!(ids.contains(&"playlist:other-public"));
    // `total` counts only the playlists this viewer can see.
    assert_eq!(listed.total, 1);
    assert_eq!(
        app.get_playlist(
            &ctx,
            PlaylistRequest {
                playlist_id: "playlist:blocked-public".into(),
            },
        )
        .unwrap_err()
        .code,
        404
    );
    assert_eq!(
        app.get_playlist(
            &ctx,
            PlaylistRequest {
                playlist_id: "playlist:blocked-private".into(),
            },
        )
        .unwrap_err()
        .code,
        404
    );
    let owner_view = app
        .list_playlists(
            &common::ctx_user("blocked@example.com"),
            BrowseRequest {
                library: None,
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(owner_view
        .playlists
        .iter()
        .any(|playlist| playlist.id == "playlist:blocked-private"));
    assert_eq!(owner_view.total, 3);
    let owner_page = app
        .list_playlists(
            &common::ctx_user("blocked@example.com"),
            BrowseRequest {
                library: None,
                offset: Some(1),
                limit: Some(1),
            },
        )
        .unwrap();
    assert_eq!(owner_page.playlists.len(), 1);
    assert_eq!(owner_page.total, 3);
}

#[test]
fn deleting_either_account_removes_block_rows() {
    let (app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin", "admin");
    seed_account(&mut conn, "blocker@example.com", "blocker", "member");
    seed_account(&mut conn, "blocked@example.com", "blocked", "member");
    seed_account(&mut conn, "other@example.com", "other", "member");
    drop(conn);

    app.block_account(
        &common::ctx_user("blocker@example.com"),
        request("blocked@example.com"),
    )
    .unwrap();
    app.block_account(
        &common::ctx_user("other@example.com"),
        request("blocker@example.com"),
    )
    .unwrap();

    let mut conn = pool.get().unwrap();
    ichoi::deletion::delete_account(&mut conn, music.path(), "blocker@example.com").unwrap();
    assert_eq!(
        store::count_account_blocks(&mut conn, "other@example.com").unwrap(),
        0
    );
    assert_eq!(
        store::count_account_blocks(&mut conn, "blocker@example.com").unwrap(),
        0
    );
}
