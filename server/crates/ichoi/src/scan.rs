//! Library scanner. Walks a library root, reads tags + technical properties, and upserts
//! artists/albums/tracks with stable content-derived ids (so re-scans update in place).
//!
//! Metadata comes from lofty where possible; for files lofty cannot parse (e.g. WMA/ASF,
//! which lofty has no reader for), it falls back to **ffprobe** (which reads them) for
//! duration + tags, and finally to filename/folder — so nothing in a messy library is lost.
//! Album grouping uses the album-artist and folder name so compilations and untagged files
//! don't fragment. Gapless trim extraction and content hashing are TODO.

use std::path::{Path, PathBuf};
use std::process::Command;

use diesel::SqliteConnection;
use lofty::file::{AudioFile, FileType, TaggedFile};
use lofty::prelude::*;
use lofty::tag::ItemKey;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::db::{models, store};

const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "m4a", "mp4", "aac", "ogg", "oga", "opus", "wav", "wave", "wma",
];

#[derive(Debug, Default)]
pub struct ScanStats {
    pub tracks: usize,
    pub errors: usize,
}

/// Normalized per-file metadata, from lofty or the ffprobe/filename fallback.
struct FileMeta {
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    track_no: Option<i32>,
    disc_no: Option<i32>,
    year: Option<i32>,
    codec: String,
    duration_ms: i64,
    bitrate_kbps: Option<i32>,
    sample_rate: i32,
    channels: i32,
    bit_depth: Option<i32>,
    has_embedded_art: bool,
    /// True when `album` came from a real tag (not a folder-name / "Unknown" fallback).
    album_tagged: bool,
}

/// Scan every audio file under `root`, indexing into `library_id`.
pub fn scan_library(
    conn: &mut SqliteConnection,
    library_id: &str,
    root: &Path,
    split_dumps: bool,
) -> anyhow::Result<ScanStats> {
    let ffprobe = resolve_ffprobe();
    let mut stats = ScanStats::default();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        if !AUDIO_EXTS.contains(&ext.as_str()) {
            continue;
        }
        match index_file(conn, library_id, root, path, &ext, ffprobe.as_deref()) {
            Ok(()) => stats.tracks += 1,
            Err(e) => {
                log::warn!("scan: {}: {e}", path.display());
                stats.errors += 1;
            }
        }
    }
    // An "album" with too few tracks is usually mis-tagged loose files; regroup those by
    // their containing folder instead (§ album detection).
    if let Err(e) = consolidate_small_albums(conn, MIN_ALBUM_TRACKS, split_dumps) {
        log::warn!("scan: album consolidation: {e}");
    }
    if let Err(e) = set_folder_album_artists(conn) {
        log::warn!("scan: folder-album artists: {e}");
    }
    Ok(stats)
}

/// Give each artist-less (folder-derived) album an artist when all its tracks share one;
/// otherwise leave it null (a mixed folder reads as "Various").
fn set_folder_album_artists(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    for album in store::albums_missing_artist(conn)? {
        let artists: std::collections::HashSet<String> = store::tracks_for_album(conn, &album.id)?
            .into_iter()
            .filter_map(|t| t.artist_id)
            .collect();
        if artists.len() == 1 {
            if let Some(a) = artists.into_iter().next() {
                store::set_album_artist(conn, &album.id, &a)?;
            }
        }
    }
    Ok(())
}

/// Albums with fewer than this many tracks are dissolved into folder-based albums.
const MIN_ALBUM_TRACKS: i64 = 4;
/// A folder is a "dump" (not an album) if it holds more than this many loose tracks, or
/// spans at least this many distinct album-artists.
const DUMP_TRACKS: usize = 30;
const DUMP_ARTISTS: usize = 4;

struct FolderEntry {
    track_id: String,
    library_id: String,
    album_artist: Option<String>,
    year: Option<i32>,
}
struct FolderGroup {
    title: String,
    has_cover: i32,
    cover_path: Option<String>,
    art_checked: i32,
    entries: Vec<FolderEntry>,
}

/// Reassign tracks of under-populated albums. By default they become a folder-based album;
/// with `split_dumps`, folders that look like mixed "dumps" instead route each track to a
/// per-artist "Singles" album.
fn consolidate_small_albums(
    conn: &mut SqliteConnection,
    min_tracks: i64,
    split_dumps: bool,
) -> anyhow::Result<()> {
    let small: Vec<String> = store::album_track_counts(conn)?
        .into_iter()
        .filter(|(_, n)| *n < min_tracks)
        .map(|(id, _)| id)
        .collect();

    // Group every small-album track by its containing folder.
    let mut folders: std::collections::HashMap<String, FolderGroup> = std::collections::HashMap::new();
    for album_id in small {
        let Some(album) = store::get_album(conn, &album_id)? else {
            continue;
        };
        for track in store::tracks_for_album(conn, &album_id)? {
            let (folder_rel, folder_title) = folder_of(&track.root_relative_path);
            let group = folders.entry(folder_rel).or_insert_with(|| FolderGroup {
                title: folder_title,
                has_cover: 0,
                cover_path: None,
                art_checked: album.art_checked,
                entries: Vec::new(),
            });
            if group.has_cover == 0 && album.has_cover_art == 1 {
                group.has_cover = 1;
                group.cover_path = album.cover_art_path.clone();
            }
            group.entries.push(FolderEntry {
                track_id: track.id,
                library_id: track.library_id,
                album_artist: album.artist_id.clone(),
                year: album.year,
            });
        }
    }

    for (folder_rel, group) in folders {
        let distinct_artists = group
            .entries
            .iter()
            .map(|e| e.album_artist.clone())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let is_dump = group.entries.len() > DUMP_TRACKS || distinct_artists >= DUMP_ARTISTS;

        if split_dumps && is_dump {
            // Per-artist "Singles" album.
            for e in &group.entries {
                let key = e.album_artist.clone().unwrap_or_default();
                let singles_id = id_of(&["singles", &key]);
                store::upsert_album(
                    conn,
                    &models::Album {
                        id: singles_id.clone(),
                        title: "Singles".to_string(),
                        artist_id: e.album_artist.clone(),
                        year: None,
                        has_cover_art: 0,
                        cover_art_path: None,
                        art_checked: 1,
                    },
                )?;
                store::set_track_album(conn, &e.track_id, &singles_id)?;
            }
        } else {
            // One folder-album for the whole folder.
            let library_id = group
                .entries
                .first()
                .map(|e| e.library_id.clone())
                .unwrap_or_default();
            let folder_id = id_of(&["folderalbum", &library_id, &folder_rel]);
            store::upsert_album(
                conn,
                &models::Album {
                    id: folder_id.clone(),
                    title: group.title,
                    artist_id: group.entries.first().and_then(|e| e.album_artist.clone()),
                    year: group.entries.first().and_then(|e| e.year),
                    has_cover_art: group.has_cover,
                    cover_art_path: group.cover_path,
                    art_checked: group.art_checked,
                },
            )?;
            for e in &group.entries {
                store::set_track_album(conn, &e.track_id, &folder_id)?;
            }
        }
    }
    store::delete_empty_albums(conn)?;
    Ok(())
}

/// `(parent_folder_relpath, folder_display_name)` for a root-relative track path.
fn folder_of(rel: &str) -> (String, String) {
    let parent = Path::new(rel)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let title = Path::new(&parent)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "Unknown Album".to_string());
    (parent, title)
}

fn index_file(
    conn: &mut SqliteConnection,
    library_id: &str,
    root: &Path,
    path: &Path,
    ext: &str,
    ffprobe: Option<&Path>,
) -> anyhow::Result<()> {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    // lofty first; on failure (unsupported format like WMA, or a malformed file) fall back.
    let m = match lofty::read_from_path(path) {
        Ok(tagged) => meta_from_lofty(&tagged, path, ext),
        Err(_) => meta_fallback(path, ext, ffprobe),
    };

    let meta = std::fs::metadata(path).ok();
    let size_bytes = meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();

    let artist_id = id_of(&["artist", &canonical_artist(&m.artist)]);
    let track_id = id_of(&["track", library_id, &rel]);
    let folder_cover = find_cover(path);

    store::upsert_artist(
        conn,
        &models::Artist {
            id: artist_id.clone(),
            name: m.artist.clone(),
        },
    )?;

    // Album identity. A real album tag groups by (album-artist, canonical title) — where
    // canonicalization collapses "Album", "Album (2001)", "Album [Remastered]" but keeps
    // "Vol. 1" vs "Vol. 2" distinct. A *blank* tag falls back to the folder, keyed by the
    // folder path so every file in that folder joins ONE album regardless of per-track
    // artist (no duplicate same-name albums); its artist is inferred post-scan.
    let (album_id, album_title, album_artist): (String, String, Option<String>) = if m.album_tagged
    {
        let aa_key = canonical_artist(&m.album_artist);
        let a_key = {
            let k = canonical_album(&m.album);
            if k.is_empty() {
                m.album.to_lowercase()
            } else {
                k
            }
        };
        let aa_id = id_of(&["artist", &aa_key]);
        if aa_id != artist_id {
            store::upsert_artist(
                conn,
                &models::Artist {
                    id: aa_id.clone(),
                    name: m.album_artist.clone(),
                },
            )?;
        }
        (id_of(&["album", &aa_key, &a_key]), m.album.clone(), Some(aa_id))
    } else {
        let (folder_rel, folder_title) = folder_of(&rel);
        (
            id_of(&["folderalbum", library_id, &folder_rel]),
            folder_title,
            None,
        )
    };

    store::upsert_album(
        conn,
        &models::Album {
            id: album_id.clone(),
            title: album_title,
            artist_id: album_artist,
            year: m.year,
            has_cover_art: i32::from(folder_cover.is_some() || m.has_embedded_art),
            cover_art_path: folder_cover,
            art_checked: 0,
        },
    )?;
    store::upsert_track(
        conn,
        &models::Track {
            id: track_id,
            library_id: library_id.to_string(),
            root_relative_path: rel,
            title: m.title,
            artist_id: Some(artist_id),
            album_id: Some(album_id),
            track_no: m.track_no,
            disc_no: m.disc_no,
            duration_ms: m.duration_ms,
            codec: m.codec,
            bitrate_kbps: m.bitrate_kbps,
            sample_rate: m.sample_rate,
            channels: m.channels,
            bit_depth: m.bit_depth,
            size_bytes,
            mtime,
            content_hash: None,
            trim_start_samples: 0,
            trim_end_samples: 0,
        },
    )?;
    Ok(())
}

fn clean(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn folder_name(path: &Path) -> Option<String> {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .and_then(clean)
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "Unknown".to_string())
}

fn meta_from_lofty(tagged: &TaggedFile, path: &Path, ext: &str) -> FileMeta {
    let props = tagged.properties();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let title = tag
        .and_then(|t| t.title())
        .and_then(|c| clean(&c))
        .unwrap_or_else(|| file_stem(path));
    let artist = tag
        .and_then(|t| t.artist())
        .and_then(|c| clean(&c))
        .unwrap_or_else(|| "Unknown Artist".to_string());
    let tagged_album = tag.and_then(|t| t.album()).and_then(|c| clean(&c));
    let album_tagged = tagged_album.is_some();
    let album = tagged_album
        .or_else(|| folder_name(path))
        .unwrap_or_else(|| "Unknown Album".to_string());
    let album_artist = tag
        .and_then(|t| t.get_string(&ItemKey::AlbumArtist))
        .and_then(clean)
        .unwrap_or_else(|| artist.clone());

    FileMeta {
        title,
        artist,
        album,
        album_artist,
        track_no: tag.and_then(|t| t.track()).map(|n| n as i32),
        disc_no: tag.and_then(|t| t.disk()).map(|n| n as i32),
        year: tag.and_then(|t| t.year()).map(|n| n as i32),
        codec: codec_for(tagged.file_type(), ext),
        duration_ms: props.duration().as_millis() as i64,
        bitrate_kbps: props.audio_bitrate().map(|b| b as i32),
        sample_rate: props.sample_rate().unwrap_or(0) as i32,
        channels: props.channels().unwrap_or(0) as i32,
        bit_depth: props.bit_depth().map(|b| b as i32),
        has_embedded_art: tag.map(|t| !t.pictures().is_empty()).unwrap_or(false),
        album_tagged,
    }
}

/// Fallback for files lofty can't parse: filename/folder metadata, enriched by ffprobe
/// (duration + tags) when it's available.
fn meta_fallback(path: &Path, ext: &str, ffprobe: Option<&Path>) -> FileMeta {
    let mut m = FileMeta {
        title: file_stem(path),
        artist: "Unknown Artist".to_string(),
        album: folder_name(path).unwrap_or_else(|| "Unknown Album".to_string()),
        album_artist: "Unknown Artist".to_string(),
        track_no: None,
        disc_no: None,
        year: None,
        codec: codec_from_ext(ext),
        duration_ms: 0,
        bitrate_kbps: None,
        sample_rate: 0,
        channels: 0,
        bit_depth: None,
        has_embedded_art: false,
        album_tagged: false,
    };

    if let Some(probe) = ffprobe {
        if let Some(json) = ffprobe_json(probe, path) {
            apply_ffprobe(&mut m, &json);
        }
    }
    if m.album_artist == "Unknown Artist" {
        m.album_artist = m.artist.clone();
    }
    m
}

fn ffprobe_json(ffprobe: &Path, path: &Path) -> Option<serde_json::Value> {
    let out = Command::new(ffprobe)
        .args(["-v", "quiet", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

fn apply_ffprobe(m: &mut FileMeta, json: &serde_json::Value) {
    let fmt = &json["format"];
    if let Some(d) = fmt["duration"].as_str().and_then(|s| s.parse::<f64>().ok()) {
        m.duration_ms = (d * 1000.0) as i64;
    }
    if let Some(br) = fmt["bit_rate"].as_str().and_then(|s| s.parse::<i64>().ok()) {
        m.bitrate_kbps = Some((br / 1000) as i32);
    }
    // Tags are case-insensitive across containers; index them lowercased.
    if let Some(tags) = fmt["tags"].as_object() {
        let get = |key: &str| -> Option<String> {
            tags.iter()
                .find(|(k, _)| k.to_ascii_lowercase() == key)
                .and_then(|(_, v)| v.as_str())
                .and_then(clean)
        };
        if let Some(t) = get("title") {
            m.title = t;
        }
        if let Some(a) = get("artist") {
            m.artist = a;
        }
        if let Some(al) = get("album") {
            m.album = al;
            m.album_tagged = true;
        }
        if let Some(aa) = get("album_artist").or_else(|| get("wm/albumartist")) {
            m.album_artist = aa;
        }
        if let Some(tn) = get("track").and_then(|s| leading_int(&s)) {
            m.track_no = Some(tn);
        }
        if let Some(y) = get("date").or_else(|| get("year")).and_then(|s| leading_int(&s)) {
            m.year = Some(y);
        }
    }
    // Prefer the audio stream's sample rate / channels if present.
    if let Some(streams) = json["streams"].as_array() {
        if let Some(audio) = streams
            .iter()
            .find(|s| s["codec_type"].as_str() == Some("audio"))
        {
            if let Some(sr) = audio["sample_rate"].as_str().and_then(|s| s.parse::<i32>().ok()) {
                m.sample_rate = sr;
            }
            if let Some(ch) = audio["channels"].as_i64() {
                m.channels = ch as i32;
            }
        }
    }
}

fn leading_int(s: &str) -> Option<i32> {
    let digits: String = s.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn codec_for(ft: FileType, ext: &str) -> String {
    match ft {
        FileType::Flac => "flac".to_string(),
        FileType::Mpeg => "mp3".to_string(),
        FileType::Vorbis => "vorbis".to_string(),
        FileType::Opus => "opus".to_string(),
        FileType::Wav => "wav".to_string(),
        FileType::Mp4 | FileType::Aac => "aac".to_string(),
        _ => codec_from_ext(ext),
    }
}

fn codec_from_ext(ext: &str) -> String {
    match ext {
        "opus" => "opus",
        "ogg" | "oga" => "vorbis",
        "flac" => "flac",
        "wav" | "wave" => "wav",
        "m4a" | "mp4" | "aac" => "aac",
        "wma" => "wma",
        _ => "mp3",
    }
    .to_string()
}

fn resolve_ffprobe() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" });
            if bundled.is_file() {
                return Some(bundled);
            }
        }
    }
    let name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(name))
            .find(|p| p.is_file())
    })
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_brackets(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn alnum_spaces(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect()
}

fn canonical_artist(name: &str) -> String {
    let mut t = name.to_lowercase();
    if let Some(rest) = t.strip_prefix("the ") {
        t = rest.to_string();
    }
    collapse_ws(&alnum_spaces(&t))
}

fn canonical_album(title: &str) -> String {
    let mut t = strip_brackets(&title.to_lowercase());
    // Drop trailing "disc N" / "cd N" markers (multi-disc = same album).
    for marker in [" disc ", " cd ", " disk "] {
        if let Some(idx) = t.find(marker) {
            let after = &t[idx + marker.len()..];
            if after.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                t.truncate(idx);
            }
        }
    }
    for word in [
        "remastered",
        "remaster",
        "deluxe",
        "expanded",
        "anniversary",
        "explicit",
        "bonus tracks",
        "bonus track",
        "special edition",
    ] {
        t = t.replace(word, " ");
    }
    collapse_ws(&alnum_spaces(&t))
}

fn id_of(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    hex::encode(&hasher.finalize()[..16])
}

/// Folder image, in the conventional precedence other players use: `folder.*` first, then
/// `cover.*`, `front.*`, `album.*`, across jpg/jpeg/png/gif.
fn find_cover(track_path: &Path) -> Option<String> {
    let dir = track_path.parent()?;
    for stem in ["folder", "cover", "front", "album"] {
        for ext in ["jpg", "jpeg", "png", "gif"] {
            let candidate = dir.join(format!("{stem}.{ext}"));
            if candidate.exists() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// The single folder shared by all of an album's tracks, if there is one (used to place a
/// fetched cover image, §art). `None` for albums whose tracks are scattered across folders.
pub fn common_folder(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut parents = paths.iter().filter_map(|p| p.parent());
    let first = parents.next()?.to_path_buf();
    if parents.all(|p| p == first) {
        Some(first)
    } else {
        None
    }
}

/// Extract the first embedded cover-art picture from a track file, as `(mime, bytes)`.
pub fn extract_embedded_cover(path: &Path) -> Option<(String, Vec<u8>)> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let pic = tag.pictures().first()?;
    let mime = pic
        .mime_type()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "image/jpeg".to_string());
    Some((mime, pic.data().to_vec()))
}
