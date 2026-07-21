//! Audiobook catalog separation, overlap reconciliation, and per-account progress.

mod common;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use common::DataMap;
use ichoi::db::{models, store};
use libichoi::csil::services::{AdminService, LibraryService, SessionService};
use libichoi::csil::types::*;

fn account(id: &str) -> models::Account {
    models::Account {
        id: id.to_string(),
        handle: id.split('@').next().unwrap_or(id).to_string(),
        display_name: None,
        role: "member".to_string(),
        created_at: "2026-07-17T00:00:00Z".to_string(),
    }
}

#[test]
fn browse_and_search_are_library_scoped() {
    let (app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    common::create_artist(&mut conn, &DataMap::new());
    common::create_album(&mut conn, &DataMap::new());
    common::create_track(&mut conn, &DataMap::new());

    let mut book_album = DataMap::new();
    book_album.insert("id".into(), "book-1".into());
    book_album.insert("title".into(), "The Long Book".into());
    common::create_album(&mut conn, &book_album);
    let mut chapter = DataMap::new();
    chapter.insert("id".into(), "chapter-1".into());
    chapter.insert("library_id".into(), "lib:audiobook".into());
    chapter.insert("album_id".into(), "book-1".into());
    chapter.insert("title".into(), "The Long Chapter".into());
    chapter.insert("root_relative_path".into(), "The Long Book/01.mp3".into());
    common::create_track(&mut conn, &chapter);
    drop(conn);

    let music = app
        .list_albums(
            &common::ctx_anon(),
            BrowseRequest {
                library: Some(Library::Music),
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    let books = app
        .list_albums(
            &common::ctx_anon(),
            BrowseRequest {
                library: Some(Library::Audiobook),
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        music
            .albums
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["album-1"]
    );
    assert_eq!(
        books
            .albums
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["book-1"]
    );

    let music_search = app
        .search(
            &common::ctx_anon(),
            SearchRequest {
                query: "Long".into(),
                library: Some(Library::Music),
                limit: None,
            },
        )
        .unwrap();
    assert!(music_search.albums.is_empty());
    assert!(music_search.tracks.is_empty());
}

#[test]
fn progress_is_isolated_by_user_and_restricted_to_audiobooks() {
    let (app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    store::upsert_account(&mut conn, &account("ann@example.com")).unwrap();
    store::upsert_account(&mut conn, &account("bob@example.com")).unwrap();
    common::create_artist(&mut conn, &DataMap::new());
    common::create_album(&mut conn, &DataMap::new());
    let mut chapter = DataMap::new();
    chapter.insert("id".into(), "chapter-1".into());
    chapter.insert("library_id".into(), "lib:audiobook".into());
    common::create_track(&mut conn, &chapter);
    let mut song = DataMap::new();
    song.insert("id".into(), "song-1".into());
    song.insert("root_relative_path".into(), "song-1.flac".into());
    common::create_track(&mut conn, &song);
    drop(conn);

    app.update_audiobook_progress(
        &common::ctx_user("ann@example.com"),
        UpdateAudiobookProgressRequest {
            track_id: "chapter-1".into(),
            position_ms: 42_000,
            completed: false,
        },
    )
    .unwrap();

    let ann = app
        .get_audiobook_progress(
            &common::ctx_user("ann@example.com"),
            AudiobookProgressRequest {
                track_ids: vec!["chapter-1".into()],
            },
        )
        .unwrap();
    let bob = app
        .get_audiobook_progress(
            &common::ctx_user("bob@example.com"),
            AudiobookProgressRequest {
                track_ids: vec!["chapter-1".into()],
            },
        )
        .unwrap();
    assert_eq!(ann.progress[0].position_ms, 42_000);
    assert!(bob.progress.is_empty());

    let err = app
        .update_audiobook_progress(
            &common::ctx_user("ann@example.com"),
            UpdateAudiobookProgressRequest {
                track_id: "song-1".into(),
                position_ms: 10,
                completed: false,
            },
        )
        .unwrap_err();
    assert_eq!(err.code, 400);
    assert_eq!(
        app.get_audiobook_progress(
            &common::ctx_anon(),
            AudiobookProgressRequest { track_ids: vec![] },
        )
        .unwrap_err()
        .code,
        401
    );
}

#[test]
fn guest_mode_uses_one_global_progress_profile() {
    let (app, pool) = common::test_app();
    let mut conn = pool.get().unwrap();
    common::create_artist(&mut conn, &DataMap::new());
    common::create_album(&mut conn, &DataMap::new());
    let mut chapter = DataMap::new();
    chapter.insert("id".into(), "guest-chapter".into());
    chapter.insert("library_id".into(), "lib:audiobook".into());
    common::create_track(&mut conn, &chapter);
    drop(conn);

    let guest = app
        .whoami(
            &common::ctx_anon(),
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(guest.role, Role::Guest);
    app.update_audiobook_progress(
        &common::ctx_anon(),
        UpdateAudiobookProgressRequest {
            track_id: "guest-chapter".into(),
            position_ms: 25_000,
            completed: false,
        },
    )
    .unwrap();
    let progress = app
        .get_audiobook_progress(
            &common::ctx_anon(),
            AudiobookProgressRequest {
                track_ids: vec!["guest-chapter".into()],
            },
        )
        .unwrap();
    assert_eq!(progress.progress[0].position_ms, 25_000);

    let mut conn = pool.get().unwrap();
    store::upsert_account(&mut conn, &account("ann@example.com")).unwrap();
    drop(conn);
    assert_eq!(
        app.get_audiobook_progress(
            &common::ctx_anon(),
            AudiobookProgressRequest {
                track_ids: vec!["guest-chapter".into()],
            },
        )
        .unwrap_err()
        .code,
        401
    );
    assert!(app
        .get_audiobook_progress(
            &common::ctx_user("ann@example.com"),
            AudiobookProgressRequest {
                track_ids: vec!["guest-chapter".into()],
            },
        )
        .unwrap()
        .progress
        .is_empty());
}

#[test]
fn nested_audiobook_root_is_removed_from_music_on_rescan() {
    let (app, pool) = common::test_app();
    let root = std::env::temp_dir().join(format!("ichoi-audiobook-{}", uuid::Uuid::now_v7()));
    let music = root.join("music");
    let books = music.join("audiobooks");
    std::fs::create_dir_all(books.join("A Book")).unwrap();
    std::fs::create_dir_all(music.join("An Artist")).unwrap();
    std::fs::write(books.join("A Book/01.mp3"), b"not-really-mp3").unwrap();
    std::fs::write(music.join("An Artist/song.mp3"), b"not-really-mp3").unwrap();

    let mut conn = pool.get().unwrap();
    common::ensure_library(&mut conn, "lib:music");
    common::ensure_library(&mut conn, "lib:audiobook");
    ichoi::scan::scan_library(&mut conn, "lib:music", &music, None, false, true, &[]).unwrap();
    assert_eq!(
        store::tracks_for_library(&mut conn, "lib:music")
            .unwrap()
            .len(),
        2
    );

    ichoi::scan::scan_library(
        &mut conn,
        "lib:music",
        &music,
        Some(&books),
        false,
        true,
        &[],
    )
    .unwrap();
    ichoi::scan::scan_library(&mut conn, "lib:audiobook", &books, None, false, true, &[]).unwrap();
    let music_paths: HashSet<_> = store::tracks_for_library(&mut conn, "lib:music")
        .unwrap()
        .into_iter()
        .map(|track| track.root_relative_path)
        .collect();
    let book_paths: HashSet<_> = store::tracks_for_library(&mut conn, "lib:audiobook")
        .unwrap()
        .into_iter()
        .map(|track| track.root_relative_path)
        .collect();
    assert_eq!(
        music_paths,
        HashSet::from(["An Artist/song.mp3".to_string()])
    );
    assert_eq!(book_paths, HashSet::from(["A Book/01.mp3".to_string()]));
    drop(conn);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn disc_subfolders_flatten_into_the_parent_album_and_can_be_disabled() {
    let (_app, pool) = common::test_app();
    let root = std::env::temp_dir().join(format!("ichoi-discs-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(root.join("A Book/CD1")).unwrap();
    std::fs::create_dir_all(root.join("A Book/CD2")).unwrap();
    std::fs::write(root.join("A Book/CD1/01.mp3"), b"not-really-mp3").unwrap();
    std::fs::write(root.join("A Book/CD2/01.mp3"), b"not-really-mp3").unwrap();
    let words = vec![
        "cd".to_string(),
        "disc".to_string(),
        "bonus disc".to_string(),
    ];

    let mut conn = pool.get().unwrap();
    common::ensure_library(&mut conn, "lib:music");
    ichoi::scan::scan_library(&mut conn, "lib:music", &root, None, false, true, &words).unwrap();
    let albums = store::list_albums(&mut conn, "lib:music", 0, 10).unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].title, "A Book");
    let tracks = store::tracks_for_album(&mut conn, &albums[0].id).unwrap();
    assert_eq!(tracks.len(), 2);
    assert!(tracks.iter().any(|track| track.title.starts_with("CD1 - ")));
    assert!(tracks.iter().any(|track| track.title.starts_with("CD2 - ")));
    assert_eq!(
        tracks
            .iter()
            .map(|track| track.disc_no)
            .collect::<HashSet<_>>(),
        HashSet::from([Some(1), Some(2)])
    );

    ichoi::scan::scan_library(&mut conn, "lib:music", &root, None, false, false, &words).unwrap();
    let albums = store::list_albums(&mut conn, "lib:music", 0, 10).unwrap();
    assert_eq!(albums.len(), 2);
    assert!(store::tracks_for_library(&mut conn, "lib:music")
        .unwrap()
        .iter()
        .all(|track| !track.title.starts_with("CD")));

    drop(conn);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn admin_resync_reconciles_missing_and_new_files_and_is_single_flight() {
    let (mut app, pool) = common::test_app();
    assert_eq!(
        app.resync_library(
            &common::ctx_user("member@example.com"),
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap_err()
        .code,
        403
    );

    let root = std::env::temp_dir().join(format!("ichoi-resync-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(root.join("New Album")).unwrap();
    std::fs::write(root.join("New Album/new.mp3"), b"not-really-mp3").unwrap();
    let mut config = common::test_config();
    config.music_dir = Some(root.clone());
    app.config = Arc::new(config);

    let mut conn = pool.get().unwrap();
    common::create_artist(&mut conn, &DataMap::new());
    common::create_album(&mut conn, &DataMap::new());
    let mut stale = DataMap::new();
    stale.insert("id".into(), "stale-track".into());
    stale.insert("root_relative_path".into(), "Missing Album/gone.mp3".into());
    common::create_track(&mut conn, &stale);
    drop(conn);

    app.scan_running.store(true, Ordering::Release);
    let duplicate = app
        .resync_library(
            &common::ctx_admin("admin@example.com"),
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(!duplicate.started);
    assert!(duplicate.running);
    app.scan_running.store(false, Ordering::Release);

    let started = app
        .resync_library(
            &common::ctx_admin("admin@example.com"),
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(started.started);
    assert!(started.running);

    for _ in 0..200 {
        if !app.library_scan_running() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!app.library_scan_running(), "resync did not finish");
    let status = app
        .get_resync_status(
            &common::ctx_admin("admin@example.com"),
            Page {
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(!status.running);

    let mut conn = pool.get().unwrap();
    assert!(store::get_track(&mut conn, "stale-track")
        .unwrap()
        .is_none());
    assert_eq!(
        store::tracks_for_library(&mut conn, "lib:music")
            .unwrap()
            .len(),
        1
    );
    drop(conn);
    std::fs::remove_dir_all(root).unwrap();
}
