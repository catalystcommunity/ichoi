//! Album-scoped image discovery. Persist image facts so browsing does not rescan audio files.
use crate::{
    artwork,
    db::{models, store},
};
use diesel::prelude::*;
use diesel::sql_types::{Nullable, Text};
use libichoi::csil::types::{ArtworkKind, CoverArt};
use lofty::{file::TaggedFile, picture::PictureType, prelude::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub role: String,
    source: PathBuf,
    picture: Option<usize>,
    fingerprint: String,
    quality: u32,
    tier: u8,
    convention: u8,
}

impl Candidate {
    pub fn eligible(&self) -> bool {
        self.tier < 9
    }
    pub fn bytes(&self) -> Option<Vec<u8>> {
        let data = if let Some(index) = self.picture {
            let tagged = crate::scan::read_tags(&self.source).ok()?;
            tagged
                .tags()
                .iter()
                .flat_map(|tag| tag.pictures())
                .nth(index)?
                .data()
                .to_vec()
        } else {
            artwork::read_bytes(&self.source)?
        };
        // A replaced file requires a new scan; never apply a different image under an old ID.
        (digest(&data) == self.fingerprint).then_some(data)
    }
}

#[derive(Clone)]
struct Facts {
    width: u32,
    height: u32,
    quality: u32,
}
#[derive(Default)]
pub struct Discovery {
    facts: HashMap<String, Option<Facts>>,
    embedded: HashMap<PathBuf, Vec<Candidate>>,
    files: HashMap<PathBuf, Vec<Candidate>>,
    original_albums: HashMap<PathBuf, String>,
}
fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn words(value: &str) -> Vec<String> {
    value
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
fn name(value: &str) -> String {
    words(value).join(" ")
}
fn is_auxiliary(tokens: &[String]) -> bool {
    tokens.iter().any(|token| {
        let base = token.trim_end_matches(|c: char| c.is_ascii_digit());
        matches!(
            base,
            "back"
                | "rear"
                | "disc"
                | "cd"
                | "label"
                | "inlay"
                | "tray"
                | "inside"
                | "interior"
                | "booklet"
                | "foldout"
                | "tracklisting"
                | "inline"
                | "spine"
                | "spread"
                | "icon"
                | "backup"
        )
    })
}
fn dimensions(bytes: &[u8]) -> Option<Facts> {
    let image = artwork::decode(bytes).ok()?.to_rgb8();
    let (width, height) = image.dimensions();
    // Exact pixel replication is an identifiable upscale. Do not reward those extra pixels.
    // Cap useful resolution at the stored/display size; size in bytes never affects rank.
    let mut scale = 1;
    for factor in [2, 3, 4, 5, 6, 8] {
        if width % factor != 0 || height % factor != 0 {
            continue;
        }
        let replicated = (0..height).all(|y| {
            (0..width).all(|x| {
                image.get_pixel(x, y) == image.get_pixel(x / factor * factor, y / factor * factor)
            })
        });
        if replicated {
            scale = factor;
        }
    }
    Some(Facts {
        width,
        height,
        quality: (width.min(height) / scale).min(1600),
    })
}
impl Discovery {
    fn inspect(
        &mut self,
        path: &Path,
        picture: Option<usize>,
        data: &[u8],
        role: &str,
        convention: u8,
    ) -> Option<Candidate> {
        let fingerprint = digest(data);
        let facts = self
            .facts
            .entry(fingerprint.clone())
            .or_insert_with(|| dimensions(data))
            .as_ref()?;
        let source_key = format!("{}:{picture:?}:{fingerprint}", path.display());
        Some(Candidate {
            id: digest(source_key.as_bytes()),
            label: path.file_name()?.to_string_lossy().into_owned(),
            width: facts.width,
            height: facts.height,
            quality: facts.quality,
            role: role.into(),
            source: path.into(),
            picture,
            fingerprint,
            tier: 9,
            convention,
        })
    }
    pub fn record(&mut self, path: &Path, tagged: &TaggedFile) {
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
        if let Some(album) = tag
            .and_then(|tag| tag.album())
            .filter(|title| !title.trim().is_empty())
        {
            self.original_albums.insert(path.into(), name(&album));
        }
        let mut candidates = Vec::new();
        for (index, picture) in tagged
            .tags()
            .iter()
            .flat_map(|tag| tag.pictures())
            .enumerate()
        {
            let (role, rank) = match picture.pic_type() {
                PictureType::CoverFront => ("Embedded front", 1),
                PictureType::Other if tagged.file_type() == lofty::file::FileType::Mp4 => {
                    ("Embedded front", 1)
                }
                PictureType::Other => ("Embedded image", 4),
                _ => ("Other embedded artwork", 9),
            };
            if let Some(mut candidate) = self.inspect(path, Some(index), picture.data(), role, rank)
            {
                candidate.label = format!("{} · {role} {}", candidate.label, index + 1);
                candidates.push(candidate);
            }
        }
        self.embedded.insert(path.into(), candidates);
    }
    fn in_folder(&mut self, folder: &Path) -> Vec<Candidate> {
        if let Some(cached) = self.files.get(folder) {
            return cached.clone();
        }
        let mut candidates = Vec::new();
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                    continue;
                }
                let path = entry.path();
                if !matches!(
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .as_str(),
                    "jpg" | "jpeg" | "png" | "webp" | "gif"
                ) {
                    continue;
                }
                let Some(data) = artwork::read_bytes(&path) else {
                    continue;
                };
                if let Some(candidate) = self.inspect(&path, None, &data, "Local image", 4) {
                    candidates.push(candidate);
                }
            }
        }
        candidates.sort_by(|a, b| a.source.cmp(&b.source));
        self.files.insert(folder.into(), candidates.clone());
        candidates
    }
}

fn album_folder(path: &Path, root: &Path, disc_words: &[String]) -> PathBuf {
    let folder = path.parent().unwrap_or(root);
    if folder != root
        && folder
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| crate::scan::is_disc_subfolder(n, disc_words))
    {
        if let Some(parent) = folder.parent().filter(|p| p.starts_with(root)) {
            return parent.into();
        }
    }
    folder.into()
}
fn folders(path: &Path, root: &Path, disc_words: &[String]) -> Vec<PathBuf> {
    let mut result = vec![
        path.parent().unwrap_or(root).to_path_buf(),
        album_folder(path, root, disc_words),
    ];
    result.sort();
    result.dedup();
    for folder in result.clone() {
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && matches!(
                        entry.file_name().to_string_lossy().to_lowercase().as_str(),
                        "art" | "artwork" | "cover" | "covers" | "scans" | "images"
                    )
                {
                    result.push(entry.path());
                }
            }
        }
    }
    result.sort();
    result.dedup();
    result
}
fn rank_file(candidate: &mut Candidate, title: &str, mixed: bool, unambiguous: bool) {
    let stem = candidate
        .source
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let tokens = words(&stem);
    let normalized = tokens.join(" ");
    let title = name(title);
    let title_match = title.len() > 2
        && (normalized == title
            || normalized.contains(&format!("{title} front"))
            || normalized.contains(&format!("{title} cover")));
    let sleeve = candidate.width > candidate.height.saturating_mul(17) / 10;
    if (is_auxiliary(&tokens) && !title_match) || sleeve {
        candidate.role = if sleeve {
            "Sleeve or wide artwork"
        } else {
            "Back, label, or booklet"
        }
        .into();
        return;
    }
    if mixed && !title_match {
        candidate.role = "Unassigned image in mixed folder".into();
        return;
    }
    let thumbnail = normalized == "albumartsmall"
        || tokens
            .iter()
            .any(|t| matches!(t.as_str(), "small" | "thumb" | "thumbnail"));
    let front = tokens
        .iter()
        .any(|t| matches!(t.as_str(), "front" | "cover"));
    let conventional = matches!(normalized.as_str(), "folder" | "album");
    let number_match = tokens.len() == 1
        && tokens[0].parse::<u32>().ok().is_some_and(|number| {
            words(&title).iter().any(|t| {
                t.trim_end_matches(|c: char| c.is_alphabetic())
                    .parse::<u32>()
                    .ok()
                    == Some(number)
            })
        });
    candidate.convention = if front {
        0
    } else if conventional {
        2
    } else if title_match {
        3
    } else {
        4
    };
    if thumbnail {
        candidate.tier = 4;
        candidate.role = "Thumbnail".into();
    } else if front || conventional || title_match {
        candidate.tier = 1;
        candidate.convention = if front {
            0
        } else if conventional {
            2
        } else {
            3
        };
        candidate.role = "Front cover".into();
    } else if number_match || unambiguous {
        candidate.tier = 2;
        candidate.role = if number_match {
            "Book-number match"
        } else {
            "Only plausible local image"
        }
        .into();
    }
    // Windows GUID images are deliberately not assigned by GUID order. A sole Large image
    // is eligible above; multiple unassigned GUIDs remain available in the picker only.
    if candidate.tier < 4 && candidate.quality <= 100 {
        candidate.tier = 4;
        candidate.role = "Thumbnail".into();
    }
}
fn sort_candidates(candidates: &mut [Candidate], previous: Option<&str>) {
    candidates.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then(b.quality.cmp(&a.quality))
            .then(a.convention.cmp(&b.convention))
            .then((Some(b.id.as_str()) == previous).cmp(&(Some(a.id.as_str()) == previous)))
            .then(a.id.cmp(&b.id))
    });
}

#[derive(QueryableByName)]
struct Catalog {
    #[diesel(sql_type=Text)]
    candidates: String,
    #[diesel(sql_type=Nullable<Text>)]
    selected_id: Option<String>,
}
fn catalog(conn: &mut SqliteConnection, id: &str) -> QueryResult<Option<Catalog>> {
    diesel::sql_query("SELECT candidates, selected_id FROM artwork_catalog WHERE album_id = ?")
        .bind::<Text, _>(id)
        .get_result(conn)
        .optional()
}

fn rebuild(
    conn: &mut SqliteConnection,
    album: &models::Album,
    tracks: &[models::Track],
    disc_words: &[String],
    discovery: &mut Discovery,
    ownership: &HashMap<PathBuf, HashSet<String>>,
) -> anyhow::Result<()> {
    let mut candidates = Vec::new();
    let mut seen_folders = HashSet::new();
    let mut source_titles = HashSet::new();
    for track in tracks {
        let Some(library) = store::get_library(conn, &track.library_id)? else {
            continue;
        };
        let path = Path::new(&library.path).join(&track.root_relative_path);
        if !discovery.embedded.contains_key(&path) {
            if let Ok(tagged) = crate::scan::read_tags(&path) {
                discovery.record(&path, &tagged);
            }
        }
        if let Some(title) = discovery.original_albums.get(&path) {
            source_titles.insert(title.clone());
        }
    }
    for track in tracks {
        let Some(library) = store::get_library(conn, &track.library_id)? else {
            continue;
        };
        let root = Path::new(&library.path);
        let path = root.join(&track.root_relative_path);
        let album_root = album_folder(&path, root, disc_words);
        let mixed =
            source_titles.len() > 1 || ownership.get(&album_root).is_some_and(|ids| ids.len() > 1);
        for mut candidate in discovery.embedded.get(&path).cloned().unwrap_or_default() {
            let associated =
                !mixed || discovery.original_albums.get(&path) == Some(&name(&album.title));
            if associated
                && candidate.convention < 9
                && candidate.width <= candidate.height.saturating_mul(17) / 10
            {
                candidate.tier = if candidate.quality <= 100 {
                    4
                } else if candidate.convention == 1 {
                    1
                } else {
                    2
                };
            }
            candidates.push(candidate);
        }
        for folder in folders(&path, root, disc_words) {
            if !seen_folders.insert(folder.clone()) {
                continue;
            }
            let mut local = discovery.in_folder(&folder);
            let plausible = local
                .iter()
                .filter(|c| {
                    !is_auxiliary(&words(
                        &c.source.file_stem().unwrap_or_default().to_string_lossy(),
                    )) && c.width <= c.height.saturating_mul(17) / 10
                        && !c.label.to_lowercase().contains("small")
                })
                .count();
            for candidate in &mut local {
                rank_file(candidate, &album.title, mixed, plausible == 1);
                candidate.label = candidate
                    .source
                    .strip_prefix(root)
                    .unwrap_or(&candidate.source)
                    .to_string_lossy()
                    .into_owned();
            }
            candidates.extend(local);
        }
    }
    let previous = catalog(conn, &album.id)?.and_then(|row| row.selected_id);
    sort_candidates(&mut candidates, previous.as_deref());
    let mut fingerprints = HashSet::new();
    candidates.retain(|c| fingerprints.insert(c.fingerprint.clone()));
    let selected = candidates
        .iter()
        .find(|c| c.eligible())
        .map(|c| c.id.clone());
    diesel::sql_query("INSERT INTO artwork_catalog (album_id,candidates,selected_id) VALUES (?,?,?) ON CONFLICT(album_id) DO UPDATE SET candidates=excluded.candidates, selected_id=excluded.selected_id")
        .bind::<Text,_>(&album.id).bind::<Text,_>(serde_json::to_string(&candidates)?).bind::<Nullable<Text>,_>(&selected).execute(conn)?;
    let cover_path = selected
        .as_ref()
        .and_then(|id| candidates.iter().find(|c| &c.id == id))
        .filter(|c| c.picture.is_none())
        .map(|c| c.source.to_string_lossy().into_owned());
    diesel::sql_query("UPDATE albums SET has_cover_art=?, cover_art_path=? WHERE id=?")
        .bind::<diesel::sql_types::Integer, _>(i32::from(selected.is_some()))
        .bind::<Nullable<Text>, _>(cover_path)
        .bind::<Text, _>(&album.id)
        .execute(conn)?;
    Ok(())
}

pub fn refresh_library(
    conn: &mut SqliteConnection,
    library_id: &str,
    words: &[String],
    discovery: &mut Discovery,
) -> anyhow::Result<()> {
    refresh_scope(conn, library_id, words, discovery, None)
}

pub fn refresh_import(
    conn: &mut SqliteConnection,
    library_id: &str,
    path: &Path,
    root: &Path,
    words: &[String],
    discovery: &mut Discovery,
) -> anyhow::Result<()> {
    let folder = album_folder(path, root, words);
    refresh_scope(conn, library_id, words, discovery, Some(&folder))
}

fn refresh_scope(
    conn: &mut SqliteConnection,
    library_id: &str,
    words: &[String],
    discovery: &mut Discovery,
    changed_folder: Option<&PathBuf>,
) -> anyhow::Result<()> {
    let mut albums: HashMap<String, Vec<models::Track>> = HashMap::new();
    let mut ownership: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let Some(library) = store::get_library(conn, library_id)? else {
        return Ok(());
    };
    for track in store::tracks_for_library(conn, library_id)? {
        if let Some(id) = &track.album_id {
            ownership
                .entry(album_folder(
                    &Path::new(&library.path).join(&track.root_relative_path),
                    Path::new(&library.path),
                    words,
                ))
                .or_default()
                .insert(id.clone());
            albums.entry(id.clone()).or_default().push(track);
        }
    }
    for id in albums.keys() {
        if changed_folder
            .is_some_and(|folder| !ownership.get(folder).is_some_and(|ids| ids.contains(id)))
        {
            continue;
        }
        if let Some(album) = store::get_album(conn, id)? {
            // Include all library copies of a stable album ID.
            let tracks = store::tracks_for_album(conn, id)?;
            rebuild(conn, &album, &tracks, words, discovery, &ownership)?;
        }
    }
    Ok(())
}
fn ensure(conn: &mut SqliteConnection, id: &str) -> anyhow::Result<()> {
    if catalog(conn, id)?.is_none() {
        let Some(album) = store::get_album(conn, id)? else {
            return Ok(());
        };
        let tracks = store::tracks_for_album(conn, id)?;
        let words = vec![
            "cd".into(),
            "disc".into(),
            "disk".into(),
            "bonus disc".into(),
        ];
        let mut ownership: HashMap<PathBuf, HashSet<String>> = HashMap::new();
        for library in store::list_libraries(conn)? {
            for track in store::tracks_for_library(conn, &library.id)? {
                if let Some(id) = track.album_id {
                    ownership
                        .entry(album_folder(
                            &Path::new(&library.path).join(&track.root_relative_path),
                            Path::new(&library.path),
                            &words,
                        ))
                        .or_default()
                        .insert(id);
                }
            }
        }
        rebuild(
            conn,
            &album,
            &tracks,
            &words,
            &mut Discovery::default(),
            &ownership,
        )?;
    }
    Ok(())
}
pub fn list(
    conn: &mut SqliteConnection,
    kind: &ArtworkKind,
    id: &str,
) -> anyhow::Result<Vec<(Candidate, bool)>> {
    let mut ids = vec![id.to_owned()];
    if matches!(kind, ArtworkKind::Artist) {
        ids.clear();
        for library in store::list_libraries(conn)? {
            ids.extend(
                store::albums_for_artist(conn, &library.id, id)?
                    .into_iter()
                    .map(|a| a.id),
            );
        }
        ids.sort();
        ids.dedup();
    }
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for id in ids {
        ensure(conn, &id)?;
        if let Some(row) = catalog(conn, &id)? {
            let candidates: Vec<Candidate> = serde_json::from_str(&row.candidates)?;
            for candidate in candidates {
                if seen.insert(candidate.fingerprint.clone()) {
                    let selected = row.selected_id.as_deref() == Some(&candidate.id);
                    result.push((candidate, selected));
                }
            }
        }
    }
    Ok(result)
}
pub fn selected(
    conn: &mut SqliteConnection,
    id: &str,
    size: u64,
) -> anyhow::Result<Option<CoverArt>> {
    for (candidate, selected) in list(conn, &ArtworkKind::Album, id)? {
        if selected {
            return Ok(candidate
                .bytes()
                .and_then(|data| artwork::normalize(&data, size).ok()));
        }
    }
    Ok(None)
}
pub fn candidate(
    conn: &mut SqliteConnection,
    kind: &ArtworkKind,
    id: &str,
    candidate_id: &str,
    size: u64,
) -> anyhow::Result<Option<CoverArt>> {
    Ok(list(conn, kind, id)?
        .into_iter()
        .find(|(c, _)| c.id == candidate_id)
        .and_then(|(c, _)| c.bytes())
        .and_then(|data| artwork::normalize(&data, size).ok()))
}

/// File-only discovery for callers without an indexed album. Full scans also compare embedded art.
pub fn file_candidates(path: &Path, root: &Path, disc_words: &[String]) -> Vec<PathBuf> {
    let mut discovery = Discovery::default();
    let album_folder = album_folder(path, root, disc_words);
    let title = album_folder
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let mut candidates: Vec<_> = folders(path, root, disc_words)
        .into_iter()
        .flat_map(|folder| discovery.in_folder(&folder))
        .collect();
    let plausible = candidates
        .iter()
        .filter(|c| !is_auxiliary(&words(&c.label)))
        .count();
    for candidate in &mut candidates {
        rank_file(candidate, &title, false, plausible == 1);
    }
    sort_candidates(&mut candidates, None);
    candidates
        .into_iter()
        .filter(|c| c.eligible())
        .map(|c| c.source)
        .collect()
}
