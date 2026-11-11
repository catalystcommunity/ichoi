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

#[test]
fn moderation_operations_are_reachable_through_the_wire_dispatcher() {
    use ichoi::transport::dispatch;
    use libichoi::csil::codec::*;
    let (app, pool) = common::test_app();
    {
        let mut conn = pool.get().unwrap();
        seed_targets(&mut conn);
        seed_account(&mut conn, "admin@example.com");
        store::set_role(&mut conn, "admin@example.com", "admin").unwrap();
    }
    let report = app
        .report_content(
            &common::ctx_user("reporter@example.com"),
            ReportContentRequest {
                target_type: ContentReportTargetType::Playlist,
                target_id: "playlist:one".into(),
                reason: ContentReportReason::Spam,
                details: None,
            },
        )
        .unwrap();
    let admin = common::ctx_admin("admin@example.com");
    let page = encode_page(&Page {
        offset: None,
        limit: None,
    });
    let bytes = dispatch(&app, &admin, "admin", "list-content-reports", &page).unwrap();
    assert_eq!(
        decode_list_content_reports_response(&bytes)
            .unwrap()
            .reports
            .len(),
        1
    );
    let update = UpdateContentReportStatusRequest {
        report_id: report.id,
        status: ContentReportStatus::Resolved,
    };
    let bytes = dispatch(
        &app,
        &admin,
        "admin",
        "update-content-report-status",
        &encode_update_content_report_status_request(&update),
    )
    .unwrap();
    assert!(matches!(
        decode_content_report(&bytes).unwrap().status,
        ContentReportStatus::Resolved
    ));
    assert_eq!(
        dispatch(
            &app,
            &common::ctx_user("reporter@example.com"),
            "admin",
            "list-content-reports",
            &page
        )
        .unwrap_err()
        .code,
        403
    );
    // These Settings operations also need explicit dispatch because both traits use delete_account.
    let deletion = dispatch(
        &app,
        &common::ctx_anon(),
        "session",
        "delete-account",
        &encode_delete_account_request(&DeleteAccountRequest {
            confirmation_handle: "target".into(),
        }),
    )
    .unwrap_err();
    assert_eq!(deletion.code, 401);
    let deletion = dispatch(
        &app,
        &common::ctx_user("reporter@example.com"),
        "admin",
        "delete-account",
        &encode_admin_delete_account_request(&AdminDeleteAccountRequest {
            account_id: "target@example.com".into(),
            confirmation_handle: "target".into(),
        }),
    )
    .unwrap_err();
    assert_eq!(deletion.code, 403);
}
