//! DataUtils test harness (§12): tests run against a real SQLite database inside a
//! transaction that is rolled back when the pool drops. Copied module, per house
//! convention (each integration-test file is its own crate).
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ichoi::config::{Config, Role};
use ichoi::db::{self, models, store, SqlitePool};
use ichoi::handlers::{App, Ctx, Identity};

/// Overrides map: a test specifies only the fields it cares about; the rest are defaulted.
pub type DataMap = HashMap<String, String>;

pub fn test_config() -> Config {
    Config {
        role: Role::Core,
        music_dir: None,
        audiobook_dir: None,
        db_dir: None,
        http_addr: "127.0.0.1:0".to_string(),
        csil_addr: "127.0.0.1:0".to_string(),
        core_addr: None,
        core_keys: vec![],
        tls_cert: None,
        tls_key: None,
        node_token: None,
        admin_token: Some("test-admin-token".to_string()),
        ffmpeg: None,
        transcode_codec: "aac".to_string(),
        web_dir: PathBuf::from("."),
        log: "warn".to_string(),
        fetch_art: false,
        split_dump_folders: false,
        album_subfolder_flat: true,
        album_subfolder_words: vec![
            "cd".into(),
            "disc".into(),
            "disk".into(),
            "bonus disc".into(),
        ],
        require_music: false,
        linkkeys_local_rp: false,
        linkkeys_local_rp_name: None,
        linkkeys_trusted_identities: vec![],
    }
}

/// A fresh app over a rolled-back test database, plus the pool for direct seeding.
pub fn test_app() -> (App, SqlitePool) {
    let pool = db::test_pool();
    let app = App::new(pool.clone(), Arc::new(test_config()));
    (app, pool)
}

pub fn ctx_anon() -> Ctx {
    Ctx {
        identity: Identity::Anonymous,
    }
}

pub fn ctx_user(account_id: &str) -> Ctx {
    Ctx {
        identity: Identity::User {
            account_id: account_id.to_string(),
            role: "member".to_string(),
        },
    }
}

pub fn ctx_admin(account_id: &str) -> Ctx {
    Ctx {
        identity: Identity::User {
            account_id: account_id.to_string(),
            role: "admin".to_string(),
        },
    }
}

pub fn ctx_node(node_id: &str) -> Ctx {
    Ctx {
        identity: Identity::Node {
            node_id: node_id.to_string(),
        },
    }
}

fn ov<'a>(m: &'a DataMap, key: &str, default: &'a str) -> String {
    m.get(key).cloned().unwrap_or_else(|| default.to_string())
}

pub fn create_artist(conn: &mut diesel::SqliteConnection, o: &DataMap) -> models::Artist {
    let row = models::Artist {
        id: ov(o, "id", "artist-1"),
        name: ov(o, "name", "Test Artist"),
    };
    store::upsert_artist(conn, &row).expect("create artist");
    row
}

pub fn create_album(conn: &mut diesel::SqliteConnection, o: &DataMap) -> models::Album {
    let row = models::Album {
        id: ov(o, "id", "album-1"),
        title: ov(o, "title", "Test Album"),
        artist_id: Some(ov(o, "artist_id", "artist-1")),
        year: Some(2020),
        has_cover_art: 0,
        cover_art_path: None,
        art_checked: 0,
    };
    store::upsert_album(conn, &row).expect("create album");
    row
}

pub fn ensure_library(conn: &mut diesel::SqliteConnection, id: &str) {
    let kind = if id == "lib:audiobook" {
        "audiobook"
    } else {
        "music"
    };
    store::upsert_library(
        conn,
        &models::Library {
            id: id.to_string(),
            kind: kind.to_string(),
            path: ".".to_string(),
        },
    )
    .expect("ensure library");
}

pub fn create_track(conn: &mut diesel::SqliteConnection, o: &DataMap) -> models::Track {
    let library_id = ov(o, "library_id", "lib:music");
    ensure_library(conn, &library_id);
    let row = models::Track {
        id: ov(o, "id", "track-1"),
        library_id: library_id.clone(),
        root_relative_path: ov(o, "root_relative_path", "Test Artist/Test Album/01.flac"),
        title: ov(o, "title", "Test Song"),
        artist_id: Some(ov(o, "artist_id", "artist-1")),
        album_id: Some(ov(o, "album_id", "album-1")),
        track_no: Some(1),
        disc_no: None,
        duration_ms: 60_000,
        codec: ov(o, "codec", "flac"),
        bitrate_kbps: Some(900),
        sample_rate: 44_100,
        channels: 2,
        bit_depth: Some(16),
        size_bytes: 1_000_000,
        mtime: "0".to_string(),
        content_hash: None,
        trim_start_samples: 0,
        trim_end_samples: 0,
    };
    store::upsert_track(conn, &row).expect("create track");
    row
}
