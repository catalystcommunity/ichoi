mod common;

use ichoi::db::{models, store};
use libichoi::csil::services::LibraryService;
use libichoi::csil::types::*;

fn seed_account(conn: &mut diesel::SqliteConnection, id: &str) {
    store::upsert_account(
        conn,
        &models::Account {
            id: id.into(),
            handle: id.split('@').next().unwrap().into(),
            display_name: None,
            role: "member".into(),
            created_at: "2026-09-12T00:00:00Z".into(),
        },
    )
    .unwrap();
}

fn seed_targets(conn: &mut diesel::SqliteConnection) {
    seed_account(conn, "reporter@example.com");
    seed_account(conn, "target@example.com");
    store::upsert_playlist(
        conn,
        &models::Playlist {
            id: "playlist:one".into(),
            name: "One".into(),
            owner: Some("target@example.com".into()),
            root_relative_path: "playlists/one.m3u".into(),
            visibility: "public".into(),
        },
    )
    .unwrap();
}

#[test]
fn stores_every_reason_for_both_target_types() {
    let (app, pool) = common::test_app();
    seed_targets(&mut pool.get().unwrap());
    let reasons = [
        ContentReportReason::ObjectionableContent,
        ContentReportReason::Harassment,
        ContentReportReason::Spam,
        ContentReportReason::Other,
    ];
    for (index, reason) in reasons.into_iter().enumerate() {
        let (target_type, target_id) = if index % 2 == 0 {
            (ContentReportTargetType::Playlist, "playlist:one")
        } else {
            (ContentReportTargetType::Account, "target@example.com")
        };
        let stored = app
            .report_content(
                &common::ctx_user("reporter@example.com"),
                ReportContentRequest {
                    target_type,
                    target_id: target_id.into(),
                    reason,
                    details: Some("Review this item".into()),
                },
            )
            .unwrap();
        assert_eq!(stored.reporter_account_id, "reporter@example.com");
        assert!(matches!(stored.status, ContentReportStatus::Open));
    }
    assert_eq!(
        store::list_content_reports(&mut pool.get().unwrap(), 0, 10)
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn rejects_anonymous_missing_targets_and_long_details() {
    let (app, pool) = common::test_app();
    seed_targets(&mut pool.get().unwrap());
    let request = || ReportContentRequest {
        target_type: ContentReportTargetType::Playlist,
        target_id: "playlist:one".into(),
        reason: ContentReportReason::Spam,
        details: None,
    };
    assert_eq!(
        app.report_content(&common::ctx_anon(), request())
            .unwrap_err()
            .code,
        401
    );
    let mut missing_playlist = request();
    missing_playlist.target_id = "playlist:missing".into();
    assert_eq!(
        app.report_content(&common::ctx_user("reporter@example.com"), missing_playlist)
            .unwrap_err()
            .code,
        404
    );
    let missing_account = ReportContentRequest {
        target_type: ContentReportTargetType::Account,
        target_id: "missing@example.com".into(),
        reason: ContentReportReason::Other,
        details: None,
    };
    assert_eq!(
        app.report_content(&common::ctx_user("reporter@example.com"), missing_account)
            .unwrap_err()
            .code,
        404
    );
    let mut too_long = request();
    too_long.details = Some("é".repeat(2_001));
    assert_eq!(
        app.report_content(&common::ctx_user("reporter@example.com"), too_long)
            .unwrap_err()
            .code,
        400
    );
    let mut exact_limit = request();
    exact_limit.details = Some("é".repeat(2_000));
    assert!(app
        .report_content(&common::ctx_user("reporter@example.com"), exact_limit)
        .is_ok());
}
