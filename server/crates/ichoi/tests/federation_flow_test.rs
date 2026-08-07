//! Instance federation export and import behavior.

mod common;

use std::sync::Arc;

use ichoi::db::{models, store};
use libichoi::csil::services::{AdminService, LibraryService};
use libichoi::csil::types::*;
use sha2::{Digest, Sha256};

fn app_with_music_root() -> (
    ichoi::handlers::App,
    ichoi::db::SqlitePool,
    tempfile::TempDir,
) {
    let (mut app, pool) = common::test_app();
    let root = tempfile::tempdir().unwrap();
    let mut config = common::test_config();
    config.music_dir = Some(root.path().to_path_buf());
    app.config = Arc::new(config);
    (app, pool, root)
}

fn hash(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

#[test]
fn a_member_can_create_a_manifest_for_an_original_track_file() {
    let (app, pool, root) = app_with_music_root();
    let relative = "Test Artist/Test Album/01.flac";
    std::fs::create_dir_all(root.path().join("Test Artist/Test Album")).unwrap();
    std::fs::write(root.path().join(relative), b"original audio bytes").unwrap();
    let mut conn = pool.get().unwrap();
    store::upsert_library(
        &mut conn,
        &models::Library {
            id: "lib:music".into(),
            kind: "music".into(),
            path: root.path().to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    common::create_artist(&mut conn, &common::DataMap::new());
    common::create_album(&mut conn, &common::DataMap::new());
    common::create_track(&mut conn, &common::DataMap::new());
    store::upsert_library(
        &mut conn,
        &models::Library {
            id: "lib:music".into(),
            kind: "music".into(),
            path: root.path().to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    drop(conn);

    let exported = app
        .export_manifest(
            &common::ctx_user("member@example.com"),
            ExportManifestRequest {
                track_id: "track-1".into(),
            },
        )
        .unwrap();

    assert_eq!(exported.files.len(), 1);
    assert_eq!(exported.files[0].sha256, hash(b"original audio bytes"));
    assert_eq!(exported.files[0].content_type, "audio/flac");
}

#[test]
fn a_guest_account_cannot_export_files() {
    let (app, _pool, _root) = app_with_music_root();
    let ctx = ichoi::handlers::Ctx {
        identity: ichoi::handlers::Identity::User {
            account_id: "guest@example.com".into(),
            role: "guest".into(),
        },
        allow_guest: false,
    };
    let error = app
        .export_manifest(
            &ctx,
            ExportManifestRequest {
                track_id: "missing".into(),
            },
        )
        .unwrap_err();
    assert_eq!(error.code, 403);
}

#[test]
fn an_admin_imports_indexes_and_deduplicates_a_track() {
    let (app, _pool, root) = app_with_music_root();
    let data = b"not a real wav, so the scanner uses its metadata fallback".to_vec();
    let content_hash = hash(&data);
    let request = |path: &str| ImportTrackRequest {
        library: Some(Library::Music),
        root_relative_path: path.into(),
        content_type: "audio/wav".into(),
        content_hash: Some(content_hash.clone()),
        data: data.clone(),
    };

    let first = app
        .import_track(
            &common::ctx_admin("admin@example.com"),
            request("imports/friend/book.wav"),
        )
        .unwrap();
    let second = app
        .import_track(
            &common::ctx_admin("admin@example.com"),
            request("another/book.wav"),
        )
        .unwrap();

    assert!(first.imported);
    assert_eq!(first.track.as_ref().unwrap().library, Library::Music);
    assert!(root.path().join("imports/friend/book.wav").is_file());
    assert!(!second.imported);
    assert!(second.skipped_existing);
    assert_eq!(second.track_id, first.track_id);
    assert!(!root.path().join("another/book.wav").exists());
}

#[test]
fn import_rejects_unsafe_paths_and_bad_hashes() {
    let (app, _pool, _root) = app_with_music_root();
    let request = |path: &str, content_hash: &str| ImportTrackRequest {
        library: Some(Library::Music),
        root_relative_path: path.into(),
        content_type: "audio/wav".into(),
        content_hash: Some(content_hash.into()),
        data: b"audio".to_vec(),
    };

    let unsafe_path = app
        .import_track(
            &common::ctx_admin("admin@example.com"),
            request("../escape.wav", &hash(b"audio")),
        )
        .unwrap_err();
    let bad_hash = app
        .import_track(
            &common::ctx_admin("admin@example.com"),
            request("safe.wav", "00"),
        )
        .unwrap_err();

    assert_eq!(unsafe_path.code, 400);
    assert_eq!(bad_hash.code, 400);
}

#[test]
fn only_an_admin_can_import_into_an_instance() {
    let (app, _pool, _root) = app_with_music_root();
    let data = b"audio".to_vec();
    let error = app
        .import_track(
            &common::ctx_user("member@example.com"),
            ImportTrackRequest {
                library: Some(Library::Music),
                root_relative_path: "song.wav".into(),
                content_type: "audio/wav".into(),
                content_hash: Some(hash(&data)),
                data,
            },
        )
        .unwrap_err();
    assert_eq!(error.code, 403);
}

#[test]
fn chunked_copy_verifies_chunks_and_includes_art_and_lyrics() {
    let (source, source_pool, source_root) = app_with_music_root();
    let folder = source_root.path().join("Test Artist/Test Album");
    std::fs::create_dir_all(&folder).unwrap();
    let mut audio = vec![7_u8; ichoi::federation::CHUNK_SIZE];
    audio.extend_from_slice(b"second chunk");
    std::fs::write(folder.join("01.flac"), &audio).unwrap();
    std::fs::write(folder.join("cover.jpg"), b"art").unwrap();
    std::fs::write(folder.join("01.lrc"), b"lyrics").unwrap();
    std::fs::write(folder.join("notes.txt"), b"do not copy").unwrap();
    std::fs::write(folder.join("album.m3u"), b"01.flac").unwrap();
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("private.jpg"), b"private").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("private.jpg"),
            folder.join("linked.jpg"),
        )
        .unwrap();
    }
    let mut conn = source_pool.get().unwrap();
    common::create_artist(&mut conn, &common::DataMap::new());
    common::create_album(&mut conn, &common::DataMap::new());
    common::create_track(&mut conn, &common::DataMap::new());
    store::upsert_library(
        &mut conn,
        &models::Library {
            id: "lib:music".into(),
            kind: "music".into(),
            path: source_root.path().to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    drop(conn);

    let manifest = source
        .export_manifest(
            &common::ctx_user("member@example.com"),
            ExportManifestRequest {
                track_id: "track-1".into(),
            },
        )
        .unwrap();
    assert_eq!(manifest.files.len(), 3);
    assert_eq!(manifest.files[0].chunks.len(), 2);
    assert!(manifest.files.iter().any(|file| file.root_relative_path.ends_with("cover.jpg")));
    assert!(manifest.files.iter().any(|file| file.root_relative_path.ends_with("01.lrc")));
    assert!(!manifest.files.iter().any(|file| file.root_relative_path.ends_with(".m3u")));
    assert!(!manifest.files.iter().any(|file| file.root_relative_path.ends_with("notes.txt")));
    assert!(!manifest.files.iter().any(|file| file.root_relative_path.ends_with("linked.jpg")));

    let (destination, _destination_pool, destination_root) = app_with_music_root();
    let destination_files: Vec<TransferFile> = manifest
        .files
        .iter()
        .cloned()
        .map(|mut file| {
            file.root_relative_path = format!("imports/friend/{}", file.root_relative_path);
            file
        })
        .collect();
    let begun = destination
        .begin_import(
            &common::ctx_admin("admin@example.com"),
            BeginImportRequest {
                library: Some(Library::Music),
                track_file_index: 0,
                files: destination_files.clone(),
            },
        )
        .unwrap();
    assert_eq!(begun.missing_chunks.len(), 4);

    let first = &begun.missing_chunks[0];
    let source_file = &manifest.files[first.file_index as usize];
    let chunk = source
        .export_chunk(
            &common::ctx_user("member@example.com"),
            ExportChunkRequest {
                track_id: "track-1".into(),
                root_relative_path: source_file.root_relative_path.clone(),
                chunk_index: first.chunk_index,
            },
        )
        .unwrap();
    let mut corrupt = chunk.data.clone();
    corrupt[0] ^= 1;
    let rejected = destination
        .import_chunk(
            &common::ctx_admin("admin@example.com"),
            ImportChunkRequest {
                transfer_id: begun.transfer_id.clone(),
                file_index: first.file_index,
                chunk_index: first.chunk_index,
                data: corrupt,
            },
        )
        .unwrap_err();
    assert_eq!(rejected.code, 400);

    for missing in &begun.missing_chunks {
        let source_file = &manifest.files[missing.file_index as usize];
        let chunk = source
            .export_chunk(
                &common::ctx_user("member@example.com"),
                ExportChunkRequest {
                    track_id: "track-1".into(),
                    root_relative_path: source_file.root_relative_path.clone(),
                    chunk_index: missing.chunk_index,
                },
            )
            .unwrap();
        destination
            .import_chunk(
                &common::ctx_admin("admin@example.com"),
                ImportChunkRequest {
                    transfer_id: begun.transfer_id.clone(),
                    file_index: missing.file_index,
                    chunk_index: missing.chunk_index,
                    data: chunk.data,
                },
            )
            .unwrap();
    }
    let finished = destination
        .finish_import(
            &common::ctx_admin("admin@example.com"),
            FinishImportRequest {
                transfer_id: begun.transfer_id,
            },
        )
        .unwrap();

    assert!(finished.imported);
    let copied = destination_root.path().join("imports/friend/Test Artist/Test Album");
    assert_eq!(std::fs::read(copied.join("01.flac")).unwrap(), audio);
    assert_eq!(std::fs::read(copied.join("cover.jpg")).unwrap(), b"art");
    assert_eq!(std::fs::read(copied.join("01.lrc")).unwrap(), b"lyrics");
    assert!(!copied.join("album.m3u").exists());
    assert!(!copied.join("notes.txt").exists());

    let repeated = destination
        .begin_import(
            &common::ctx_admin("admin@example.com"),
            BeginImportRequest {
                library: Some(Library::Music),
                track_file_index: 0,
                files: destination_files,
            },
        )
        .unwrap();
    assert!(repeated.missing_chunks.is_empty());
    let repeated = destination
        .finish_import(
            &common::ctx_admin("admin@example.com"),
            FinishImportRequest {
                transfer_id: repeated.transfer_id,
            },
        )
        .unwrap();
    assert!(!repeated.imported);
    assert!(repeated.skipped_existing);
}

#[cfg(unix)]
#[test]
fn chunked_import_rejects_a_symlink_that_leaves_the_library() {
    let (destination, _pool, destination_root) = app_with_music_root();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), destination_root.path().join("imports")).unwrap();
    let data = b"audio";
    let error = destination
        .begin_import(
            &common::ctx_admin("admin@example.com"),
            BeginImportRequest {
                library: Some(Library::Music),
                track_file_index: 0,
                files: vec![TransferFile {
                    root_relative_path: "imports/friend/song.wav".into(),
                    content_type: "audio/wav".into(),
                    size_bytes: data.len() as u64,
                    sha256: hash(data),
                    chunks: vec![TransferChunk {
                        index: 0,
                        offset: 0,
                        size: data.len() as u64,
                        sha256: hash(data),
                    }],
                }],
            },
        )
        .unwrap_err();
    assert_eq!(error.code, 400);
    assert!(!outside.path().join("friend/song.wav").exists());
}
