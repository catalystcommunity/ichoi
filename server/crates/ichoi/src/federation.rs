//! Bounded, client-mediated file transfer for instance federation.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use libichoi::csil::types::*;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::db::{models, store};

pub const CHUNK_SIZE: usize = 16 * 1024 * 1024;
const MAX_FILES: usize = 1024;
const SESSION_LIFETIME: Duration = Duration::from_secs(60 * 60);

fn err(code: i64, message: impl Into<String>) -> ServiceError {
    ServiceError {
        code,
        message: message.into(),
    }
}

fn internal(error: impl std::fmt::Display) -> ServiceError {
    err(500, format!("internal: {error}"))
}

fn sha256_bytes(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn sha256_file(path: &Path) -> Result<String, ServiceError> {
    let mut reader = BufReader::new(File::open(path).map_err(internal)?);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; CHUNK_SIZE];
    loop {
        let count = reader.read(&mut buffer).map_err(internal)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn safe_relative_path(value: &str) -> Result<PathBuf, ServiceError> {
    if value.is_empty() || value.len() > 1024 || value.contains('\\') || value.contains('\0') {
        return Err(err(400, "invalid transfer path"));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(err(400, "transfer path must stay inside the library"));
    }
    Ok(path.to_path_buf())
}

pub(crate) fn destination_path(root: &Path, relative: &Path) -> Result<PathBuf, ServiceError> {
    std::fs::create_dir_all(root).map_err(internal)?;
    let canonical_root = root.canonicalize().map_err(internal)?;
    let destination = root.join(relative);
    if std::fs::symlink_metadata(&destination)
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(err(400, "transfer destination cannot be a symbolic link"));
    }
    let mut ancestor = destination.parent().unwrap_or(root);
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| err(400, "transfer destination is outside the library"))?;
    }
    let canonical_ancestor = ancestor.canonicalize().map_err(internal)?;
    if !canonical_ancestor.starts_with(&canonical_root) {
        return Err(err(400, "transfer destination is outside the library"));
    }
    Ok(destination)
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("flac") => "audio/flac",
        Some("mp3") => "audio/mpeg",
        Some("aac") => "audio/aac",
        Some("wav" | "wave") => "audio/wav",
        Some("ogg" | "oga" | "opus") => "audio/ogg",
        Some("m4a" | "mp4") => "audio/mp4",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("lrc") => "text/plain",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

fn is_sidecar(path: &Path, track_stem: &str) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if matches!(
        extension.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "lrc"
    ) {
        return true;
    }
    if extension != "txt" {
        return false;
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    stem.eq_ignore_ascii_case(track_stem) || stem.eq_ignore_ascii_case("lyrics")
}

fn export_paths(root: &Path, track_relative_path: &str) -> Result<Vec<PathBuf>, ServiceError> {
    let relative = safe_relative_path(track_relative_path)?;
    let primary = root.join(&relative);
    if !primary.is_file()
        || std::fs::symlink_metadata(&primary)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(err(404, "track file not found"));
    }
    let parent = primary
        .parent()
        .ok_or_else(|| err(400, "track has no folder"))?;
    let track_stem = primary
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut sidecars = std::fs::read_dir(parent)
        .map_err(internal)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| path != &primary && is_sidecar(path, track_stem))
        .collect::<Vec<_>>();
    sidecars.sort();
    let mut paths = vec![primary];
    paths.extend(sidecars);
    Ok(paths)
}

fn manifest_file(root: &Path, path: &Path) -> Result<TransferFile, ServiceError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| err(400, "export file is outside the library"))?
        .to_string_lossy()
        .replace('\\', "/");
    let mut reader = BufReader::new(File::open(path).map_err(internal)?);
    let mut file_hasher = Sha256::new();
    let mut chunks = Vec::new();
    let mut buffer = vec![0; CHUNK_SIZE];
    let mut offset = 0_u64;
    loop {
        let count = reader.read(&mut buffer).map_err(internal)?;
        if count == 0 {
            break;
        }
        let data = &buffer[..count];
        file_hasher.update(data);
        chunks.push(TransferChunk {
            index: chunks.len() as u64,
            offset,
            size: count as u64,
            sha256: sha256_bytes(data),
        });
        offset += count as u64;
    }
    Ok(TransferFile {
        root_relative_path: relative,
        content_type: content_type(path).to_string(),
        size_bytes: offset,
        sha256: hex::encode(file_hasher.finalize()),
        chunks,
    })
}

pub fn export_manifest(
    conn: &mut diesel::SqliteConnection,
    track_id: &str,
) -> Result<ExportManifest, ServiceError> {
    let row = store::get_track(conn, track_id)
        .map_err(internal)?
        .ok_or_else(|| err(404, "track not found"))?;
    let library = store::get_library(conn, &row.library_id)
        .map_err(internal)?
        .ok_or_else(|| err(404, "library not found"))?;
    let root = Path::new(&library.path);
    let files = export_paths(root, &row.root_relative_path)?
        .iter()
        .map(|path| manifest_file(root, path))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(primary) = files.first() {
        store::set_track_content_hash(conn, &row.id, &primary.sha256).map_err(internal)?;
    }
    let mut row = row;
    row.content_hash = files.first().map(|file| file.sha256.clone());
    Ok(ExportManifest {
        track: crate::handlers::track_from_model(&row),
        files,
    })
}

pub fn export_chunk(
    conn: &mut diesel::SqliteConnection,
    input: &ExportChunkRequest,
) -> Result<ExportChunk, ServiceError> {
    let row = store::get_track(conn, &input.track_id)
        .map_err(internal)?
        .ok_or_else(|| err(404, "track not found"))?;
    let library = store::get_library(conn, &row.library_id)
        .map_err(internal)?
        .ok_or_else(|| err(404, "library not found"))?;
    let root = Path::new(&library.path);
    let requested = safe_relative_path(&input.root_relative_path)?;
    let allowed = export_paths(root, &row.root_relative_path)?
        .into_iter()
        .any(|path| path.strip_prefix(root).ok() == Some(requested.as_path()));
    if !allowed {
        return Err(err(404, "export file not found"));
    }
    let path = root.join(&requested);
    let length = path.metadata().map_err(internal)?.len();
    let offset = input
        .chunk_index
        .checked_mul(CHUNK_SIZE as u64)
        .ok_or_else(|| err(400, "chunk index is too large"))?;
    if offset >= length && !(length == 0 && input.chunk_index == 0) {
        return Err(err(416, "chunk is outside the file"));
    }
    let count = (length.saturating_sub(offset)).min(CHUNK_SIZE as u64) as usize;
    let mut file = File::open(path).map_err(internal)?;
    file.seek(SeekFrom::Start(offset)).map_err(internal)?;
    let mut data = vec![0; count];
    file.read_exact(&mut data).map_err(internal)?;
    Ok(ExportChunk {
        root_relative_path: input.root_relative_path.clone(),
        chunk_index: input.chunk_index,
        data,
    })
}

#[derive(Clone)]
struct ImportFileState {
    manifest: TransferFile,
    destination: PathBuf,
    temporary: PathBuf,
    received: HashSet<u64>,
    exists: bool,
}

struct ImportTransfer {
    owner: String,
    created: Instant,
    library_id: String,
    library_kind: Library,
    root: PathBuf,
    track_file_index: usize,
    primary_new: bool,
    temporary_dir: PathBuf,
    files: Vec<ImportFileState>,
}

#[derive(Clone, Default)]
pub struct ImportHub {
    inner: Arc<Mutex<HashMap<String, ImportTransfer>>>,
}

fn validate_manifest(file: &TransferFile) -> Result<PathBuf, ServiceError> {
    let path = safe_relative_path(&file.root_relative_path)?;
    if file.sha256.len() != 64 || !file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(err(400, "invalid file hash"));
    }
    let mut offset = 0_u64;
    for (index, chunk) in file.chunks.iter().enumerate() {
        if chunk.index != index as u64
            || chunk.offset != offset
            || chunk.size == 0
            || chunk.size > CHUNK_SIZE as u64
            || chunk.sha256.len() != 64
            || !chunk.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(err(400, "invalid chunk manifest"));
        }
        offset = offset
            .checked_add(chunk.size)
            .ok_or_else(|| err(400, "file size is too large"))?;
    }
    if offset != file.size_bytes || (file.size_bytes > 0 && file.chunks.is_empty()) {
        return Err(err(400, "chunk sizes do not match the file size"));
    }
    Ok(path)
}

fn cleanup_orphan_transfers(root: &Path) {
    let transfer_root = root.join(".ichoi-transfers");
    let Ok(entries) = std::fs::read_dir(transfer_root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let expired = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > SESSION_LIFETIME);
        if expired && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

impl ImportHub {
    pub fn begin(
        &self,
        config: &Config,
        conn: &mut diesel::SqliteConnection,
        owner: String,
        input: BeginImportRequest,
    ) -> Result<BeginImportResult, ServiceError> {
        if input.files.is_empty() || input.files.len() > MAX_FILES {
            return Err(err(400, "invalid transfer file count"));
        }
        let track_file_index = usize::try_from(input.track_file_index)
            .ok()
            .filter(|index| *index < input.files.len())
            .ok_or_else(|| err(400, "invalid track file index"))?;
        let library_kind = input.library.unwrap_or(Library::Music);
        let (library_id, root) = match library_kind {
            Library::Music => ("lib:music", config.music_dir.as_ref()),
            Library::Audiobook => ("lib:audiobook", config.audiobook_dir.as_ref()),
        };
        let root = root
            .cloned()
            .ok_or_else(|| err(503, "destination library is not configured"))?;
        let requested = input
            .files
            .iter()
            .map(validate_manifest)
            .collect::<Result<Vec<_>, _>>()?;
        let unique = requested.iter().collect::<HashSet<_>>();
        if unique.len() != requested.len() {
            return Err(err(400, "duplicate transfer path"));
        }

        let primary_manifest = &input.files[track_file_index];
        let existing_track = store::track_by_library_content_hash(
            conn,
            library_id,
            &primary_manifest.sha256.to_ascii_lowercase(),
        )
        .map_err(internal)?;
        let requested_primary_parent = requested[track_file_index]
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let existing_parent = existing_track
            .as_ref()
            .and_then(|track| Path::new(&track.root_relative_path).parent())
            .map(Path::to_path_buf);

        cleanup_orphan_transfers(&root);
        let transfer_id = uuid::Uuid::now_v7().to_string();
        let temporary_dir = root.join(".ichoi-transfers").join(&transfer_id);
        std::fs::create_dir_all(&temporary_dir).map_err(internal)?;
        let mut files = Vec::with_capacity(input.files.len());
        let mut missing_chunks = Vec::new();
        for (index, (manifest, requested_path)) in
            input.files.into_iter().zip(requested).enumerate()
        {
            let destination_relative = if let Some(parent) = &existing_parent {
                if index == track_file_index {
                    PathBuf::from(existing_track.as_ref().unwrap().root_relative_path.clone())
                } else {
                    let suffix = requested_path
                        .strip_prefix(&requested_primary_parent)
                        .map_err(|_| err(400, "sidecar is outside the track folder"))?;
                    parent.join(suffix)
                }
            } else {
                requested_path
            };
            let destination = destination_path(&root, &destination_relative)?;
            let exists = if destination.is_file() {
                let matches = sha256_file(&destination)? == manifest.sha256.to_ascii_lowercase();
                if !matches {
                    let _ = std::fs::remove_dir_all(&temporary_dir);
                    return Err(err(409, "a different file already uses a transfer path"));
                }
                true
            } else {
                false
            };
            if !exists {
                if manifest.chunks.is_empty() {
                    File::create(temporary_dir.join(format!("{index}.part"))).map_err(internal)?;
                }
                missing_chunks.extend(manifest.chunks.iter().map(|chunk| MissingChunk {
                    file_index: index as u64,
                    chunk_index: chunk.index,
                }));
            }
            files.push(ImportFileState {
                manifest,
                destination,
                temporary: temporary_dir.join(format!("{index}.part")),
                received: HashSet::new(),
                exists,
            });
        }
        let primary_new = !files[track_file_index].exists;
        let transfer = ImportTransfer {
            owner,
            created: Instant::now(),
            library_id: library_id.to_string(),
            library_kind,
            root,
            track_file_index,
            primary_new,
            temporary_dir,
            files,
        };
        let mut transfers = self.inner.lock().map_err(internal)?;
        let stale = transfers
            .extract_if(|_, transfer| transfer.created.elapsed() > SESSION_LIFETIME)
            .map(|(_, transfer)| transfer.temporary_dir)
            .collect::<Vec<_>>();
        transfers.insert(transfer_id.clone(), transfer);
        drop(transfers);
        for path in stale {
            let _ = std::fs::remove_dir_all(path);
        }
        Ok(BeginImportResult {
            transfer_id,
            missing_chunks,
        })
    }

    pub fn chunk(&self, owner: &str, input: ImportChunkRequest) -> Result<Ok, ServiceError> {
        let mut transfers = self.inner.lock().map_err(internal)?;
        let transfer = transfers
            .get_mut(&input.transfer_id)
            .ok_or_else(|| err(404, "transfer not found"))?;
        if transfer.owner != owner {
            return Err(err(403, "transfer belongs to another session"));
        }
        let file = transfer
            .files
            .get_mut(input.file_index as usize)
            .ok_or_else(|| err(400, "invalid file index"))?;
        if file.exists {
            return Err(err(409, "file is already complete"));
        }
        let chunk = file
            .manifest
            .chunks
            .get(input.chunk_index as usize)
            .filter(|chunk| chunk.index == input.chunk_index)
            .ok_or_else(|| err(400, "invalid chunk index"))?;
        if input.data.len() as u64 != chunk.size
            || sha256_bytes(&input.data) != chunk.sha256.to_ascii_lowercase()
        {
            return Err(err(400, "chunk hash does not match import data"));
        }
        let mut output = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&file.temporary)
            .map_err(internal)?;
        output
            .seek(SeekFrom::Start(chunk.offset))
            .map_err(internal)?;
        output.write_all(&input.data).map_err(internal)?;
        file.received.insert(chunk.index);
        Ok(Ok { ok: true })
    }

    pub fn finish(
        &self,
        config: &Config,
        conn: &mut diesel::SqliteConnection,
        owner: &str,
        input: FinishImportRequest,
    ) -> Result<ImportResult, ServiceError> {
        let mut transfers = self.inner.lock().map_err(internal)?;
        let complete = transfers
            .get(&input.transfer_id)
            .ok_or_else(|| err(404, "transfer not found"))?;
        if complete.owner != owner {
            return Err(err(403, "transfer belongs to another session"));
        }
        if complete
            .files
            .iter()
            .any(|file| !file.exists && file.received.len() != file.manifest.chunks.len())
        {
            return Err(err(409, "transfer is incomplete"));
        }
        let transfer = transfers.remove(&input.transfer_id).unwrap();
        drop(transfers);

        for file in &transfer.files {
            if !file.exists
                && sha256_file(&file.temporary)? != file.manifest.sha256.to_ascii_lowercase()
            {
                let _ = std::fs::remove_dir_all(&transfer.temporary_dir);
                return Err(err(400, "completed file hash does not match manifest"));
            }
        }
        let mut linked = Vec::new();
        for file in &transfer.files {
            if file.exists {
                continue;
            }
            if let Some(parent) = file.destination.parent() {
                if let Err(error) = std::fs::create_dir_all(parent) {
                    for path in &linked {
                        let _ = std::fs::remove_file(path);
                    }
                    let _ = std::fs::remove_dir_all(&transfer.temporary_dir);
                    return Err(internal(error));
                }
            }
            if let Err(error) = std::fs::hard_link(&file.temporary, &file.destination) {
                for path in &linked {
                    let _ = std::fs::remove_file(path);
                }
                let _ = std::fs::remove_dir_all(&transfer.temporary_dir);
                return Err(internal(error));
            }
            linked.push(file.destination.clone());
        }
        let primary = &transfer.files[transfer.track_file_index];
        let indexed = (|| -> Result<models::Track, ServiceError> {
            store::upsert_library(
                conn,
                &models::Library {
                    id: transfer.library_id.clone(),
                    kind: match transfer.library_kind {
                        Library::Music => "music".to_string(),
                        Library::Audiobook => "audiobook".to_string(),
                    },
                    path: transfer.root.to_string_lossy().into_owned(),
                },
            )
            .map_err(internal)?;
            let indexed = crate::scan::index_imported_file(
                conn,
                &transfer.library_id,
                &transfer.root,
                &primary.destination,
                config.album_subfolder_flat,
                &config.album_subfolder_words,
            )
            .map_err(internal)?;
            store::set_track_content_hash(conn, &indexed.id, &primary.manifest.sha256)
                .map_err(internal)?;
            Ok(indexed)
        })();
        let indexed = match indexed {
            Ok(indexed) => indexed,
            Err(error) => {
                for path in &linked {
                    let _ = std::fs::remove_file(path);
                }
                let _ = std::fs::remove_dir_all(&transfer.temporary_dir);
                return Err(error);
            }
        };
        let mut indexed = indexed;
        indexed.content_hash = Some(primary.manifest.sha256.clone());
        let track = crate::handlers::track_from_model(&indexed);
        let _ = std::fs::remove_dir_all(&transfer.temporary_dir);
        Ok(ImportResult {
            imported: transfer.primary_new,
            track_id: Some(track.id.clone()),
            track: Some(track),
            skipped_existing: !transfer.primary_new,
        })
    }

    pub fn cancel(&self, owner: &str, input: CancelImportRequest) -> Result<Ok, ServiceError> {
        let mut transfers = self.inner.lock().map_err(internal)?;
        let transfer = transfers
            .get(&input.transfer_id)
            .ok_or_else(|| err(404, "transfer not found"))?;
        if transfer.owner != owner {
            return Err(err(403, "transfer belongs to another session"));
        }
        let transfer = transfers.remove(&input.transfer_id).unwrap();
        drop(transfers);
        let _ = std::fs::remove_dir_all(transfer.temporary_dir);
        Ok(Ok { ok: true })
    }
}
