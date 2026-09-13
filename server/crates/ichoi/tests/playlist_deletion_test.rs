mod common;

use std::sync::Arc;

use diesel::connection::SimpleConnection;
use ichoi::db::{models, store};
use libichoi::csil::services::LibraryService;
use libichoi::csil::types::DeletePlaylistRequest;

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

fn configured_app() -> (
    ichoi::handlers::App,
    ichoi::db::SqlitePool,
    tempfile::TempDir,
) {
    let (mut app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(music.path().join("playlists")).unwrap();
    let mut config = common::test_config();
    config.music_dir = Some(music.path().to_owned());
    app.config = Arc::new(config);
    (app, pool, music)
}

fn seed_playlist(conn: &mut diesel::SqliteConnection, id: &str, owner: &str, relative: &str) {
    store::upsert_playlist(
        conn,
        &models::Playlist {
            id: id.into(),
            name: id.into(),
            owner: Some(owner.into()),
            root_relative_path: relative.into(),
            visibility: "public".into(),
        },
    )
    .unwrap();
}

#[test]
fn owner_and_admin_delete_playlists_and_target_reports() {
    let (app, pool, music) = configured_app();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    seed_account(&mut conn, "owner@example.com", "member");
    seed_playlist(
        &mut conn,
        "playlist:owner",
        "owner@example.com",
        "playlists/owner.m3u",
    );
    seed_playlist(
        &mut conn,
        "playlist:admin",
        "owner@example.com",
        "playlists/admin.m3u",
    );
    for name in ["owner", "admin"] {
        std::fs::write(music.path().join(format!("playlists/{name}.m3u")), name).unwrap();
    }
    store::insert_content_report(
        &mut conn,
        &models::ContentReport {
            id: "report".into(),
            reporter_account_id: "owner@example.com".into(),
            target_type: "playlist".into(),
            target_id: "playlist:owner".into(),
            reason: "spam".into(),
            details: None,
            status: "open".into(),
            created_at: "2026-09-12T00:00:00Z".into(),
            resolved_at: None,
        },
    )
    .unwrap();
    drop(conn);

    app.delete_playlist(
        &common::ctx_user("owner@example.com"),
        DeletePlaylistRequest {
            playlist_id: "playlist:owner".into(),
        },
    )
    .unwrap();
    app.delete_playlist(
        &common::ctx_admin("admin@example.com"),
        DeletePlaylistRequest {
            playlist_id: "playlist:admin".into(),
        },
    )
    .unwrap();
    assert!(store::list_playlists(&mut pool.get().unwrap())
        .unwrap()
        .is_empty());
    assert!(store::list_content_reports(&mut pool.get().unwrap(), 0, 10)
        .unwrap()
        .is_empty());
}

#[test]
fn deletion_rejects_other_member_anonymous_missing_and_unsafe_files() {
    for (id, relative, make_dir) in [
        ("playlist:normal", "playlists/normal.m3u", false),
        ("playlist:directory", "playlists/directory.m3u", true),
        ("playlist:escape", "../escape.m3u", false),
    ] {
        let (app, pool, music) = configured_app();
        let mut conn = pool.get().unwrap();
        seed_account(&mut conn, "admin@example.com", "admin");
        seed_account(&mut conn, "owner@example.com", "member");
        seed_account(&mut conn, "other@example.com", "member");
        seed_playlist(&mut conn, id, "owner@example.com", relative);
        if make_dir {
            std::fs::create_dir_all(music.path().join(relative)).unwrap();
        } else if relative.starts_with("playlists/") {
            std::fs::write(music.path().join(relative), "playlist").unwrap();
        }
        drop(conn);
        let request = || DeletePlaylistRequest {
            playlist_id: id.into(),
        };
        assert_eq!(
            app.delete_playlist(&common::ctx_user("other@example.com"), request())
                .unwrap_err()
                .code,
            403
        );
        assert_eq!(
            app.delete_playlist(&common::ctx_anon(), request())
                .unwrap_err()
                .code,
            401
        );
        let owner_result = app.delete_playlist(&common::ctx_user("owner@example.com"), request());
        if id == "playlist:normal" {
            assert!(owner_result.is_ok());
            assert_eq!(
                app.delete_playlist(&common::ctx_admin("admin@example.com"), request())
                    .unwrap_err()
                    .code,
                404
            );
        } else {
            assert!(owner_result.is_err());
            assert!(store::get_playlist(&mut pool.get().unwrap(), id)
                .unwrap()
                .is_some());
        }
    }
}

#[test]
fn database_rollback_restores_playlist_file() {
    let (app, pool, music) = configured_app();
    let path = music.path().join("playlists/rollback.m3u");
    std::fs::write(&path, "playlist").unwrap();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "owner@example.com", "member");
    seed_playlist(
        &mut conn,
        "playlist:rollback",
        "owner@example.com",
        "playlists/rollback.m3u",
    );
    conn.batch_execute(
        "CREATE TRIGGER reject_playlist_delete BEFORE DELETE ON playlists
         BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
    )
    .unwrap();
    drop(conn);
    assert!(app
        .delete_playlist(
            &common::ctx_user("owner@example.com"),
            DeletePlaylistRequest {
                playlist_id: "playlist:rollback".into(),
            },
        )
        .is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "playlist");
}
