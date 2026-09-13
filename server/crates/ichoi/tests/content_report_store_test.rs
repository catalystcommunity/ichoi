mod common;

use diesel::prelude::*;
use ichoi::db::{models, schema, store};

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

fn report(id: &str, reporter: &str) -> models::ContentReport {
    models::ContentReport {
        id: id.into(),
        reporter_account_id: reporter.into(),
        target_type: "playlist".into(),
        target_id: "playlist:one".into(),
        reason: "spam".into(),
        details: Some("Repeated playlist".into()),
        status: "open".into(),
        created_at: format!("2026-09-12T00:00:{id}Z"),
        resolved_at: None,
    }
}

#[test]
fn inserts_pages_updates_and_deletes_with_reporter() {
    let (_app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "reporter@example.com");
    for id in ["01", "02", "03"] {
        store::insert_content_report(&mut conn, &report(id, "reporter@example.com")).unwrap();
    }

    let page = store::list_content_reports(&mut conn, 1, 1).unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, "02");

    assert_eq!(
        store::update_content_report_status(
            &mut conn,
            "02",
            "resolved",
            Some("2026-09-12T01:00:00Z"),
        )
        .unwrap(),
        1
    );
    let updated = store::get_content_report(&mut conn, "02").unwrap().unwrap();
    assert_eq!(updated.status, "resolved");
    assert_eq!(updated.resolved_at.as_deref(), Some("2026-09-12T01:00:00Z"));

    diesel::delete(schema::accounts::table.find("reporter@example.com"))
        .execute(&mut conn)
        .unwrap();
    assert!(store::list_content_reports(&mut conn, 0, 10)
        .unwrap()
        .is_empty());
}

#[test]
fn database_rejects_invalid_target_reason_and_status_values() {
    let (_app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "reporter@example.com");

    for (id, target_type, reason, status) in [
        ("bad-target", "track", "spam", "open"),
        ("bad-reason", "playlist", "copyright", "open"),
        ("bad-status", "account", "other", "pending"),
    ] {
        let mut row = report(id, "reporter@example.com");
        row.target_type = target_type.into();
        row.reason = reason.into();
        row.status = status.into();
        assert!(store::insert_content_report(&mut conn, &row).is_err());
    }
}
