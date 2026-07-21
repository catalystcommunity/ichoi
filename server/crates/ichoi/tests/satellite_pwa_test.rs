//! Browser-satellite credentials and output ACLs against the real SQLite schema.

mod common;

use ichoi::db::{models, store};
use libichoi::csil::services::{AdminService, NodeService, PlayerService, SessionService};
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
