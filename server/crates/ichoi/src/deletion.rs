//! Crash-safe playlist file staging for account and playlist deletion.

use std::path::{Component, Path, PathBuf};

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{models, store};

const STAGING_DIR: &str = ".ichoi-delete-staging";

#[derive(Debug, Error)]
pub enum DeletionError {
    #[error("account not found")]
    AccountNotFound,
    #[error("playlist not found")]
    PlaylistNotFound,
    #[error("the last administrator cannot be deleted")]
    LastAdmin,
    #[error("invalid playlist path: {0}")]
    InvalidPath(String),
    #[error("playlist file operation failed: {0}")]
    File(#[from] std::io::Error),
    #[error("database operation failed: {0}")]
    Database(#[from] diesel::result::Error),
}

#[derive(Debug, Serialize, Deserialize)]
struct StagingMetadata {
    playlist_id: String,
    original_relative: String,
    staged_name: String,
}

#[derive(Debug)]
pub struct StagedPlaylist {
    metadata_path: PathBuf,
    staged_path: PathBuf,
    original_path: PathBuf,
}

fn playlist_path(music_root: &Path, relative: &str) -> Result<PathBuf, DeletionError> {
    let relative = Path::new(relative);
    let mut components = relative.components();
    if components.next() != Some(Component::Normal("playlists".as_ref())) {
        return Err(DeletionError::InvalidPath(relative.display().to_string()));
    }
    let tail = components.collect::<PathBuf>();
    if tail.as_os_str().is_empty()
        || tail
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DeletionError::InvalidPath(relative.display().to_string()));
    }
    let playlist_dir = music_root.join("playlists");
    std::fs::create_dir_all(&playlist_dir)?;
    let canonical_dir = playlist_dir.canonicalize()?;
    let candidate = playlist_dir.join(tail);
    let parent = candidate
        .parent()
        .ok_or_else(|| DeletionError::InvalidPath(relative.display().to_string()))?;
    let canonical_parent = parent.canonicalize()?;
    if !canonical_parent.starts_with(&canonical_dir) {
        return Err(DeletionError::InvalidPath(relative.display().to_string()));
    }
    Ok(candidate)
}

pub fn stage_playlist_file(
    music_root: &Path,
    playlist: &models::Playlist,
) -> Result<Option<StagedPlaylist>, DeletionError> {
    let original_path = playlist_path(music_root, &playlist.root_relative_path)?;
    let metadata = match std::fs::symlink_metadata(&original_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(DeletionError::InvalidPath(
            playlist.root_relative_path.clone(),
        ));
    }

    let staging_dir = music_root.join("playlists").join(STAGING_DIR);
    std::fs::create_dir_all(&staging_dir)?;
    let staged_name = format!("{}.playlist", uuid::Uuid::new_v4());
    let staged_path = staging_dir.join(&staged_name);
    let metadata_path = staging_dir.join(format!("{staged_name}.json"));
    let record = StagingMetadata {
        playlist_id: playlist.id.clone(),
        original_relative: playlist.root_relative_path.clone(),
        staged_name,
    };
    std::fs::write(
        &metadata_path,
        serde_json::to_vec(&record).expect("serializable metadata"),
    )?;
    if let Err(error) = std::fs::rename(&original_path, &staged_path) {
        let _ = std::fs::remove_file(&metadata_path);
        return Err(error.into());
    }
    Ok(Some(StagedPlaylist {
        metadata_path,
        staged_path,
        original_path,
    }))
}

fn restore_staged(items: &[StagedPlaylist]) -> Result<(), DeletionError> {
    for item in items.iter().rev() {
        if item.staged_path.exists() {
            std::fs::rename(&item.staged_path, &item.original_path)?;
        }
        if item.metadata_path.exists() {
            std::fs::remove_file(&item.metadata_path)?;
        }
    }
    Ok(())
}

pub fn finish_staged(items: &[StagedPlaylist]) {
    for item in items {
        if let Err(error) = std::fs::remove_file(&item.staged_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!(
                    "deleting staged playlist {}: {error}",
                    item.staged_path.display()
                );
                continue;
            }
        }
        if let Err(error) = std::fs::remove_file(&item.metadata_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!(
                    "deleting playlist staging metadata {}: {error}",
                    item.metadata_path.display()
                );
            }
        }
    }
}

pub fn delete_account(
    conn: &mut diesel::SqliteConnection,
    music_root: &Path,
    account_id: &str,
) -> Result<(), DeletionError> {
    let account = store::get_account(conn, account_id)?.ok_or(DeletionError::AccountNotFound)?;
    if account.role == "admin" && store::count_admin_accounts(conn)? <= 1 {
        return Err(DeletionError::LastAdmin);
    }
    let private_playlists = store::private_playlists_for_owner(conn, account_id)?;
    let mut staged = Vec::new();
    for playlist in &private_playlists {
        match stage_playlist_file(music_root, playlist) {
            Ok(Some(item)) => staged.push(item),
            Ok(None) => {}
            Err(error) => {
                restore_staged(&staged)?;
                return Err(error);
            }
        }
    }

    let transaction = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        store::delete_content_reports_for_target(conn, "account", account_id)?;
        for playlist in &private_playlists {
            store::delete_content_reports_for_target(conn, "playlist", &playlist.id)?;
            store::delete_playlist_row(conn, &playlist.id)?;
        }
        store::clear_public_playlist_owner(conn, account_id)?;
        store::delete_account_row(conn, account_id)?;
        Ok(())
    });
    if let Err(error) = transaction {
        restore_staged(&staged)?;
        return Err(error.into());
    }
    finish_staged(&staged);
    Ok(())
}

pub fn delete_playlist(
    conn: &mut diesel::SqliteConnection,
    music_root: &Path,
    playlist_id: &str,
) -> Result<(), DeletionError> {
    let playlist =
        store::get_playlist(conn, playlist_id)?.ok_or(DeletionError::PlaylistNotFound)?;
    let staged = stage_playlist_file(music_root, &playlist)?
        .into_iter()
        .collect::<Vec<_>>();
    let transaction = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        store::delete_content_reports_for_target(conn, "playlist", playlist_id)?;
        store::delete_playlist_row(conn, playlist_id)?;
        Ok(())
    });
    if let Err(error) = transaction {
        restore_staged(&staged)?;
        return Err(error.into());
    }
    finish_staged(&staged);
    Ok(())
}

pub fn reconcile_playlist_staging(
    conn: &mut diesel::SqliteConnection,
    music_root: &Path,
) -> Result<(), DeletionError> {
    let staging_dir = music_root.join("playlists").join(STAGING_DIR);
    let entries = match std::fs::read_dir(&staging_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let metadata_path = entry?.path();
        if metadata_path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let record: StagingMetadata = serde_json::from_slice(&std::fs::read(&metadata_path)?)
            .map_err(|error| {
                DeletionError::InvalidPath(format!(
                    "invalid staging metadata {}: {error}",
                    metadata_path.display()
                ))
            })?;
        if Path::new(&record.staged_name)
            .file_name()
            .and_then(|v| v.to_str())
            != Some(record.staged_name.as_str())
        {
            return Err(DeletionError::InvalidPath(record.staged_name));
        }
        let staged_path = staging_dir.join(&record.staged_name);
        let original_path = playlist_path(music_root, &record.original_relative)?;
        if store::get_playlist(conn, &record.playlist_id)?.is_some() {
            if staged_path.exists() && !original_path.exists() {
                std::fs::rename(&staged_path, &original_path)?;
            } else if staged_path.exists() {
                std::fs::remove_file(&staged_path)?;
            }
        } else if staged_path.exists() {
            std::fs::remove_file(&staged_path)?;
        }
        std::fs::remove_file(metadata_path)?;
    }
    Ok(())
}
