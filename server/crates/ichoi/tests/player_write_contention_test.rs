//! Player commands and output reports write the same player row at the same time.
//!
//! This test uses a real WAL database file with a multi-connection pool, not the
//! single-connection rolled-back test pool, because the failure needs two connections.
//! With DEFERRED transactions, a command that read the state and then tried to write it
//! after an output report committed failed with "database is locked" (busy_timeout does not
//! apply to that case). The Android app hit this: it sends one command per track while the
//! satellite reports its state.

mod common;

use common::DataMap;
use libichoi::csil::services::PlayerService;
use libichoi::csil::types::*;
use std::sync::Arc;

const ROUNDS: usize = 150;

#[test]
fn commands_and_output_reports_on_one_player_do_not_lock_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ichoi.db");
    let pool = ichoi::db::establish_pool(path.to_str().unwrap()).unwrap();
    {
        let mut conn = pool.get().unwrap();
        ichoi::db::run_migrations(&mut conn).unwrap();
        common::create_artist(&mut conn, &DataMap::new());
        common::create_album(&mut conn, &DataMap::new());
        common::create_track(&mut conn, &DataMap::new());
        ichoi::db::store::create_player(
            &mut conn,
            &ichoi::db::models::Player {
                id: "player-1".into(),
                kind: "shared".into(),
                output_device_id: None,
                owner_account_id: None,
                name: "player-1".into(),
                name_suffix: None,
            },
        )
        .unwrap();
    }
    let app = ichoi::handlers::App::new(pool, Arc::new(common::test_config()));

    let playing = app
        .control(
            &common::ctx_anon(),
            CommandRequest {
                player_id: "player-1".into(),
                command: PlayerCommand::Variant8(CmdReplaceAndPlay {
                    op: "replace-and-play".into(),
                    track_ids: vec!["track-1".into()],
                    start_index: Some(0),
                    position_ms: Some(0),
                }),
            },
        )
        .expect("start playback");
    let playback_id = playing.playback_id.clone().expect("playback id");
    let queue_item_id = playing.queue[0].queue_item_id;

    let (command_failures, report_failures) = std::thread::scope(|scope| {
        let commands = scope.spawn(|| {
            (0..ROUNDS)
                .filter_map(|_| {
                    app.control(
                        &common::ctx_anon(),
                        CommandRequest {
                            player_id: "player-1".into(),
                            command: PlayerCommand::Variant0(CmdEnqueue {
                                op: "enqueue".into(),
                                track_ids: vec!["track-1".into()],
                                at_index: None,
                            }),
                        },
                    )
                    .err()
                    .map(|error| error.message)
                })
                .collect::<Vec<_>>()
        });
        let reports = scope.spawn(|| {
            (0..ROUNDS)
                .filter_map(|round| {
                    app.record_node_report(NodeReport {
                        player_id: "player-1".into(),
                        event: Some(NodeEvent::State),
                        status: PlayerStatus::Playing,
                        queue_item_id: Some(queue_item_id),
                        playback_id: Some(playback_id.clone()),
                        position_ms: Some(round as u64 * 100),
                        error: None,
                        audio_blocked: None,
                    })
                    .err()
                    .map(|error| error.message)
                })
                .collect::<Vec<_>>()
        });
        (commands.join().unwrap(), reports.join().unwrap())
    });

    assert!(
        command_failures.is_empty(),
        "{} of {ROUNDS} commands failed, first: {:?}",
        command_failures.len(),
        command_failures.first()
    );
    assert!(
        report_failures.is_empty(),
        "{} of {ROUNDS} reports failed, first: {:?}",
        report_failures.len(),
        report_failures.first()
    );
    let state = app
        .get_state(
            &common::ctx_anon(),
            SubscribeRequest {
                player_id: "player-1".into(),
                active: None,
            },
        )
        .unwrap();
    assert_eq!(state.queue.len(), ROUNDS + 1, "every enqueue is kept");
}
