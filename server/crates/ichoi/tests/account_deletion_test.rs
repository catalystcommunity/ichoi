mod common;

use diesel::connection::SimpleConnection;
use ichoi::db::{models, store};

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

fn playlist(id: &str, owner: &str, path: &str, visibility: &str) -> models::Playlist {
    models::Playlist {
        id: id.into(),
        name: id.into(),
        owner: Some(owner.into()),
        root_relative_path: path.into(),
        visibility: visibility.into(),
    }
}

#[test]
fn deletion_removes_private_data_and_retains_public_playlist() {
    let (_app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(music.path().join("playlists")).unwrap();
    std::fs::write(music.path().join("playlists/private.m3u"), "private").unwrap();
    std::fs::write(music.path().join("playlists/public.m3u"), "public").unwrap();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    seed_account(&mut conn, "member@example.com", "member");
    seed_account(&mut conn, "other@example.com", "member");
    store::create_session(
        &mut conn,
        &ichoi::auth::sha256_hex("member-token"),
        "member@example.com",
        "2099-01-01T00:00:00Z",
    )
    .unwrap();
    let mut track_data = common::DataMap::new();
    track_data.insert("id".into(), "book-track".into());
    track_data.insert("library_id".into(), "lib:audiobook".into());
    common::create_artist(&mut conn, &common::DataMap::new());
    common::create_album(&mut conn, &common::DataMap::new());
    common::create_track(&mut conn, &track_data);
    store::upsert_audiobook_progress(
        &mut conn,
        &models::AudiobookProgress {
            account_id: "member@example.com".into(),
            track_id: "book-track".into(),
            position_ms: 42,
            completed: 0,
            updated_at: "2026-09-12T00:00:00Z".into(),
        },
    )
    .unwrap();
    store::create_player(
        &mut conn,
        &models::Player {
            id: "member-device".into(),
            kind: "private".into(),
            output_device_id: None,
            owner_account_id: Some("member@example.com".into()),
            name: "Member device".into(),
            name_suffix: None,
        },
    )
    .unwrap();
    store::upsert_playlist(
        &mut conn,
        &playlist(
            "playlist:private",
            "member@example.com",
            "playlists/private.m3u",
            "private",
        ),
    )
    .unwrap();
    store::upsert_playlist(
        &mut conn,
        &playlist(
            "playlist:public",
            "member@example.com",
            "playlists/public.m3u",
            "public",
        ),
    )
    .unwrap();
    for (id, reporter, target_type, target_id) in [
        (
            "submitted",
            "member@example.com",
            "account",
            "other@example.com",
        ),
        (
            "target-account",
            "other@example.com",
            "account",
            "member@example.com",
        ),
        (
            "target-playlist",
            "other@example.com",
            "playlist",
            "playlist:private",
        ),
    ] {
        store::insert_content_report(
            &mut conn,
            &models::ContentReport {
                id: id.into(),
                reporter_account_id: reporter.into(),
                target_type: target_type.into(),
                target_id: target_id.into(),
                reason: "other".into(),
                details: None,
                status: "open".into(),
                created_at: "2026-09-12T00:00:00Z".into(),
                resolved_at: None,
            },
        )
        .unwrap();
    }

    ichoi::deletion::delete_account(&mut conn, music.path(), "member@example.com").unwrap();

    assert!(store::get_account(&mut conn, "member@example.com")
        .unwrap()
        .is_none());
    assert!(
        store::account_for_token(&mut conn, &ichoi::auth::sha256_hex("member-token"))
            .unwrap()
            .is_none()
    );
    assert!(store::audiobook_progress_for_tracks(
        &mut conn,
        "member@example.com",
        &["book-track".into()]
    )
    .unwrap()
    .is_empty());
    assert!(store::get_player(&mut conn, "member-device")
        .unwrap()
        .is_none());
    assert!(store::get_playlist(&mut conn, "playlist:private")
        .unwrap()
        .is_none());
    let public = store::get_playlist(&mut conn, "playlist:public")
        .unwrap()
        .unwrap();
    assert!(public.owner.is_none());
    assert!(!music.path().join("playlists/private.m3u").exists());
    assert!(music.path().join("playlists/public.m3u").exists());
    assert!(store::list_content_reports(&mut conn, 0, 10)
        .unwrap()
        .is_empty());
}

#[test]
fn missing_file_is_allowed_but_file_errors_and_path_escapes_stop_deletion() {
    for (id, relative, make_directory) in [
        ("missing@example.com", "playlists/missing.m3u", false),
        ("directory@example.com", "playlists/not-a-file.m3u", true),
        ("escape@example.com", "../outside.m3u", false),
    ] {
        let (_app, pool) = common::test_app();
        let music = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(music.path().join("playlists")).unwrap();
        if make_directory {
            std::fs::create_dir_all(music.path().join(relative)).unwrap();
        }
        let mut conn = pool.get().unwrap();
        seed_account(&mut conn, "admin@example.com", "admin");
        seed_account(&mut conn, id, "member");
        store::upsert_playlist(&mut conn, &playlist("private", id, relative, "private")).unwrap();
        let result = ichoi::deletion::delete_account(&mut conn, music.path(), id);
        if id == "missing@example.com" {
            assert!(result.is_ok());
            assert!(store::get_account(&mut conn, id).unwrap().is_none());
        } else {
            assert!(result.is_err());
            assert!(store::get_account(&mut conn, id).unwrap().is_some());
        }
    }
}

#[test]
fn database_rollback_restores_the_staged_file() {
    let (_app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(music.path().join("playlists")).unwrap();
    let path = music.path().join("playlists/private.m3u");
    std::fs::write(&path, "private").unwrap();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    seed_account(&mut conn, "member@example.com", "member");
    store::upsert_playlist(
        &mut conn,
        &playlist(
            "playlist:private",
            "member@example.com",
            "playlists/private.m3u",
            "private",
        ),
    )
    .unwrap();
    conn.batch_execute(
        "CREATE TRIGGER reject_member_delete BEFORE DELETE ON accounts
         WHEN OLD.id = 'member@example.com' BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
    )
    .unwrap();

    assert!(
        ichoi::deletion::delete_account(&mut conn, music.path(), "member@example.com").is_err()
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "private");
    assert!(store::get_playlist(&mut conn, "playlist:private")
        .unwrap()
        .is_some());
}

#[test]
fn startup_reconciles_staging_for_present_and_deleted_rows() {
    let (_app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(music.path().join("playlists")).unwrap();
    let original = music.path().join("playlists/private.m3u");
    std::fs::write(&original, "private").unwrap();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "member@example.com", "member");
    let row = playlist(
        "playlist:private",
        "member@example.com",
        "playlists/private.m3u",
        "private",
    );
    store::upsert_playlist(&mut conn, &row).unwrap();

    ichoi::deletion::stage_playlist_file(music.path(), &row).unwrap();
    assert!(!original.exists());
    ichoi::deletion::reconcile_playlist_staging(&mut conn, music.path()).unwrap();
    assert!(original.exists());

    ichoi::deletion::stage_playlist_file(music.path(), &row).unwrap();
    store::delete_playlist_row(&mut conn, &row.id).unwrap();
    ichoi::deletion::reconcile_playlist_staging(&mut conn, music.path()).unwrap();
    assert!(!original.exists());
    assert!(
        std::fs::read_dir(music.path().join("playlists/.ichoi-delete-staging"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn last_administrator_is_protected() {
    let (_app, pool) = common::test_app();
    let music = tempfile::tempdir().unwrap();
    let mut conn = pool.get().unwrap();
    seed_account(&mut conn, "admin@example.com", "admin");
    let error =
        ichoi::deletion::delete_account(&mut conn, music.path(), "admin@example.com").unwrap_err();
    assert!(matches!(error, ichoi::deletion::DeletionError::LastAdmin));
}
