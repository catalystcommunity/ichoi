//! Library handlers exercised end-to-end against a real (rolled-back) database.

mod common;

use common::DataMap;
use ichoi::transport::decode_event_envelope;
use libichoi::csil::codec::decode_player_state;
use libichoi::csil::services::{LibraryService, NodeService, PlayerService};
use libichoi::csil::types::*;
use libichoi::csil_channel::decode_node_directive;
use std::sync::Arc;

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
    assert_eq!(resp.albums[0].artist_name.as_deref(), Some("Test Artist"));
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
                library: None,
                limit: None,
            },
        )
        .expect("search");

    assert_eq!(resp.tracks.len(), 1);
    assert_eq!(resp.tracks[0].title, "Rare Gem");
    assert_eq!(resp.tracks[0].artist_name.as_deref(), Some("Test Artist"));
    assert_eq!(resp.tracks[0].album_title.as_deref(), Some("Test Album"));
}

#[test]
fn search_includes_albums_by_a_matching_artist() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        let mut artist = DataMap::new();
        artist.insert("name".into(), "Madonna".into());
        common::create_artist(&mut conn, &artist);
        let mut album = DataMap::new();
        album.insert("title".into(), "Ray of Light".into());
        common::create_album(&mut conn, &album);
        common::create_track(&mut conn, &DataMap::new());
    }

    let resp = app
        .search(
            &common::ctx_anon(),
            SearchRequest {
                query: "Madonna".to_string(),
                library: None,
                limit: None,
            },
        )
        .expect("search");

    assert_eq!(resp.albums.len(), 1);
    assert_eq!(resp.albums[0].title, "Ray of Light");
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
fn zero_account_guest_access_flag_gates_guest_control_of_native_outputs() {
    let pause = |player_id: &str| CommandRequest {
        player_id: player_id.into(),
        command: PlayerCommand::Variant9(CmdPause { op: "pause".into() }),
    };
    let seed = |pool: &ichoi::db::SqlitePool| {
        let mut conn = pool.get().unwrap();
        ichoi::db::store::ensure_core_node(&mut conn, "testhost").unwrap();
        ichoi::db::store::sync_core_outputs(
            &mut conn,
            "testhost",
            &[ichoi::audio::AudioOutput {
                os_device_id: "speakers".into(),
                friendly_name: "Living room".into(),
                channels: 2,
                sample_rates: vec![48_000],
                is_default: true,
            }],
        )
        .unwrap();
        // An ownerless browser share left from an earlier guest session.
        ichoi::db::store::create_player(
            &mut conn,
            &ichoi::db::models::Player {
                id: "share:guest:Old".into(),
                kind: "shared".into(),
                output_device_id: None,
                owner_account_id: None,
                name: "Guest Old".into(),
                name_suffix: Some("Old".into()),
            },
        )
        .unwrap();
    };
    let native = "player:core:testhost:speakers";

    // Default: a fresh zero-account server lets guests control shared devices.
    let (app, pool) = common::test_app();
    seed(&pool);
    app.control(&common::ctx_anon(), pause(native))
        .expect("guest controls native output while elevated guest access is on");

    // With the flag off, guests must wait for the first sign-in.
    let (mut app, pool) = common::test_app();
    let mut config = common::test_config();
    config.zero_account_guest_access = false;
    app.config = Arc::new(config);
    seed(&pool);
    for player_id in [native, "share:guest:Old"] {
        let error = app
            .control(&common::ctx_anon(), pause(player_id))
            .expect_err("guest control needs the zero-account flag");
        assert_eq!(error.code, 401, "{player_id}");
    }
}

#[test]
fn anonymous_cannot_control_shared_device_once_users_exist() {
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
        ichoi::db::store::create_player(
            &mut conn,
            &ichoi::db::models::Player {
                id: "speaker-1".into(),
                kind: "shared".into(),
                output_device_id: None,
                owner_account_id: None,
                name: "Kitchen".into(),
                name_suffix: None,
            },
        )
        .unwrap();
    }
    let err = app
        .control(
            &common::ctx_anon(),
            CommandRequest {
                player_id: "speaker-1".into(),
                command: PlayerCommand::Variant9(CmdPause { op: "pause".into() }),
            },
        )
        .expect_err("anonymous control is not allowed once accounts exist");
    assert_eq!(err.code, 401);
}

#[test]
fn anonymous_jukebox_commands_do_not_credit_the_previous_signed_in_listener() {
    use diesel::prelude::*;
    use ichoi::db::schema::listens;

    let (mut app, pool) = common::test_app();
    let mut config = common::test_config();
    config.anonymous_jukebox = true;
    app.config = Arc::new(config);
    {
        let mut conn = pool.get().unwrap();
        ichoi::db::store::upsert_account(
            &mut conn,
            &ichoi::db::models::Account {
                id: "alice@example.invalid".into(),
                handle: "alice".into(),
                display_name: None,
                role: "member".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
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
    let alice_listens = || -> i64 {
        listens::table
            .filter(listens::account_id.eq("alice@example.invalid"))
            .count()
            .get_result(&mut pool.get().unwrap())
            .unwrap()
    };
    // The anonymous browser tab is the share's output and reports each completion.
    let complete = |state: &PlayerState, player_id: &str| {
        let item = state.queue[state.current_index.expect("current index") as usize].queue_item_id;
        app.control(
            &common::ctx_anon(),
            CommandRequest {
                player_id: player_id.into(),
                command: PlayerCommand::Variant17(CmdPlaybackCompleted {
                    op: "playback-completed".into(),
                    playback_id: state.playback_id.clone().expect("playback id"),
                    queue_item_id: item,
                }),
            },
        )
        .expect("output reports completion")
    };

    let share = app
        .enable_share(
            &common::ctx_anon(),
            EnableShareRequest {
                suffix: Some("Party".into()),
            },
        )
        .expect("anonymous jukebox share");
    let pid = share.player.id;
    let playing = app
        .control(
            &common::ctx_user("alice@example.invalid"),
            CommandRequest {
                player_id: pid.clone(),
                command: PlayerCommand::Variant8(CmdReplaceAndPlay {
                    op: "replace-and-play".into(),
                    track_ids: vec!["track-1".into(), "track-2".into(), "track-1".into()],
                    start_index: Some(0),
                    position_ms: Some(0),
                }),
            },
        )
        .expect("alice starts the jukebox");
    // Alice's own playback is credited to Alice.
    complete(&playing, &pid);
    assert_eq!(alice_listens(), 1);

    let guest_next = app
        .control(
            &common::ctx_anon(),
            CommandRequest {
                player_id: pid.clone(),
                command: PlayerCommand::Variant10(CmdNext { op: "next".into() }),
            },
        )
        .expect("guest skips");
    complete(&guest_next, &pid);
    assert_eq!(
        alice_listens(),
        1,
        "a guest command must not record listens for the earlier signed-in listener"
    );
    let guest_rows: i64 = listens::table
        .filter(listens::account_id.eq("__guest__"))
        .count()
        .get_result(&mut pool.get().unwrap())
        .unwrap();
    assert_eq!(
        guest_rows, 0,
        "guest listens are not recorded once an account exists"
    );
}

#[test]
fn anonymous_jukebox_allows_ownerless_browser_share_after_an_account_exists() {
    let (mut app, pool) = common::test_app();
    let mut config = common::test_config();
    config.anonymous_jukebox = true;
    app.config = Arc::new(config);
    {
        let mut conn = pool.get().unwrap();
        ichoi::db::store::upsert_account(
            &mut conn,
            &ichoi::db::models::Account {
                id: "admin@example.invalid".into(),
                handle: "admin".into(),
                display_name: None,
                role: "admin".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
    }

    let share = app
        .enable_share(
            &common::ctx_anon(),
            EnableShareRequest {
                suffix: Some("Demo".into()),
            },
        )
        .expect("anonymous jukebox share");
    app.control(
        &common::ctx_anon(),
        CommandRequest {
            player_id: share.player.id.clone(),
            command: PlayerCommand::Variant9(CmdPause { op: "pause".into() }),
        },
    )
    .expect("anonymous jukebox control");
    app.disable_share(
        &common::ctx_anon(),
        DisableShareRequest {
            player_id: share.player.id,
        },
    )
    .expect("anonymous jukebox removal");
}

#[test]
fn anonymous_jukebox_does_not_expose_account_owned_browser_shares() {
    let (mut app, pool) = common::test_app();
    let mut config = common::test_config();
    config.anonymous_jukebox = true;
    app.config = Arc::new(config);
    {
        let mut conn = pool.get().unwrap();
        ichoi::db::store::upsert_account(
            &mut conn,
            &ichoi::db::models::Account {
                id: "member@example.invalid".into(),
                handle: "member".into(),
                display_name: None,
                role: "member".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
    }
    let share = app
        .enable_share(
            &common::ctx_user("member@example.invalid"),
            EnableShareRequest {
                suffix: Some("Private".into()),
            },
        )
        .expect("account-owned share");

    let error = app
        .control(
            &common::ctx_anon(),
            CommandRequest {
                player_id: share.player.id,
                command: PlayerCommand::Variant9(CmdPause { op: "pause".into() }),
            },
        )
        .expect_err("account-owned share must still require login");
    assert_eq!(error.code, 401);
}

#[test]
fn shared_devices_are_listed_only_while_a_connection_owns_them() {
    let (app, _pool) = common::test_app();
    let share = app
        .enable_share(
            &common::ctx_anon(),
            EnableShareRequest {
                suffix: Some("TodPhone".into()),
            },
        )
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
    assert!(
        listed.players.iter().any(|p| p.id == id),
        "a claimed device is live"
    );

    // Re-claiming the same device by the same owner is idempotent (no 409), so a client can
    // re-attach after a refresh instead of failing on its own device.
    let again = app
        .enable_share(
            &common::ctx_anon(),
            EnableShareRequest {
                suffix: Some("TodPhone".into()),
            },
        )
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
    assert!(matches!(state.status, PlayerStatus::Stopped));
    assert_eq!(state.current_index, Some(0));

    // Play it.
    let play = CommandRequest {
        player_id: "player-1".to_string(),
        command: PlayerCommand::Variant7(CmdPlay {
            op: "play".to_string(),
            index: None,
            queue_item_id: None,
        }),
    };
    let state = app.control(&common::ctx_anon(), play).expect("play");
    assert!(matches!(state.status, PlayerStatus::Playing));
    assert_eq!(state.current_index, Some(0));

    let snapshot = app
        .get_state(
            &common::ctx_anon(),
            SubscribeRequest {
                player_id: "player-1".into(),
                active: None,
            },
        )
        .expect("target snapshot");
    assert!(matches!(snapshot.status, PlayerStatus::Playing));
    assert_eq!(snapshot.queue.len(), 1);
    assert_eq!(snapshot.queue[0].track_id, "track-1");
}

#[test]
fn satellite_player_receives_load_directive() {
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        common::create_artist(&mut conn, &DataMap::new());
        common::create_album(&mut conn, &DataMap::new());
        common::create_track(&mut conn, &DataMap::new());
    }

    let registered = app
        .register(
            &common::ctx_node("pending:test-token"),
            RegisterNodeRequest {
                hostname: "kitchenpi".to_string(),
                platform: "linux".to_string(),
                arch: "aarch64".to_string(),
                outputs: vec![AudioOutput {
                    os_device_id: "default".to_string(),
                    friendly_name: Some("Main".to_string()),
                    channels: 2,
                    sample_rates: vec![44_100, 48_000],
                    is_default: true,
                }],
            },
        )
        .expect("register satellite");
    let player_id = registered.players[0].id.clone();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.nodes.subscribe(player_id.clone(), 7, tx);

    app.control(
        &common::ctx_anon(),
        CommandRequest {
            player_id: player_id.clone(),
            command: PlayerCommand::Variant0(CmdEnqueue {
                op: "enqueue".to_string(),
                track_ids: vec!["track-1".to_string()],
                at_index: None,
            }),
        },
    )
    .expect("enqueue");
    app.control(
        &common::ctx_anon(),
        CommandRequest {
            player_id: player_id.clone(),
            command: PlayerCommand::Variant7(CmdPlay {
                op: "play".to_string(),
                index: None,
                queue_item_id: None,
            }),
        },
    )
    .expect("play");

    let payload = rx.try_recv().expect("directive pushed");
    let directive = decode_node_directive(&payload).expect("decode directive");
    let NodeDirective::Variant0(load) = directive else {
        panic!("expected load directive")
    };
    assert_eq!(load.track_id, "track-1");
    assert!(matches!(
        load.pref.transcode_codec,
        Some(TranscodeCodec::Aac)
    ));

    let state = app
        .control(
            &common::ctx_anon(),
            CommandRequest {
                player_id,
                command: PlayerCommand::Variant13(CmdVolume {
                    op: "volume".into(),
                    volume: 37,
                }),
            },
        )
        .expect("set satellite volume");
    assert_eq!(state.volume, 37);

    let payload = rx.try_recv().expect("volume directive pushed");
    let directive = decode_node_directive(&payload).expect("decode volume directive");
    let NodeDirective::Variant4(volume) = directive else {
        panic!("expected volume directive")
    };
    assert_eq!(volume.volume, 37);
}

#[test]
fn output_completion_advances_one_player_and_pushes_every_subscriber() {
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
        for player_id in ["player-1", "player-2"] {
            ichoi::db::store::create_player(
                &mut conn,
                &ichoi::db::models::Player {
                    id: player_id.into(),
                    kind: "shared".into(),
                    output_device_id: None,
                    owner_account_id: None,
                    name: player_id.into(),
                    name_suffix: None,
                },
            )
            .unwrap();
        }
    }

    let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
    let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
    let (other_tx, mut other_rx) = tokio::sync::mpsc::unbounded_channel();
    app.subs.subscribe("player-1".into(), 1, first_tx);
    app.subs.subscribe("player-1".into(), 2, second_tx);
    app.subs.subscribe("player-2".into(), 3, other_tx);

    let playing = app
        .control(
            &common::ctx_anon(),
            CommandRequest {
                player_id: "player-1".into(),
                command: PlayerCommand::Variant8(CmdReplaceAndPlay {
                    op: "replace-and-play".into(),
                    track_ids: vec!["track-1".into(), "track-2".into()],
                    start_index: Some(0),
                    position_ms: Some(0),
                }),
            },
        )
        .expect("start player one");
    let original_revision = playing.revision;
    let original_playback = playing.playback_id.clone().expect("playback id");
    let original_item = playing.queue[0].queue_item_id;

    for receiver in [&mut first_rx, &mut second_rx] {
        let frame = receiver.try_recv().expect("state pushed to subscriber");
        let envelope = decode_event_envelope(&frame).expect("decode state envelope");
        let pushed = decode_player_state(&envelope.payload).expect("decode player state");
        assert_eq!(pushed.revision, original_revision);
    }
    assert!(
        other_rx.try_recv().is_err(),
        "player two must not receive player one state"
    );

    let uncorrelated = app
        .record_node_report(NodeReport {
            player_id: "player-1".into(),
            event: Some(NodeEvent::State),
            status: PlayerStatus::Stopped,
            queue_item_id: None,
            playback_id: None,
            position_ms: None,
            error: None,
            audio_blocked: Some(true),
        })
        .expect("ignore uncorrelated output state");
    assert!(matches!(uncorrelated.status, PlayerStatus::Playing));
    assert_eq!(uncorrelated.revision, original_revision);
    for receiver in [&mut first_rx, &mut second_rx] {
        let frame = receiver
            .try_recv()
            .expect("health state pushed to subscriber");
        let envelope = decode_event_envelope(&frame).expect("decode health state envelope");
        let pushed = decode_player_state(&envelope.payload).expect("decode health player state");
        assert_eq!(pushed.revision, original_revision);
        assert!(matches!(pushed.status, PlayerStatus::Playing));
    }

    let advanced = app
        .record_node_report(NodeReport {
            player_id: "player-1".into(),
            event: Some(NodeEvent::Completed),
            status: PlayerStatus::Stopped,
            queue_item_id: Some(original_item),
            playback_id: Some(original_playback.clone()),
            position_ms: None,
            error: None,
            audio_blocked: None,
        })
        .expect("complete first item");
    assert!(matches!(advanced.status, PlayerStatus::Playing));
    assert_eq!(advanced.current_index, Some(1));
    assert_ne!(
        advanced.playback_id.as_deref(),
        Some(original_playback.as_str())
    );
    assert!(advanced.revision > original_revision);

    for receiver in [&mut first_rx, &mut second_rx] {
        let frame = receiver.try_recv().expect("advance pushed to subscriber");
        let envelope = decode_event_envelope(&frame).expect("decode advance envelope");
        let pushed = decode_player_state(&envelope.payload).expect("decode advanced state");
        assert_eq!(pushed.current_index, Some(1));
        assert_eq!(pushed.revision, advanced.revision);
    }
    assert!(
        other_rx.try_recv().is_err(),
        "player two must remain independent"
    );

    let stale = app
        .record_node_report(NodeReport {
            player_id: "player-1".into(),
            event: Some(NodeEvent::Completed),
            status: PlayerStatus::Stopped,
            queue_item_id: Some(original_item),
            playback_id: Some(original_playback),
            position_ms: None,
            error: None,
            audio_blocked: None,
        })
        .expect("ignore stale completion");
    assert_eq!(stale.revision, advanced.revision);
    assert_eq!(stale.current_index, Some(1));

    let other = app
        .get_state(
            &common::ctx_anon(),
            SubscribeRequest {
                player_id: "player-2".into(),
                active: None,
            },
        )
        .expect("load player two");
    assert!(other.queue.is_empty());
    assert_eq!(other.revision, 0);
}
