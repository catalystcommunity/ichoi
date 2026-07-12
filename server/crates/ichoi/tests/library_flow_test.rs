//! Library handlers exercised end-to-end against a real (rolled-back) database.

mod common;

use common::DataMap;
use libichoi::csil::services::{LibraryService, PlayerService};
use libichoi::csil::types::*;

#[test]
fn lists_albums_with_track_counts() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        common::create_artist(&mut conn, &DataMap::new());
        common::create_album(&mut conn, &DataMap::new());
        common::create_track(&mut conn, &DataMap::new());
        let mut second = DataMap::new();
        second.insert("id".into(), "track-2".into());
        second.insert(
            "root_relative_path".into(),
            "Test Artist/Test Album/02.flac".into(),
        );
        common::create_track(&mut conn, &second);
    }

    let resp = app
        .list_albums(
            &common::ctx_anon(),
            BrowseRequest {
                library: None,
                offset: None,
                limit: None,
            },
        )
        .expect("list albums");

    assert_eq!(resp.total, 1);
    assert_eq!(resp.albums.len(), 1);
    assert_eq!(resp.albums[0].title, "Test Album");
    assert_eq!(resp.albums[0].track_count, 2);
}

#[test]
fn search_finds_track_by_title() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        common::create_artist(&mut conn, &DataMap::new());
        common::create_album(&mut conn, &DataMap::new());
        let mut o = DataMap::new();
        o.insert("title".into(), "Rare Gem".into());
        common::create_track(&mut conn, &o);
    }

    let resp = app
        .search(
            &common::ctx_anon(),
            SearchRequest {
                query: "Rare".to_string(),
                limit: None,
            },
        )
        .expect("search");

    assert_eq!(resp.tracks.len(), 1);
    assert_eq!(resp.tracks[0].title, "Rare Gem");
}

#[test]
fn album_detail_returns_ordered_tracks() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        common::create_artist(&mut conn, &DataMap::new());
        common::create_album(&mut conn, &DataMap::new());
        common::create_track(&mut conn, &DataMap::new());
    }

    let detail = app
        .get_album(
            &common::ctx_anon(),
            AlbumRequest {
                album_id: "album-1".to_string(),
            },
        )
        .expect("album detail");

    assert_eq!(detail.album.id, "album-1");
    assert_eq!(detail.tracks.len(), 1);
    assert!(matches!(detail.tracks[0].codec, Codec::Flac));
}

#[test]
fn anonymous_can_share_device_when_no_users() {
    let (app, _pool) = common::test_app();
    let share = app
        .enable_share(&common::ctx_anon(), EnableShareRequest { suffix: None })
        .expect("anonymous share is allowed on a login-less instance");
    assert_eq!(share.player.name, "Guest's Device");
    assert!(share.player.owner.is_none());
}

#[test]
fn anonymous_cannot_share_once_users_exist() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        ichoi::db::store::upsert_account(
            &mut conn,
            &ichoi::db::models::Account {
                id: "a@b".into(),
                handle: "a".into(),
                display_name: None,
                role: "admin".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
    }
    let err = app
        .enable_share(&common::ctx_anon(), EnableShareRequest { suffix: None })
        .expect_err("sign-in required once accounts exist");
    assert_eq!(err.code, 401);
}

#[test]
fn shared_devices_are_listed_only_while_a_connection_owns_them() {
    let (app, _pool) = common::test_app();
    let share = app
        .enable_share(&common::ctx_anon(), EnableShareRequest { suffix: Some("TodPhone".into()) })
        .expect("share");
    let id = share.player.id.clone();

    // No connection owns its output yet → reconciled out of the device list.
    let listed = app
        .list_players(&common::ctx_anon(), ListPlayersRequest { kind: None })
        .expect("list");
    assert!(
        !listed.players.iter().any(|p| p.id == id),
        "an unclaimed shared device must not appear"
    );

    // A live connection claims output (as ws_conn does on a successful EnableShare) → it appears.
    app.presence.attach(id.clone(), 42);
    let listed = app
        .list_players(&common::ctx_anon(), ListPlayersRequest { kind: None })
        .expect("list");
    assert!(listed.players.iter().any(|p| p.id == id), "a claimed device is live");

    // Re-claiming the same device by the same owner is idempotent (no 409), so a client can
    // re-attach after a refresh instead of failing on its own device.
    let again = app
        .enable_share(&common::ctx_anon(), EnableShareRequest { suffix: Some("TodPhone".into()) })
        .expect("re-claiming your own device must succeed");
    assert_eq!(again.player.id, id);

    // The owning connection drops → the device is reconciled back out.
    app.presence.detach_conn(42);
    let listed = app
        .list_players(&common::ctx_anon(), ListPlayersRequest { kind: None })
        .expect("list");
    assert!(
        !listed.players.iter().any(|p| p.id == id),
        "device disappears once its output connection is gone"
    );
}

#[test]
fn control_enqueues_and_plays() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        common::create_artist(&mut conn, &DataMap::new());
        common::create_album(&mut conn, &DataMap::new());
        common::create_track(&mut conn, &DataMap::new());
        // A shared player to control.
        ichoi::db::store::create_player(
            &mut conn,
            &ichoi::db::models::Player {
                id: "player-1".into(),
                kind: "shared".into(),
                output_device_id: None,
                owner_account_id: None,
                name: "Test Speaker".into(),
                name_suffix: None,
            },
        )
        .unwrap();
    }

    // Enqueue a track.
    let enqueue = CommandRequest {
        player_id: "player-1".to_string(),
        command: PlayerCommand::Variant0(CmdEnqueue {
            op: "enqueue".to_string(),
            track_ids: vec!["track-1".to_string()],
            at_index: None,
        }),
    };
    let state = app.control(&common::ctx_anon(), enqueue).expect("enqueue");
    assert_eq!(state.queue.len(), 1);
    assert_eq!(state.queue[0].track_id, "track-1");

    // Play it.
    let play = CommandRequest {
        player_id: "player-1".to_string(),
        command: PlayerCommand::Variant4(CmdPlay {
            op: "play".to_string(),
            index: None,
        }),
    };
    let state = app.control(&common::ctx_anon(), play).expect("play");
    assert!(matches!(state.status, PlayerStatus::Playing));
    assert_eq!(state.current_index, Some(0));
}
