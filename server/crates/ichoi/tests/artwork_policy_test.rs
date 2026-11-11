mod common;
use ichoi::{
    artwork_discovery,
    db::{models, store},
    scan,
};
use libichoi::csil::{services::LibraryService, types::*};
use lofty::{
    config::WriteOptions,
    picture::{MimeType, Picture, PictureType},
    prelude::*,
    tag::{Tag, TagType},
};
use std::path::Path;

fn image(path: &Path, width: u32, height: u32, salt: u32) {
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([
            ((x * 17 + y * 5 + salt) % 255) as u8,
            ((x * 3 + y * 11 + salt) % 255) as u8,
            ((x + y + salt) % 255) as u8,
        ])
    });
    image.save(path).unwrap();
}
fn wav(path: &Path, title: &str, picture: Option<Vec<u8>>) {
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
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_album(title.into());
    tag.set_artist("Test Reader".into());
    if let Some(bytes) = picture {
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            Some(MimeType::Png),
            None,
            bytes,
        ));
    }
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}
fn setup(root: &Path) -> (ichoi::handlers::App, ichoi::db::SqlitePool) {
    let (app, pool) = common::test_app();
    store::upsert_library(
        &mut pool.get().unwrap(),
        &models::Library {
            id: "lib:audiobook".into(),
            kind: "audiobook".into(),
            path: root.to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    (app, pool)
}
fn rescan(pool: &ichoi::db::SqlitePool, root: &Path) {
    scan::scan_library(
        &mut pool.get().unwrap(),
        "lib:audiobook",
        root,
        None,
        false,
        true,
        &["cd".into(), "disc".into()],
    )
    .unwrap();
}
fn album(pool: &ichoi::db::SqlitePool) -> String {
    store::list_albums(&mut pool.get().unwrap(), "lib:audiobook", 0, 100).unwrap()[0]
        .id
        .clone()
}
fn selected(pool: &ichoi::db::SqlitePool, id: &str) -> Option<artwork_discovery::Candidate> {
    artwork_discovery::list(&mut pool.get().unwrap(), &ArtworkKind::Album, id)
        .unwrap()
        .into_iter()
        .find(|(_, selected)| *selected)
        .map(|(c, _)| c)
}

#[test]
fn better_artwork_subfolder_cover_beats_smaller_conventional_cover_and_sleeve_spread() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Running On Ice");
    std::fs::create_dir_all(folder.join("Artwork")).unwrap();
    wav(&folder.join("01.wav"), "Running On Ice", None);
    image(&folder.join("Cover.png"), 500, 498, 1);
    image(&folder.join("Artwork/Front.png"), 800, 797, 1);
    image(&folder.join("Artwork/Front-back.png"), 1600, 797, 2);
    let (_, pool) = setup(root.path());
    rescan(&pool, root.path());
    let id = album(&pool);
    assert!(selected(&pool, &id)
        .unwrap()
        .label
        .ends_with("Artwork/Front.png"));
    let all = artwork_discovery::list(&mut pool.get().unwrap(), &ArtworkKind::Album, &id).unwrap();
    assert!(all
        .iter()
        .any(|(c, s)| c.label.ends_with("Front-back.png") && !s && !c.eligible()));
}
#[test]
fn embedded_front_competes_on_quality_with_local_images_and_selected_candidate_can_be_pinned() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Book");
    std::fs::create_dir(&folder).unwrap();
    let embedded = root.path().join("embedded.png");
    image(&embedded, 600, 600, 3);
    wav(
        &folder.join("01.wav"),
        "Book",
        Some(std::fs::read(embedded).unwrap()),
    );
    image(&folder.join("Folder.png"), 200, 200, 1);
    image(&folder.join("Back.png"), 700, 700, 5);
    let (app, pool) = setup(root.path());
    rescan(&pool, root.path());
    let id = album(&pool);
    assert_eq!(selected(&pool, &id).unwrap().role, "Embedded front");
    store::upsert_account(
        &mut pool.get().unwrap(),
        &models::Account {
            id: "admin".into(),
            handle: "admin".into(),
            display_name: None,
            role: "admin".into(),
            created_at: "2026-11-10T00:00:00Z".into(),
        },
    )
    .unwrap();
    let page = app
        .list_artwork_candidates(
            &common::ctx_admin("admin"),
            ArtworkCandidatesRequest {
                kind: ArtworkKind::Album,
                id: id.clone(),
                offset: Some(0),
                limit: Some(100),
            },
        )
        .unwrap();
    let back = page
        .candidates
        .iter()
        .find(|c| c.label.ends_with("Back.png"))
        .unwrap();
    assert!(!back.suggested);
    app.set_artwork(
        &common::ctx_admin("admin"),
        SetArtworkRequest {
            kind: ArtworkKind::Album,
            id: id.clone(),
            data: None,
            candidate_id: Some(back.id.clone()),
        },
    )
    .unwrap();
    let before = app
        .get_artwork(
            &common::ctx_anon(),
            ArtworkRequest {
                kind: ArtworkKind::Album,
                id: id.clone(),
                max_size: Some(100),
            },
        )
        .unwrap();
    rescan(&pool, root.path());
    assert_eq!(
        before.data,
        app.get_artwork(
            &common::ctx_anon(),
            ArtworkRequest {
                kind: ArtworkKind::Album,
                id: id.clone(),
                max_size: Some(100)
            }
        )
        .unwrap()
        .data
    );
    let request = || ArtworkCandidateRequest {
        kind: ArtworkKind::Album,
        id: id.clone(),
        candidate_id: "../../etc/passwd".into(),
        max_size: None,
    };
    assert_eq!(
        app.get_artwork_candidate(&common::ctx_anon(), request())
            .unwrap_err()
            .code,
        404
    );
    image(&folder.join("Back.png"), 700, 700, 8);
    assert_eq!(
        app.get_artwork_candidate(
            &common::ctx_anon(),
            ArtworkCandidateRequest {
                candidate_id: back.id.clone(),
                ..request()
            }
        )
        .unwrap_err()
        .code,
        404
    );
}
#[test]
fn book_numbers_are_valid_but_unassigned_numbered_scans_are_not_automatic_covers() {
    let root = tempfile::tempdir().unwrap();
    let book = root.path().join("Book The 10th - The Slippery Slope");
    std::fs::create_dir(&book).unwrap();
    wav(
        &book.join("01.wav"),
        "Book The 10th - The Slippery Slope",
        None,
    );
    image(&book.join("10.png"), 418, 475, 1);
    let (_, pool) = setup(root.path());
    rescan(&pool, root.path());
    assert!(selected(&pool, &album(&pool))
        .unwrap()
        .label
        .ends_with("10.png"));
    std::fs::remove_file(book.join("10.png")).unwrap();
    image(&book.join("01.png"), 418, 475, 1);
    image(&book.join("02.png"), 418, 475, 2);
    rescan(&pool, root.path());
    assert!(selected(&pool, &album(&pool)).is_none());
}
#[test]
fn mixed_folders_do_not_assign_windows_guids_or_folder_images_to_every_album() {
    let root = tempfile::tempdir().unwrap();
    wav(&root.path().join("a.wav"), "Album A", None);
    wav(&root.path().join("b.wav"), "Album B", None);
    image(&root.path().join("AlbumArt_{one}_Large.png"), 300, 300, 1);
    image(&root.path().join("AlbumArt_{two}_Large.png"), 500, 500, 2);
    image(&root.path().join("Folder.png"), 600, 600, 3);
    image(&root.path().join("Album A Front.png"), 400, 400, 4);
    let (_, pool) = setup(root.path());
    rescan(&pool, root.path());
    let albums = store::list_albums(&mut pool.get().unwrap(), "lib:audiobook", 0, 100).unwrap();
    assert_eq!(albums.len(), 2);
    for album in albums {
        if album.title == "Album A" {
            assert!(selected(&pool, &album.id)
                .unwrap()
                .label
                .ends_with("Album A Front.png"));
        } else {
            assert!(selected(&pool, &album.id).is_none());
        }
    }
}
#[test]
fn pixel_replication_does_not_beat_real_detail_and_equal_choices_remain_stable() {
    let root = tempfile::tempdir().unwrap();
    wav(&root.path().join("01.wav"), "Book", None);
    let original = root.path().join("source.png");
    image(&original, 200, 200, 3);
    let upscale = image::open(&original).unwrap().resize_exact(
        800,
        800,
        image::imageops::FilterType::Nearest,
    );
    upscale.save(root.path().join("Front.png")).unwrap();
    std::fs::remove_file(original).unwrap();
    image(&root.path().join("Cover.png"), 400, 400, 5);
    let (_, pool) = setup(root.path());
    rescan(&pool, root.path());
    let id = album(&pool);
    let prior = selected(&pool, &id).unwrap();
    assert!(prior.label.ends_with("Cover.png"));
    image(&root.path().join("Other Cover.png"), 400, 400, 7);
    rescan(&pool, root.path());
    assert_eq!(selected(&pool, &id).unwrap().id, prior.id);
}

#[test]
fn imported_track_refreshes_cover_candidates_without_full_scan() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("01.wav");
    wav(&path, "Imported Book", None);
    image(&root.path().join("Folder.png"), 200, 200, 1);
    let (_app, pool) = setup(root.path());
    rescan(&pool, root.path());
    let id = album(&pool);
    assert_eq!(selected(&pool, &id).unwrap().width, 200);
    image(&root.path().join("Cover.png"), 800, 800, 2);
    scan::index_imported_file(
        &mut pool.get().unwrap(),
        "lib:audiobook",
        root.path(),
        &path,
        true,
        &["cd".into()],
    )
    .unwrap();
    assert_eq!(selected(&pool, &id).unwrap().width, 800);
}
