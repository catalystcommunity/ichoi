//! External cover-art fill-in.
//!
//! For albums with no folder image, look up the release on **MusicBrainz** and fetch the
//! **Cover Art Archive** front image, then save it into the album's folder as `cover.jpg` —
//! the same file-in-folder method the scanner already reads (§ cover art). **Music files are
//! never modified.** Albums whose tracks are scattered across folders are skipped (there is
//! no single folder to place the image in). Respects MusicBrainz's ~1 request/second policy
//! and sends a descriptive User-Agent, as their API requires.

use std::path::PathBuf;
use std::time::Duration;

use diesel::SqliteConnection;

use crate::db::store;
use crate::scan;

const USER_AGENT: &str = "Ichoi/0.0.0 (https://github.com/catalystcommunity/ichoi)";

#[derive(Debug, Default)]
pub struct ArtStats {
    pub fetched: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Fetch cover art for up to `limit` albums that currently have none.
pub fn fetch_missing(conn: &mut SqliteConnection, limit: usize) -> anyhow::Result<ArtStats> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(25))
        .build()?;

    let albums = store::albums_without_cover(conn, limit as i64)?;
    log::info!("art: {} albums missing cover art", albums.len());
    let mut stats = ArtStats::default();

    for album in albums {
        // Resolve the album's folder from its tracks' absolute paths.
        let tracks = store::tracks_for_album(conn, &album.id)?;
        let mut paths: Vec<PathBuf> = Vec::new();
        for t in &tracks {
            if let Some(lib) = store::get_library(conn, &t.library_id)? {
                paths.push(PathBuf::from(lib.path).join(&t.root_relative_path));
            }
        }
        let Some(folder) = scan::common_folder(&paths) else {
            // Scattered across folders: nowhere to place a cover. Mark checked so we don't
            // recompute it every startup.
            store::mark_art_checked(conn, &album.id)?;
            stats.skipped += 1;
            continue;
        };
        let dest = folder.join("cover.jpg");
        if dest.exists() {
            store::set_album_cover(conn, &album.id, &dest.to_string_lossy())?;
            stats.skipped += 1;
            continue;
        }

        let artist = album
            .artist_id
            .as_ref()
            .and_then(|id| store::get_artist(conn, id).ok().flatten())
            .map(|a| a.name)
            .unwrap_or_default();

        match fetch_one(&client, &album.title, &artist) {
            Ok(Some(bytes)) => {
                std::fs::write(&dest, &bytes)?;
                store::set_album_cover(conn, &album.id, &dest.to_string_lossy())?;
                stats.fetched += 1;
                log::info!("art: saved {} — {}", artist, album.title);
            }
            Ok(None) => {
                store::mark_art_checked(conn, &album.id)?;
                stats.failed += 1;
            }
            Err(e) => {
                log::warn!("art: {artist} — {}: {e}", album.title);
                store::mark_art_checked(conn, &album.id)?;
                stats.failed += 1;
            }
        }

        // MusicBrainz asks for no more than one request per second.
        std::thread::sleep(Duration::from_millis(1100));
    }
    Ok(stats)
}

fn fetch_one(
    client: &reqwest::blocking::Client,
    album: &str,
    artist: &str,
) -> anyhow::Result<Option<Vec<u8>>> {
    if album.is_empty() || album.starts_with("Unknown") {
        return Ok(None);
    }
    let mut query = format!("release:\"{}\"", album.replace('"', ""));
    if !artist.is_empty() && !artist.starts_with("Unknown") {
        query.push_str(&format!(" AND artist:\"{}\"", artist.replace('"', "")));
    }

    let resp: serde_json::Value = client
        .get("https://musicbrainz.org/ws/2/release/")
        .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "1")])
        .send()?
        .error_for_status()?
        .json()?;

    let Some(mbid) = resp["releases"][0]["id"].as_str() else {
        return Ok(None);
    };

    let art = client
        .get(format!("https://coverartarchive.org/release/{mbid}/front"))
        .send()?;
    if art.status().is_success() {
        Ok(Some(art.bytes()?.to_vec()))
    } else {
        Ok(None)
    }
}
