//! Diesel row models. Numeric columns follow the schema: counts/flags are `Integer` (i32),
//! durations/sizes/sample counts are `BigInt` (i64).

use diesel::prelude::*;

use super::schema::*;

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Account {
    pub id: String,
    pub handle: String,
    pub display_name: Option<String>,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = sessions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Session {
    pub token_sha256: String,
    pub account_id: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = settings, primary_key(key))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Setting {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = libraries)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Library {
    pub id: String,
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = artists)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = albums)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist_id: Option<String>,
    pub year: Option<i32>,
    pub has_cover_art: i32,
    pub cover_art_path: Option<String>,
    pub art_checked: i32,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = tracks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Track {
    pub id: String,
    pub library_id: String,
    pub root_relative_path: String,
    pub title: String,
    pub artist_id: Option<String>,
    pub album_id: Option<String>,
    pub track_no: Option<i32>,
    pub disc_no: Option<i32>,
    pub duration_ms: i64,
    pub codec: String,
    pub bitrate_kbps: Option<i32>,
    pub sample_rate: i32,
    pub channels: i32,
    pub bit_depth: Option<i32>,
    pub size_bytes: i64,
    pub mtime: String,
    pub content_hash: Option<String>,
    pub trim_start_samples: i64,
    pub trim_end_samples: i64,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = playlists)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub root_relative_path: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = nodes)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub hostname: String,
    pub friendly_name: String,
    pub token_sha256: Option<String>,
    pub platform: String,
    pub arch: String,
    pub audio_outputs: String,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = output_devices)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct OutputDevice {
    pub id: String,
    pub node_id: String,
    pub os_device_id: String,
    pub friendly_name: String,
    pub is_default: i32,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = players)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Player {
    pub id: String,
    pub kind: String,
    pub output_device_id: Option<String>,
    pub owner_account_id: Option<String>,
    pub name: String,
    pub name_suffix: Option<String>,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable, AsChangeset)]
#[diesel(table_name = player_state, primary_key(player_id))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlayerStateRow {
    pub player_id: String,
    pub status: String,
    pub current_index: Option<i32>,
    pub position_ms: Option<i64>,
    pub volume: i32,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = player_queue_items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct QueueItem {
    pub id: i32,
    pub player_id: String,
    pub track_id: String,
    pub position: i32,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = player_queue_items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewQueueItem {
    pub player_id: String,
    pub track_id: String,
    pub position: i32,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = trusted_domains)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TrustedDomain {
    pub domain: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable)]
#[diesel(table_name = linkkeys_local_rp_identities, primary_key(fingerprint))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LinkkeysLocalRpIdentity {
    pub fingerprint: String,
    pub name: String,
    pub identity_bundle: Vec<u8>,
    pub active: i32,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Identifiable)]
#[diesel(table_name = linkkeys_trusted_identities, primary_key(domain, handle))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LinkkeysTrustedIdentity {
    pub domain: String,
    pub handle: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = linkkeys_login_attempts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LinkkeysLoginAttempt {
    pub attempt_sha256: String,
    pub pending_login: String,
    pub expected_handle: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = linkkeys_login_exchanges)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct LinkkeysLoginExchange {
    pub code_sha256: String,
    pub account_id: String,
    pub created_at: String,
    pub expires_at: String,
}
