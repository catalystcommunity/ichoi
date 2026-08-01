//! Browser-satellite credentials and output ACLs against the real SQLite schema.

mod common;

use ichoi::db::{models, store};
use libichoi::csil::services::{
    AdminService, LibraryService, NodeService, PlayerService, SessionService,
};
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

#[test]
fn loginless_guest_can_administer_but_a_member_cannot() {
    let (app, pool) = common::test_app();
    let guest = app
        .whoami(
            &common::ctx_anon(),
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(guest.can_admin);

    app.create_node_token(
        &common::ctx_anon(),
        CreateNodeTokenRequest {
            label: Some("Kitchen".into()),
            default_enabled: true,
            default_group_ids: vec!["everyone".into()],
        },
    )
    .expect("guest instance permits administration");

    {
        let mut conn = pool.get().unwrap();
        seed_account(&mut conn, "member@example.com", "member");
    }
    let error = app
        .create_node_token(
            &common::ctx_user("member@example.com"),
            CreateNodeTokenRequest {
                label: Some("Bedroom".into()),
                default_enabled: true,
                default_group_ids: vec!["everyone".into()],
            },
        )
        .expect_err("ordinary members cannot create satellite credentials");
    assert_eq!(error.code, 403);
}

#[test]
fn satellite_defaults_flow_to_new_outputs_and_groups_filter_players() {
    let (app, pool) = common::test_app();
    let group = app
        .create_group(
            &common::ctx_anon(),
            CreateGroupRequest {
                name: "Household".into(),
            },
        )
        .unwrap();
    let token = app
        .create_node_token(
            &common::ctx_anon(),
            CreateNodeTokenRequest {
                label: Some("Kitchen Chromebook".into()),
                default_enabled: true,
                default_group_ids: vec![group.id.clone()],
            },
        )
        .unwrap();

    {
        let mut conn = pool.get().unwrap();
        seed_account(&mut conn, "allowed@example.com", "member");
        seed_account(&mut conn, "outside@example.com", "member");
    }
    app.set_group_members(
        &common::ctx_admin("admin@example.com"),
        SetGroupMembersRequest {
            group_id: group.id.clone(),
            member_account_ids: vec!["allowed@example.com".into()],
        },
    )
    .unwrap();

    let registered = app
        .register(
            &common::ctx_node(&token.satellite.id),
            RegisterNodeRequest {
                hostname: "chromebook-pwa".into(),
                platform: "chromeos".into(),
                arch: "x86_64".into(),
                outputs: vec![AudioOutput {
                    os_device_id: "default".into(),
                    friendly_name: Some("HDMI".into()),
                    channels: 2,
                    sample_rates: vec![48_000],
                    is_default: true,
                }],
            },
        )
        .unwrap();
    assert_eq!(registered.players.len(), 1);
    assert_eq!(registered.players[0].name, "Kitchen Chromebook · HDMI");

    let player_id = registered.players[0].id.clone();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    app.nodes.subscribe(player_id.clone(), 42, tx);

    let visible = app
        .list_players(
            &common::ctx_user("allowed@example.com"),
            ListPlayersRequest {
                kind: Some(PlayerKind::Shared),
            },
        )
        .unwrap();
    assert_eq!(visible.players.len(), 1);
    let hidden = app
        .list_players(
            &common::ctx_user("outside@example.com"),
            ListPlayersRequest {
                kind: Some(PlayerKind::Shared),
            },
        )
        .unwrap();
    assert!(hidden.players.is_empty());

    let device_id = registered.players[0].device_id.clone().unwrap();
    app.set_device_access(
        &common::ctx_admin("admin@example.com"),
        SetDeviceAccessRequest {
            device_id,
            enabled: false,
            group_ids: vec![group.id],
        },
    )
    .unwrap();
    let disabled = app
        .list_players(
            &common::ctx_user("allowed@example.com"),
            ListPlayersRequest {
                kind: Some(PlayerKind::Shared),
            },
        )
        .unwrap();
    assert!(
        disabled.players.is_empty(),
        "disabled outputs show up for nobody"
    );
}

#[test]
fn a_satellite_that_cannot_make_sound_says_so_to_every_controller() {
    let (app, _pool) = common::test_app();
    let token = app
        .create_node_token(
            &common::ctx_anon(),
            CreateNodeTokenRequest {
                label: Some("Kitchen Chromebook".into()),
                default_enabled: true,
                default_group_ids: vec!["everyone".into()],
            },
        )
        .unwrap();
    let node_ctx = common::ctx_node(&token.satellite.id);
    let registered = app
        .register(
            &node_ctx,
            RegisterNodeRequest {
                hostname: "chromebook-pwa".into(),
                platform: "chromeos".into(),
                arch: "x86_64".into(),
                outputs: vec![AudioOutput {
                    os_device_id: "default".into(),
                    friendly_name: Some("HDMI".into()),
                    channels: 2,
                    sample_rates: vec![48_000],
                    is_default: true,
                }],
            },
        )
        .unwrap();
    let player_id = registered.players[0].id.clone();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    app.nodes.subscribe(player_id.clone(), 42, tx);

    let blocked_for_controller = || {
        app.list_players(
            &common::ctx_anon(),
            ListPlayersRequest {
                kind: Some(PlayerKind::Shared),
            },
        )
        .unwrap()
        .players
        .into_iter()
        .find(|player| player.id == player_id)
        .expect("the registered output is listed")
        .audio_blocked
    };

    assert_eq!(
        blocked_for_controller(),
        None,
        "an output that has not reported is not accused of being blocked"
    );

    // The browser satellite loads, finds the autoplay policy against it, and says so.
    app.session(
        &node_ctx,
        NodeReport {
            player_id: player_id.clone(),
            status: PlayerStatus::Stopped,
            position_ms: None,
            audio_blocked: Some(true),
        },
    )
    .unwrap();
    assert_eq!(blocked_for_controller(), Some(true));

    // Somebody touches the satellite; its next report clears the warning everywhere.
    app.session(
        &node_ctx,
        NodeReport {
            player_id: player_id.clone(),
            status: PlayerStatus::Playing,
            position_ms: Some(1_000),
            audio_blocked: Some(false),
        },
    )
    .unwrap();
    assert_eq!(blocked_for_controller(), None);
}

#[test]
fn satellite_is_a_named_session_confined_to_its_own_player_and_queue() {
    let (app, pool) = common::test_app();
    let kitchen = app
        .create_node_token(
            &common::ctx_anon(),
            CreateNodeTokenRequest {
                label: Some("Kitchen Chromebook".into()),
                default_enabled: true,
                default_group_ids: vec!["everyone".into()],
            },
        )
        .unwrap();
    let bedroom = app
        .create_node_token(
            &common::ctx_anon(),
            CreateNodeTokenRequest {
                label: Some("Bedroom".into()),
                default_enabled: true,
                default_group_ids: vec!["everyone".into()],
            },
        )
        .unwrap();

    let register = |satellite_id: &str, output: &str| {
        app.register(
            &common::ctx_node(satellite_id),
            RegisterNodeRequest {
                hostname: "browser-pwa".into(),
                platform: "chromeos".into(),
                arch: "browser".into(),
                outputs: vec![AudioOutput {
                    os_device_id: output.into(),
                    friendly_name: Some(output.into()),
                    channels: 2,
                    sample_rates: vec![48_000],
                    is_default: true,
                }],
            },
        )
        .unwrap()
        .players
        .remove(0)
    };
    let kitchen_player = register(&kitchen.satellite.id, "Kitchen speakers");
    let bedroom_player = register(&bedroom.satellite.id, "Bedroom speakers");

    {
        let mut conn = pool.get().unwrap();
        seed_account(&mut conn, "person@example.com", "member");
        common::create_artist(&mut conn, &common::DataMap::new());
        common::create_album(&mut conn, &common::DataMap::new());
        common::create_track(&mut conn, &common::DataMap::new());
        store::create_player(
            &mut conn,
            &models::Player {
                id: "private:person".into(),
                kind: "private".into(),
                output_device_id: None,
                owner_account_id: Some("person@example.com".into()),
                name: "Person's private player".into(),
                name_suffix: None,
            },
        )
        .unwrap();
    }

    let identity = common::ctx_node(&kitchen.satellite.id);
    let session = app
        .whoami(
            &identity,
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(session.handle, "Kitchen Chromebook");
    assert_eq!(session.display_name.as_deref(), Some("Kitchen Chromebook"));
    assert_eq!(
        session.account_id,
        format!("satellite:{}", kitchen.satellite.id)
    );
    assert!(!session.can_admin);

    let library = app
        .list_albums(
            &identity,
            BrowseRequest {
                library: Some(Library::Music),
                offset: None,
                limit: None,
            },
        )
        .expect("satellite identity may browse songs");
    assert_eq!(library.total, 1);

    let visible = app
        .list_players(&identity, ListPlayersRequest { kind: None })
        .unwrap();
    assert_eq!(
        visible
            .players
            .iter()
            .map(|player| player.id.as_str())
            .collect::<Vec<_>>(),
        vec![kitchen_player.id.as_str()]
    );

    app.control(
        &identity,
        CommandRequest {
            player_id: kitchen_player.id.clone(),
            command: PlayerCommand::Variant0(CmdEnqueue {
                op: "enqueue".into(),
                track_ids: vec!["track-1".into()],
                at_index: None,
            }),
        },
    )
    .expect("satellite may manipulate its own queue even when accounts exist");
    let playing = app
        .control(
            &identity,
            CommandRequest {
                player_id: kitchen_player.id,
                command: PlayerCommand::Variant4(CmdPlay {
                    op: "play".into(),
                    index: Some(0),
                }),
            },
        )
        .expect("satellite may play its own queue");
    assert_eq!(playing.status, PlayerStatus::Playing);
    assert_eq!(playing.queue[0].track_id, "track-1");

    let other_error = app
        .control(
            &identity,
            CommandRequest {
                player_id: bedroom_player.id,
                command: PlayerCommand::Variant3(CmdClear { op: "clear".into() }),
            },
        )
        .expect_err("satellite must not control another satellite");
    assert_eq!(other_error.code, 403);

    let private_error = app
        .control(
            &identity,
            CommandRequest {
                player_id: "private:person".into(),
                command: PlayerCommand::Variant3(CmdClear { op: "clear".into() }),
            },
        )
        .expect_err("satellite must not control a private player");
    assert_eq!(private_error.code, 403);
}

#[test]
fn revoking_a_satellite_token_removes_it_from_authentication_lookup() {
    let (app, pool) = common::test_app();
    let result = app
        .create_node_token(
            &common::ctx_anon(),
            CreateNodeTokenRequest {
                label: Some("Portable".into()),
                default_enabled: true,
                default_group_ids: vec!["everyone".into()],
            },
        )
        .unwrap();
    {
        let mut conn = pool.get().unwrap();
        assert!(
            store::satellite_for_hash(&mut conn, &ichoi::auth::sha256_hex(&result.token))
                .unwrap()
                .is_some()
        );
    }
    app.revoke_satellite_token(
        &common::ctx_anon(),
        RevokeSatelliteTokenRequest {
            satellite_id: result.satellite.id,
        },
    )
    .unwrap();
    let mut conn = pool.get().unwrap();
    assert!(
        store::satellite_for_hash(&mut conn, &ichoi::auth::sha256_hex(&result.token))
            .unwrap()
            .is_none()
    );
}
