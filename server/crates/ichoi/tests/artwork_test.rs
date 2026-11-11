mod common;

use ichoi::{
    artwork,
    db::{models, store},
    scan,
    transport::dispatch,
};
use libichoi::csil::{
    codec::*,
    services::{AdminService, LibraryService, SessionService},
    types::*,
};
use lofty::{
    config::WriteOptions,
    picture::{MimeType, Picture, PictureType},
    prelude::*,
    tag::{Tag, TagType},
};
use std::io::Cursor;
use std::path::Path;

fn png(color: [u8; 3]) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(40, 60, image::Rgb(color));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn cached_thumbnails_keep_size_and_image_content_separate() {
    let red = png([240, 10, 10]);
    let blue = png([10, 10, 240]);
    let small = artwork::normalize(&red, 32).unwrap();
    let large = artwork::normalize(&red, 64).unwrap();
    let changed = artwork::normalize(&blue, 32).unwrap();
    assert_eq!(image::load_from_memory(&small.data).unwrap().height(), 32);
    assert_eq!(image::load_from_memory(&large.data).unwrap().height(), 64);
    assert_ne!(small.data, changed.data);
    assert_eq!(small.data, artwork::normalize(&red, 32).unwrap().data);
}
fn account(conn: &mut diesel::SqliteConnection, id: &str, role: &str) {
    store::upsert_account(
        conn,
        &models::Account {
            id: id.into(),
            handle: id.into(),
            display_name: None,
            role: role.into(),
            created_at: "2026-11-10T00:00:00Z".into(),
        },
    )
    .unwrap();
}
fn seed(conn: &mut diesel::SqliteConnection) {
    account(conn, "admin", "admin");
    account(conn, "editor", "member");
    common::create_artist(conn, &common::DataMap::new());
    common::create_album(conn, &common::DataMap::new());
    common::create_track(conn, &common::DataMap::new());
}
fn set(kind: ArtworkKind, id: &str, data: Option<Vec<u8>>) -> SetArtworkRequest {
    SetArtworkRequest {
        kind,
        id: id.into(),
        data,
        candidate_id: None,
    }
}
fn get(kind: ArtworkKind, id: &str) -> ArtworkRequest {
    ArtworkRequest {
        kind,
        id: id.into(),
        max_size: Some(64),
    }
}
fn wav(path: &Path) {
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend(38u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(8000u32.to_le_bytes());
    bytes.extend(16000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(0i16.to_le_bytes());
    std::fs::write(path, bytes).unwrap();
}
fn library(conn: &mut diesel::SqliteConnection, root: &Path) {
    store::upsert_library(
        conn,
        &models::Library {
            id: "lib:audiobook".into(),
            kind: "audiobook".into(),
            path: root.to_string_lossy().into_owned(),
        },
    )
    .unwrap();
}
fn scan_book(conn: &mut diesel::SqliteConnection, root: &Path) {
    scan::scan_library(
        conn,
        "lib:audiobook",
        root,
        None,
        false,
        true,
        &["cd".into(), "disc".into()],
    )
    .unwrap();
}

#[test]
fn permission_is_explicit_and_revocation_applies_to_existing_connections() {
    let (app, pool) = common::test_app();
    seed(&mut pool.get().unwrap());
    let request = || set(ArtworkKind::Album, "album-1", Some(png([250, 10, 20])));
    assert_eq!(
        app.set_artwork(&common::ctx_anon(), request())
            .unwrap_err()
            .code,
        401
    );
    assert_eq!(
        app.set_artwork(&common::ctx_node("satellite"), request())
            .unwrap_err()
            .code,
        403
    );
    let editor = common::ctx_user("editor");
    assert_eq!(app.set_artwork(&editor, request()).unwrap_err().code, 403);
    let grant = SetLibraryEditorRequest {
        account_id: "editor".into(),
        enabled: true,
    };
    assert_eq!(
        app.set_library_editor(&editor, grant.clone())
            .unwrap_err()
            .code,
        403
    );
    let result = dispatch(
        &app,
        &common::ctx_admin("admin"),
        "admin",
        "set-library-editor",
        &encode_set_library_editor_request(&grant),
    )
    .unwrap();
    assert!(decode_ok(&result).unwrap().ok);
    assert_eq!(
        app.whoami(
            &editor,
            Page {
                offset: None,
                limit: None
            }
        )
        .unwrap()
        .can_edit_library,
        Some(true)
    );
    let result = dispatch(
        &app,
        &editor,
        "library",
        "set-artwork",
        &encode_set_artwork_request(&request()),
    )
    .unwrap();
    assert!(decode_ok(&result).unwrap().ok);
    app.set_library_editor(
        &common::ctx_admin("admin"),
        SetLibraryEditorRequest {
            enabled: false,
            ..grant
        },
    )
    .unwrap();
    assert_eq!(app.set_artwork(&editor, request()).unwrap_err().code, 403);
    assert_eq!(
        app.set_artwork(&editor, set(ArtworkKind::Album, "album-1", None))
            .unwrap_err()
            .code,
        403
    );
}

#[test]
fn manual_album_and_artist_covers_override_automatic_and_reset_independently() {
    let (app, pool) = common::test_app();
    seed(&mut pool.get().unwrap());
    let automatic = artwork::normalize(&png([10, 250, 20]), 64).unwrap();
    artwork::save(
        &mut pool.get().unwrap(),
        "album",
        "album-1",
        "automatic",
        &automatic,
        None,
    )
    .unwrap();
    let admin = common::ctx_admin("admin");
    app.set_artwork(
        &admin,
        set(ArtworkKind::Album, "album-1", Some(png([250, 10, 20]))),
    )
    .unwrap();
    let album = app
        .get_artwork(&common::ctx_anon(), get(ArtworkKind::Album, "album-1"))
        .unwrap();
    let inherited = app
        .get_artwork(&common::ctx_anon(), get(ArtworkKind::Artist, "artist-1"))
        .unwrap();
    assert_eq!(album.data, inherited.data);
    app.set_artwork(
        &admin,
        set(ArtworkKind::Artist, "artist-1", Some(png([10, 20, 250]))),
    )
    .unwrap();
    assert_ne!(
        album.data,
        app.get_artwork(&common::ctx_anon(), get(ArtworkKind::Artist, "artist-1"))
            .unwrap()
            .data
    );
    app.set_artwork(&admin, set(ArtworkKind::Artist, "artist-1", None))
        .unwrap();
    assert_eq!(
        album.data,
        app.get_artwork(&common::ctx_anon(), get(ArtworkKind::Artist, "artist-1"))
            .unwrap()
            .data
    );
    app.set_artwork(&admin, set(ArtworkKind::Album, "album-1", None))
        .unwrap();
    let restored = app
        .get_artwork(&common::ctx_anon(), get(ArtworkKind::Album, "album-1"))
        .unwrap();
    assert_ne!(album.data, restored.data);
    assert_eq!(
        restored.data,
        artwork::normalize(&automatic.data, 64).unwrap().data
    );
}

#[test]
fn invalid_uploads_leave_the_previous_cover_in_place() {
    let (app, pool) = common::test_app();
    seed(&mut pool.get().unwrap());
    let admin = common::ctx_admin("admin");
    app.set_artwork(
        &admin,
        set(ArtworkKind::Album, "album-1", Some(png([40, 50, 60]))),
    )
    .unwrap();
    let before = app
        .get_artwork(&admin, get(ArtworkKind::Album, "album-1"))
        .unwrap();
    for data in [
        b"<svg>script</svg>".to_vec(),
        vec![0; artwork::MAX_UPLOAD + 1],
        Vec::new(),
    ] {
        assert_eq!(
            app.set_artwork(&admin, set(ArtworkKind::Album, "album-1", Some(data)))
                .unwrap_err()
                .code,
            400
        );
    }
    let large = image::RgbImage::new(8193, 1);
    let mut bytes = Cursor::new(Vec::new());
    large.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    assert_eq!(
        app.set_artwork(
            &admin,
            set(ArtworkKind::Album, "album-1", Some(bytes.into_inner()))
        )
        .unwrap_err()
        .code,
        400
    );
    assert_eq!(
        before.data,
        app.get_artwork(&admin, get(ArtworkKind::Album, "album-1"))
            .unwrap()
            .data
    );
    assert_eq!(
        app.set_artwork(&admin, set(ArtworkKind::Album, "missing", None))
            .unwrap_err()
            .code,
        404
    );
}

#[test]
fn discovery_handles_case_numbered_fronts_artwork_folders_and_disc_parents() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let book = root.join("The Book");
    let disc = book.join("CD1");
    std::fs::create_dir_all(book.join("Artwork")).unwrap();
    std::fs::create_dir_all(&disc).unwrap();
    for name in [
        "Folder.JPG",
        "00 - Cover.PNG",
        "back.jpg",
        "AlbumArtSmall.jpg",
    ] {
        std::fs::write(book.join(name), png([10, 20, 30])).unwrap();
    }
    let candidates = scan::cover_candidates(&disc.join("01.wav"), root, &["cd".into()]);
    assert_eq!(candidates[0], book.join("00 - Cover.PNG"));
    assert!(candidates.contains(&book.join("Folder.JPG")));
    assert!(!candidates.contains(&book.join("back.jpg")));
    std::fs::write(book.join("Artwork/Book Front.webp"), png([30, 40, 50])).unwrap();
    assert!(
        scan::cover_candidates(&disc.join("01.wav"), root, &["cd".into()])
            .contains(&book.join("Artwork/Book Front.webp"))
    );
    // An album cannot inherit an unrelated artist or library-root cover.
    std::fs::write(root.join("cover.jpg"), png([30, 40, 50])).unwrap();
    let other = root.join("Other Album");
    std::fs::create_dir(&other).unwrap();
    assert!(scan::cover_candidates(&other.join("01.wav"), root, &["cd".into()]).is_empty());
    assert!(
        !scan::cover_candidates(&disc.join("01.wav"), &disc, &["cd".into()])
            .contains(&book.join("Folder.JPG"))
    );
}

#[test]
fn scan_skips_broken_candidates_and_preserves_manual_cover_after_rescan() {
    let (app, pool) = common::test_app();
    let temp = tempfile::tempdir().unwrap();
    let book = temp.path().join("The Book");
    std::fs::create_dir(&book).unwrap();
    wav(&book.join("01.wav"));
    wav(&book.join("02.wav"));
    std::fs::write(book.join("Front.jpg"), b"broken").unwrap();
    std::fs::write(book.join("Folder.JPG"), png([10, 220, 10])).unwrap();
    let id = {
        let mut conn = pool.get().unwrap();
        account(&mut conn, "admin", "admin");
        library(&mut conn, temp.path());
        scan_book(&mut conn, temp.path());
        let albums = store::list_albums(&mut conn, "lib:audiobook", 0, 10).unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].has_cover_art, 1);
        assert!(albums[0]
            .cover_art_path
            .as_ref()
            .unwrap()
            .ends_with("Folder.JPG"));
        albums[0].id.clone()
    };
    app.set_artwork(
        &common::ctx_admin("admin"),
        set(ArtworkKind::Album, &id, Some(png([220, 10, 10]))),
    )
    .unwrap();
    let before = app
        .get_artwork(&common::ctx_anon(), get(ArtworkKind::Album, &id))
        .unwrap();
    std::fs::remove_file(book.join("Folder.JPG")).unwrap();
    scan_book(&mut pool.get().unwrap(), temp.path());
    let albums = app
        .list_albums(
            &common::ctx_anon(),
            BrowseRequest {
                library: Some(Library::Audiobook),
                offset: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(albums.albums[0].has_cover_art);
    assert_eq!(
        before.data,
        app.get_cover_art(
            &common::ctx_anon(),
            CoverArtRequest {
                album_id: id.clone(),
                max_size: Some(64)
            }
        )
        .unwrap()
        .data
    );
    app.set_artwork(
        &common::ctx_admin("admin"),
        set(ArtworkKind::Album, &id, None),
    )
    .unwrap();
    assert_eq!(
        app.get_artwork(&common::ctx_anon(), get(ArtworkKind::Album, &id))
            .unwrap_err()
            .code,
        404
    );
}

#[test]
fn embedded_front_is_preferred_and_later_tracks_do_not_clear_its_flag() {
    let (_app, pool) = common::test_app();
    let temp = tempfile::tempdir().unwrap();
    let book = temp.path().join("The Book");
    std::fs::create_dir(&book).unwrap();
    let first = book.join("01.wav");
    wav(&first);
    wav(&book.join("02.wav"));
    let front = png([220, 10, 10]);
    let mut tag = Tag::new(TagType::Id3v2);
    tag.push_picture(Picture::new_unchecked(
        PictureType::CoverBack,
        Some(MimeType::Png),
        None,
        png([10, 220, 10]),
    ));
    tag.push_picture(Picture::new_unchecked(
        PictureType::CoverFront,
        Some(MimeType::Png),
        None,
        front.clone(),
    ));
    tag.save_to_path(&first, WriteOptions::default()).unwrap();
    assert_eq!(scan::extract_embedded_cover(&first).unwrap().1, front);
    let mut conn = pool.get().unwrap();
    library(&mut conn, temp.path());
    scan_book(&mut conn, temp.path());
    assert_eq!(
        store::list_albums(&mut conn, "lib:audiobook", 0, 10).unwrap()[0].has_cover_art,
        1
    );
    // Removing the only embedded cover clears the automatic flag on the next scan.
    wav(&first);
    scan_book(&mut conn, temp.path());
    assert_eq!(
        store::list_albums(&mut conn, "lib:audiobook", 0, 10).unwrap()[0].has_cover_art,
        0
    );
}

#[test]
fn music_regrouping_keeps_the_manual_cover_when_temporary_album_rows_are_rebuilt() {
    let (app, pool) = common::test_app();
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("Folder Collection");
    std::fs::create_dir(&folder).unwrap();
    for number in [1, 2] {
        let path = folder.join(format!("{number}.wav"));
        wav(&path);
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_album(format!("Small tagged album {number}"));
        tag.set_artist("Test Artist".into());
        tag.save_to_path(&path, WriteOptions::default()).unwrap();
    }
    let rescan = |conn: &mut diesel::SqliteConnection| {
        scan::scan_library(
            conn,
            "lib:music",
            temp.path(),
            None,
            false,
            true,
            &["cd".into()],
        )
        .unwrap();
    };
    let id = {
        let mut conn = pool.get().unwrap();
        account(&mut conn, "admin", "admin");
        store::upsert_library(
            &mut conn,
            &models::Library {
                id: "lib:music".into(),
                kind: "music".into(),
                path: temp.path().to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        rescan(&mut conn);
        let albums = store::list_albums(&mut conn, "lib:music", 0, 10).unwrap();
        assert_eq!(albums.len(), 1);
        albums[0].id.clone()
    };
    app.set_artwork(
        &common::ctx_admin("admin"),
        set(ArtworkKind::Album, &id, Some(png([10, 20, 240]))),
    )
    .unwrap();
    let before = app
        .get_artwork(&common::ctx_anon(), get(ArtworkKind::Album, &id))
        .unwrap();
    rescan(&mut pool.get().unwrap());
    assert_eq!(
        before.data,
        app.get_artwork(&common::ctx_anon(), get(ArtworkKind::Album, &id))
            .unwrap()
            .data
    );
}

#[test]
fn library_requests_do_not_wait_for_the_artwork_worker() {
    let (app, _) = common::test_app();
    let worker = app.artwork_worker.clone();
    let (started, ready) = std::sync::mpsc::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let busy = std::thread::spawn(move || {
        worker.run(move || {
            started.send(()).unwrap();
            let _ = gate.recv_timeout(std::time::Duration::from_secs(5));
            Ok(())
        })
    });
    ready
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    let (reply, response) = std::sync::mpsc::channel();
    let request = std::thread::spawn(move || {
        let _ = reply.send(dispatch(
            &app,
            &common::ctx_anon(),
            "LibraryService",
            "list-libraries",
            &encode_page(&Page {
                offset: None,
                limit: None,
            }),
        ));
    });
    let result = response.recv_timeout(std::time::Duration::from_secs(2));
    release.send(()).unwrap();
    busy.join().unwrap().unwrap();
    request.join().unwrap();
    assert!(result.unwrap().is_ok());
}
