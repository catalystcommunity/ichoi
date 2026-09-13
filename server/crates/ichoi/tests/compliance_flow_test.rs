mod common;

use std::sync::Arc;

use ichoi::db::{models, store};
use libichoi::csil::services::{AdminService, LibraryService};
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

#[test]
fn report_review_playlist_deletion_and_account_deletion_complete_end_to_end() {
    let (mut app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(music.path().join("playlists")).unwrap();
    std::fs::write(music.path().join("playlists/reported.m3u"), "playlist").unwrap();
    let mut config = common::test_config();
    config.music_dir = Some(music.path().to_owned());
    app.config = Arc::new(config);
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    seed_account(&mut conn, "reporter@example.com", "member");
    seed_account(&mut conn, "owner@example.com", "member");
    store::upsert_playlist(
        &mut conn,
        &models::Playlist {
            id: "playlist:reported".into(),
            name: "Reported".into(),
            owner: Some("owner@example.com".into()),
            root_relative_path: "playlists/reported.m3u".into(),
            visibility: "public".into(),
        },
    )
    .unwrap();
    drop(conn);

    let report = app
        .report_content(
            &common::ctx_user("reporter@example.com"),
            ReportContentRequest {
                target_type: ContentReportTargetType::Playlist,
                target_id: "playlist:reported".into(),
                reason: ContentReportReason::ObjectionableContent,
                details: Some("Review this playlist".into()),
            },
        )
        .unwrap();
    let queue = app
        .list_content_reports(
            &common::ctx_admin("admin@example.com"),
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(queue.reports.len(), 1);
    app.update_content_report_status(
        &common::ctx_admin("admin@example.com"),
        UpdateContentReportStatusRequest {
            report_id: report.id,
            status: ContentReportStatus::Resolved,
        },
    )
    .unwrap();
    app.delete_playlist(
        &common::ctx_admin("admin@example.com"),
        DeletePlaylistRequest {
            playlist_id: "playlist:reported".into(),
        },
    )
    .unwrap();
    AdminService::delete_account(
        &app,
        &common::ctx_admin("admin@example.com"),
        AdminDeleteAccountRequest {
            account_id: "owner@example.com".into(),
            confirmation_handle: "owner".into(),
        },
    )
    .unwrap();

    let mut conn = pool.get().unwrap();
    assert!(store::get_playlist(&mut conn, "playlist:reported")
        .unwrap()
        .is_none());
    assert!(store::get_account(&mut conn, "owner@example.com")
        .unwrap()
        .is_none());
    assert!(store::list_content_reports(&mut conn, 0, 10)
        .unwrap()
        .is_empty());
}
