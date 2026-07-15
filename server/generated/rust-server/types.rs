//! Generated types from CSIL specification

#![allow(non_camel_case_types, clippy::large_enum_variant)]

/// Returned by a generated `validate` method when a field violates one of its
/// CSIL constraints. `field` names the offending field; `message` explains.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "validation failed for `{}`: {}",
            self.field, self.message
        )
    }
}

impl std::error::Error for ValidationError {}

pub type AccountId = String;

pub type Handle = String;

pub type TrackId = String;

pub type AlbumId = String;

pub type ArtistId = String;

pub type PlaylistId = String;

pub type NodeId = String;

pub type DeviceId = String;

pub type PlayerId = String;

/// Role variants
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Admin,
    Member,
    Guest,
}

/// PlayerStatus variants
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerStatus {
    Stopped,
    Playing,
    Paused,
}

/// Codec variants
#[derive(Debug, Clone, PartialEq)]
pub enum Codec {
    Mp3,
    Aac,
    Vorbis,
    Flac,
    Alac,
    Opus,
    Wav,
    Wma,
}

/// TranscodeCodec variants
#[derive(Debug, Clone, PartialEq)]
pub enum TranscodeCodec {
    Aac,
    Mp3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamPref {
    pub max_bitrate_kbps: Option<u64>,
    /// default: false
    pub prefer_original: Option<bool>,
    /// default: "aac"
    pub transcode_codec: Option<TranscodeCodec>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// default: 0
    pub offset: Option<u64>,
    /// default: 100
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ok {
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthRequest {
    pub linkkeys_assertion: Option<Vec<u8>>,
    pub bootstrap_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionInfo {
    pub account_id: AccountId,
    pub handle: Handle,
    pub display_name: Option<String>,
    pub role: Role,
    pub token: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub title: String,
    pub artist_id: Option<ArtistId>,
    pub album_id: Option<AlbumId>,
    pub track_no: Option<u64>,
    pub disc_no: Option<u64>,
    pub duration_ms: u64,
    pub codec: Codec,
    pub bitrate_kbps: Option<u64>,
    pub sample_rate: u64,
    pub channels: u64,
    pub bit_depth: Option<u64>,
    pub root_relative_path: String,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Album {
    pub id: AlbumId,
    pub title: String,
    pub artist_id: Option<ArtistId>,
    pub year: Option<u64>,
    /// default: false
    pub has_cover_art: bool,
    pub track_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
    pub album_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub owner: Option<AccountId>,
    pub entry_count: u64,
    pub root_relative_path: String,
}

/// Library variants
#[derive(Debug, Clone, PartialEq)]
pub enum Library {
    Music,
    Audiobook,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrowseRequest {
    /// default: "music"
    pub library: Option<Library>,
    /// default: 0
    pub offset: Option<u64>,
    /// default: 100
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumsResponse {
    pub albums: Vec<Album>,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtistsResponse {
    pub artists: Vec<Artist>,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumRequest {
    pub album_id: AlbumId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumDetail {
    pub album: Album,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtistRequest {
    pub artist_id: ArtistId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtistDetail {
    pub artist: Artist,
    pub albums: Vec<Album>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchRequest {
    /// constraint: size in 1..=256
    pub query: String,
    /// default: 50
    pub limit: Option<u64>,
}

impl SearchRequest {
    /// Validate this value against the constraints declared in the CSIL spec.
    pub fn validate(&self) -> Result<(), ValidationError> {
        {
            let v = &self.query;
            if v.is_empty() || v.len() > 256usize {
                return Err(ValidationError {
                    field: "query".to_string(),
                    message: "length must be in 1..=256".to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResponse {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistsResponse {
    pub playlists: Vec<Playlist>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistRequest {
    pub playlist_id: PlaylistId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistDetail {
    pub playlist: Playlist,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoverArtRequest {
    pub album_id: AlbumId,
    pub max_size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoverArt {
    pub content_type: String,
    pub data: Vec<u8>,
}

/// PlayerKind variants
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerKind {
    Shared,
    Private,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub kind: PlayerKind,
    pub name: String,
    pub node_id: Option<NodeId>,
    pub device_id: Option<DeviceId>,
    pub owner: Option<AccountId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueueItem {
    pub track_id: TrackId,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    pub player_id: PlayerId,
    pub status: PlayerStatus,
    pub current_index: Option<u64>,
    pub position_ms: Option<u64>,
    /// constraint: <= 100
    /// default: 100
    pub volume: u64,
    pub queue: Vec<QueueItem>,
}

impl PlayerState {
    /// Validate this value against the constraints declared in the CSIL spec.
    pub fn validate(&self) -> Result<(), ValidationError> {
        {
            let v = &self.volume;
            if *v > 100 {
                return Err(ValidationError {
                    field: "volume".to_string(),
                    message: "is above maximum".to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListPlayersRequest {
    pub kind: Option<PlayerKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListPlayersResponse {
    pub players: Vec<Player>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubscribeRequest {
    pub player_id: PlayerId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdEnqueue {
    pub op: String,
    pub track_ids: Vec<TrackId>,
    pub at_index: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdRemove {
    pub op: String,
    pub index: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdReorder {
    pub op: String,
    pub from_index: u64,
    pub to_index: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdClear {
    pub op: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdPlay {
    pub op: String,
    pub index: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdPause {
    pub op: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdNext {
    pub op: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdPrevious {
    pub op: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdSeek {
    pub op: String,
    pub position_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdVolume {
    pub op: String,
    /// constraint: <= 100
    pub volume: u64,
}

impl CmdVolume {
    /// Validate this value against the constraints declared in the CSIL spec.
    pub fn validate(&self) -> Result<(), ValidationError> {
        {
            let v = &self.volume;
            if *v > 100 {
                return Err(ValidationError {
                    field: "volume".to_string(),
                    message: "is above maximum".to_string(),
                });
            }
        }
        Ok(())
    }
}

/// PlayerCommand variants
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCommand {
    Variant0(CmdEnqueue),
    Variant1(CmdRemove),
    Variant2(CmdReorder),
    Variant3(CmdClear),
    Variant4(CmdPlay),
    Variant5(CmdPause),
    Variant6(CmdNext),
    Variant7(CmdPrevious),
    Variant8(CmdSeek),
    Variant9(CmdVolume),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandRequest {
    pub player_id: PlayerId,
    pub command: PlayerCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnableShareRequest {
    /// constraint: size in 1..=48
    /// default: "Device"
    pub suffix: Option<String>,
}

impl EnableShareRequest {
    /// Validate this value against the constraints declared in the CSIL spec.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(v) = &self.suffix {
            if v.is_empty() || v.len() > 48usize {
                return Err(ValidationError {
                    field: "suffix".to_string(),
                    message: "length must be in 1..=48".to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisableShareRequest {
    pub player_id: PlayerId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShareResult {
    pub player: Player,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaOpen {
    pub kind: String,
    pub track_id: TrackId,
    pub pref: StreamPref,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaSeek {
    pub kind: String,
    pub position_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaPause {
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaResume {
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaStop {
    pub kind: String,
}

/// MediaControl variants
#[derive(Debug, Clone, PartialEq)]
pub enum MediaControl {
    Variant0(MediaOpen),
    Variant1(MediaSeek),
    Variant2(MediaPause),
    Variant3(MediaResume),
    Variant4(MediaStop),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaHeader {
    pub kind: String,
    pub codec: Codec,
    pub transcoded: bool,
    pub sample_rate: u64,
    pub channels: u64,
    pub duration_ms: Option<u64>,
    /// default: 0
    pub trim_start_samples: u64,
    /// default: 0
    pub trim_end_samples: u64,
    pub codec_config: Option<Vec<u8>>,
}

/// MediaEndReason variants
#[derive(Debug, Clone, PartialEq)]
pub enum MediaEndReason {
    Eos,
    Stopped,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaChunk {
    pub kind: String,
    pub seq: u64,
    pub timestamp_ms: Option<u64>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaEnd {
    pub kind: String,
    pub reason: Option<MediaEndReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaFail {
    pub kind: String,
    pub error: ServiceError,
}

/// MediaEvent variants
#[derive(Debug, Clone, PartialEq)]
pub enum MediaEvent {
    Variant0(MediaHeader),
    Variant1(MediaChunk),
    Variant2(MediaEnd),
    Variant3(MediaFail),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioOutput {
    pub os_device_id: DeviceId,
    pub friendly_name: Option<String>,
    pub channels: u64,
    pub sample_rates: Vec<u64>,
    /// default: false
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegisterNodeRequest {
    pub hostname: String,
    pub platform: String,
    pub arch: String,
    pub outputs: Vec<AudioOutput>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegisterNodeResponse {
    pub node_id: NodeId,
    pub players: Vec<Player>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirLoad {
    pub op: String,
    pub player_id: PlayerId,
    pub track_id: TrackId,
    pub pref: StreamPref,
    pub position_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirPause {
    pub op: String,
    pub player_id: PlayerId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirResume {
    pub op: String,
    pub player_id: PlayerId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirStop {
    pub op: String,
    pub player_id: PlayerId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirVolume {
    pub op: String,
    pub player_id: PlayerId,
    /// constraint: <= 100
    pub volume: u64,
}

impl DirVolume {
    /// Validate this value against the constraints declared in the CSIL spec.
    pub fn validate(&self) -> Result<(), ValidationError> {
        {
            let v = &self.volume;
            if *v > 100 {
                return Err(ValidationError {
                    field: "volume".to_string(),
                    message: "is above maximum".to_string(),
                });
            }
        }
        Ok(())
    }
}

/// NodeDirective variants
#[derive(Debug, Clone, PartialEq)]
pub enum NodeDirective {
    Variant0(DirLoad),
    Variant1(DirPause),
    Variant2(DirResume),
    Variant3(DirStop),
    Variant4(DirVolume),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeReport {
    pub player_id: PlayerId,
    pub status: PlayerStatus,
    pub position_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub id: AccountId,
    pub handle: Handle,
    pub display_name: Option<String>,
    pub role: Role,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListAccountsResponse {
    pub accounts: Vec<Account>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetRoleRequest {
    pub account_id: AccountId,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrustDomainRequest {
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrustedDomains {
    pub domains: Vec<String>,
}

/// NodeKind variants
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Core,
    Satellite,
    Client,
}

/// AudioOutputsState variants
#[derive(Debug, Clone, PartialEq)]
pub enum AudioOutputsState {
    None,
    Some,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub os_device_id: String,
    pub friendly_name: String,
    /// default: false
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeInfo {
    pub id: NodeId,
    pub kind: NodeKind,
    pub hostname: String,
    pub friendly_name: String,
    pub platform: String,
    pub arch: String,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
    pub audio_outputs: AudioOutputsState,
    pub devices: Vec<DeviceInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListNodesResponse {
    pub nodes: Vec<NodeInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenameNodeRequest {
    pub node_id: NodeId,
    /// constraint: size in 1..=64
    pub friendly_name: String,
}

impl RenameNodeRequest {
    /// Validate this value against the constraints declared in the CSIL spec.
    pub fn validate(&self) -> Result<(), ValidationError> {
        {
            let v = &self.friendly_name;
            if v.is_empty() || v.len() > 64usize {
                return Err(ValidationError {
                    field: "friendly_name".to_string(),
                    message: "length must be in 1..=64".to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenameDeviceRequest {
    pub device_id: DeviceId,
    /// constraint: size in 1..=64
    pub friendly_name: String,
}

impl RenameDeviceRequest {
    /// Validate this value against the constraints declared in the CSIL spec.
    pub fn validate(&self) -> Result<(), ValidationError> {
        {
            let v = &self.friendly_name;
            if v.is_empty() || v.len() > 64usize {
                return Err(ValidationError {
                    field: "friendly_name".to_string(),
                    message: "length must be in 1..=64".to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateNodeTokenRequest {
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeTokenResult {
    pub token: String,
    pub fingerprints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportTrackRequest {
    pub root_relative_path: String,
    pub content_type: String,
    pub content_hash: Option<String>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportResult {
    pub imported: bool,
    pub track_id: Option<TrackId>,
    /// default: false
    pub skipped_existing: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub entries: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetSettingRequest {
    pub key: String,
    pub value: String,
}
