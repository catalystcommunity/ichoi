mod common;

use ichoi::db::{models, store};
use libichoi::csil::services::AdminService;
use libichoi::csil::types::*;

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

fn seed(pool: &ichoi::db::SqlitePool) {
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    seed_account(&mut conn, "member@example.com", "member");
    for id in ["01", "02", "03"] {
        store::insert_content_report(
            &mut conn,
            &models::ContentReport {
                id: id.into(),
                reporter_account_id: "member@example.com".into(),
                target_type: "account".into(),
                target_id: "admin@example.com".into(),
                reason: "other".into(),
                details: None,
                status: "open".into(),
                created_at: format!("2026-09-12T00:00:{id}Z"),
                resolved_at: None,
            },
        )
        .unwrap();
    }
}

#[test]
fn admin_lists_pages_and_changes_every_status() {
    let (app, pool) = common::test_app();
    seed(&pool);
    let page = app
        .list_content_reports(
            &common::ctx_admin("admin@example.com"),
            Page {
                offset: Some(1),
                limit: Some(1),
            },
        )
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.reports[0].id, "02");

    for status in [
        ContentReportStatus::Resolved,
        ContentReportStatus::Dismissed,
        ContentReportStatus::Open,
    ] {
        let updated = app
            .update_content_report_status(
                &common::ctx_admin("admin@example.com"),
                UpdateContentReportStatusRequest {
                    report_id: "02".into(),
                    status,
                },
            )
            .unwrap();
        if matches!(updated.status, ContentReportStatus::Open) {
            assert!(updated.resolved_at.is_none());
        } else {
            assert!(updated.resolved_at.is_some());
        }
    }
}

#[test]
fn report_administration_rejects_missing_member_and_anonymous_calls() {
    let (app, pool) = common::test_app();
    seed(&pool);
    let page = Page {
        offset: None,
        limit: None,
    };
    assert_eq!(
        app.list_content_reports(&common::ctx_user("member@example.com"), page.clone())
            .unwrap_err()
            .code,
        403
    );
    assert_eq!(
        app.list_content_reports(&common::ctx_anon(), page)
            .unwrap_err()
            .code,
        401
    );
    assert_eq!(
        app.update_content_report_status(
            &common::ctx_admin("admin@example.com"),
            UpdateContentReportStatusRequest {
                report_id: "missing".into(),
                status: ContentReportStatus::Resolved,
            },
        )
        .unwrap_err()
        .code,
        404
    );
}
