//! Generated service traits from CSIL specification

use super::codec::*;
use super::types::*;

#[derive(Debug)]
pub enum ChannelRouteError {
    UnknownMethod(String),
    UnknownOrdinal(u64),
    Decode(String),
    Handler(ServiceError),
}

impl std::fmt::Display for ChannelRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMethod(method) => write!(f, "unknown channel {method}"),
            Self::UnknownOrdinal(op) => write!(f, "unknown channel ordinal {op}"),
            Self::Decode(message) => write!(f, "invalid channel payload: {message}"),
            Self::Handler(error) => write!(f, "channel handler failed: {error:?}"),
        }
    }
}

impl std::error::Error for ChannelRouteError {}

impl From<CsilCborError> for ChannelRouteError {
    fn from(error: CsilCborError) -> Self {
        Self::Decode(error.0)
    }
}

/// SessionService service trait
pub trait SessionService {
    type Context;
    #[allow(clippy::result_large_err)]
    /// authenticate (request/response).
    fn authenticate(
        &self,
        ctx: &Self::Context,
        input: AuthRequest,
    ) -> Result<SessionInfo, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// whoami (request/response).
    fn whoami(&self, ctx: &Self::Context, input: Page) -> Result<SessionInfo, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// logout (request/response).
    fn logout(&self, ctx: &Self::Context, input: Page) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// delete-account (request/response).
    fn delete_account(
        &self,
        ctx: &Self::Context,
        input: DeleteAccountRequest,
    ) -> Result<Ok, ServiceError>;
}

/// LibraryService service trait
pub trait LibraryService {
    type Context;
    #[allow(clippy::result_large_err)]
    /// list-libraries (request/response).
    fn list_libraries(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<LibrariesResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-albums (request/response).
    fn list_albums(
        &self,
        ctx: &Self::Context,
        input: BrowseRequest,
    ) -> Result<AlbumsResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-artists (request/response).
    fn list_artists(
        &self,
        ctx: &Self::Context,
        input: BrowseRequest,
    ) -> Result<ArtistsResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-album (request/response).
    fn get_album(
        &self,
        ctx: &Self::Context,
        input: AlbumRequest,
    ) -> Result<AlbumDetail, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-artist (request/response).
    fn get_artist(
        &self,
        ctx: &Self::Context,
        input: ArtistRequest,
    ) -> Result<ArtistDetail, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// search (request/response).
    fn search(
        &self,
        ctx: &Self::Context,
        input: SearchRequest,
    ) -> Result<SearchResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// export-manifest (request/response).
    fn export_manifest(
        &self,
        ctx: &Self::Context,
        input: ExportManifestRequest,
    ) -> Result<ExportManifest, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// export-chunk (request/response).
    fn export_chunk(
        &self,
        ctx: &Self::Context,
        input: ExportChunkRequest,
    ) -> Result<ExportChunk, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-playlists (request/response).
    fn list_playlists(
        &self,
        ctx: &Self::Context,
        input: BrowseRequest,
    ) -> Result<PlaylistsResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-playlist (request/response).
    fn get_playlist(
        &self,
        ctx: &Self::Context,
        input: PlaylistRequest,
    ) -> Result<PlaylistDetail, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// delete-playlist (request/response).
    fn delete_playlist(
        &self,
        ctx: &Self::Context,
        input: DeletePlaylistRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// block-account (request/response).
    fn block_account(
        &self,
        ctx: &Self::Context,
        input: BlockAccountRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// unblock-account (request/response).
    fn unblock_account(
        &self,
        ctx: &Self::Context,
        input: BlockAccountRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-blocked-accounts (request/response).
    fn list_blocked_accounts(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<BlockedAccountsResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-cover-art (request/response).
    fn get_cover_art(
        &self,
        ctx: &Self::Context,
        input: CoverArtRequest,
    ) -> Result<CoverArt, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-artwork (request/response).
    fn get_artwork(
        &self,
        ctx: &Self::Context,
        input: ArtworkRequest,
    ) -> Result<CoverArt, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-artwork-candidates (request/response).
    fn list_artwork_candidates(
        &self,
        ctx: &Self::Context,
        input: ArtworkCandidatesRequest,
    ) -> Result<ArtworkCandidates, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-artwork-candidate (request/response).
    fn get_artwork_candidate(
        &self,
        ctx: &Self::Context,
        input: ArtworkCandidateRequest,
    ) -> Result<CoverArt, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// set-artwork (request/response).
    fn set_artwork(
        &self,
        ctx: &Self::Context,
        input: SetArtworkRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-audiobook-progress (request/response).
    fn get_audiobook_progress(
        &self,
        ctx: &Self::Context,
        input: AudiobookProgressRequest,
    ) -> Result<AudiobookProgressResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// update-audiobook-progress (request/response).
    fn update_audiobook_progress(
        &self,
        ctx: &Self::Context,
        input: UpdateAudiobookProgressRequest,
    ) -> Result<AudiobookProgress, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// report-content (request/response).
    fn report_content(
        &self,
        ctx: &Self::Context,
        input: ReportContentRequest,
    ) -> Result<ContentReport, ServiceError>;
}

/// PlayerService service trait
pub trait PlayerService {
    type Context;
    #[allow(clippy::result_large_err)]
    /// list-players (request/response).
    fn list_players(
        &self,
        ctx: &Self::Context,
        input: ListPlayersRequest,
    ) -> Result<ListPlayersResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-state (request/response).
    fn get_state(
        &self,
        ctx: &Self::Context,
        input: SubscribeRequest,
    ) -> Result<PlayerState, ServiceError>;
    /// subscribe (channel inbound (bidirectional)).
    fn subscribe(&self, ctx: &Self::Context, msg: SubscribeRequest) -> Result<(), ServiceError>;
    #[allow(clippy::result_large_err)]
    /// control (request/response).
    fn control(
        &self,
        ctx: &Self::Context,
        input: CommandRequest,
    ) -> Result<PlayerState, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// enable-share (request/response).
    fn enable_share(
        &self,
        ctx: &Self::Context,
        input: EnableShareRequest,
    ) -> Result<ShareResult, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// disable-share (request/response).
    fn disable_share(
        &self,
        ctx: &Self::Context,
        input: DisableShareRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-share-access (request/response).
    fn get_share_access(
        &self,
        ctx: &Self::Context,
        input: ShareAccessRequest,
    ) -> Result<ShareAccessInfo, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// set-share-access (request/response).
    fn set_share_access(
        &self,
        ctx: &Self::Context,
        input: SetShareAccessRequest,
    ) -> Result<ShareAccessInfo, ServiceError>;
}

/// Decode one inbound channel frame for PlayerService (with the generated
/// per-type codec) and dispatch to the matching trait method. The implementer
/// feeds raw bytes from its connection here; we never own the wire.
pub fn route_player_service_channel<H>(
    handlers: &H,
    ctx: &H::Context,
    method: &str,
    bytes: &[u8],
) -> Result<(), ChannelRouteError>
where
    H: PlayerService,
{
    match method {
        "subscribe" => {
            let msg = decode_subscribe_request(bytes)?;
            handlers.subscribe(ctx, msg)
        }
        _ => return Err(ChannelRouteError::UnknownMethod(method.to_owned())),
    }
    .map_err(ChannelRouteError::Handler)
}

/// Encode a `subscribe` message pushed from PlayerService's server
/// side; the implementer frames `(method, bytes)` onto its connection.
pub fn encode_player_service_subscribe(msg: &PlayerState) -> (String, Vec<u8>) {
    ("subscribe".to_string(), encode_player_state(msg))
}

/// MediaService service trait
pub trait MediaService {
    type Context;
    /// stream (channel inbound (bidirectional)).
    fn stream(&self, ctx: &Self::Context, msg: MediaControl) -> Result<(), ServiceError>;
}

/// Decode one inbound channel frame for MediaService (with the generated
/// per-type codec) and dispatch to the matching trait method. The implementer
/// feeds raw bytes from its connection here; we never own the wire.
pub fn route_media_service_channel<H>(
    _handlers: &H,
    _ctx: &H::Context,
    method: &str,
    _bytes: &[u8],
) -> Result<(), ChannelRouteError>
where
    H: MediaService,
{
    Err(ChannelRouteError::UnknownMethod(method.to_owned()))
}


/// NodeService service trait
pub trait NodeService {
    type Context;
    #[allow(clippy::result_large_err)]
    /// register (request/response).
    fn register(
        &self,
        ctx: &Self::Context,
        input: RegisterNodeRequest,
    ) -> Result<RegisterNodeResponse, ServiceError>;
    /// session (channel inbound (bidirectional)).
    fn session(&self, ctx: &Self::Context, msg: NodeReport) -> Result<(), ServiceError>;
}

/// Decode one inbound channel frame for NodeService (with the generated
/// per-type codec) and dispatch to the matching trait method. The implementer
/// feeds raw bytes from its connection here; we never own the wire.
pub fn route_node_service_channel<H>(
    handlers: &H,
    ctx: &H::Context,
    method: &str,
    bytes: &[u8],
) -> Result<(), ChannelRouteError>
where
    H: NodeService,
{
    match method {
        "session" => {
            let msg = decode_node_report(bytes)?;
            handlers.session(ctx, msg)
        }
        _ => return Err(ChannelRouteError::UnknownMethod(method.to_owned())),
    }
    .map_err(ChannelRouteError::Handler)
}


/// AdminService service trait
pub trait AdminService {
    type Context;
    #[allow(clippy::result_large_err)]
    /// list-accounts (request/response).
    fn list_accounts(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<ListAccountsResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// set-role (request/response).
    fn set_role(&self, ctx: &Self::Context, input: SetRoleRequest)
        -> Result<Account, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// set-library-editor (request/response).
    fn set_library_editor(
        &self,
        ctx: &Self::Context,
        input: SetLibraryEditorRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// delete-account (request/response).
    fn delete_account(
        &self,
        ctx: &Self::Context,
        input: AdminDeleteAccountRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// trust-domain (request/response).
    fn trust_domain(
        &self,
        ctx: &Self::Context,
        input: TrustDomainRequest,
    ) -> Result<TrustedDomains, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-trusted-domains (request/response).
    fn list_trusted_domains(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<TrustedDomains, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-trusted-identities (request/response).
    fn list_trusted_identities(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<TrustedIdentities, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// trust-identity (request/response).
    fn trust_identity(
        &self,
        ctx: &Self::Context,
        input: TrustIdentityRequest,
    ) -> Result<TrustedIdentities, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// revoke-trusted-identity (request/response).
    fn revoke_trusted_identity(
        &self,
        ctx: &Self::Context,
        input: RevokeTrustedIdentityRequest,
    ) -> Result<TrustedIdentities, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-nodes (request/response).
    fn list_nodes(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<ListNodesResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// rename-node (request/response).
    fn rename_node(
        &self,
        ctx: &Self::Context,
        input: RenameNodeRequest,
    ) -> Result<NodeInfo, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// rename-device (request/response).
    fn rename_device(
        &self,
        ctx: &Self::Context,
        input: RenameDeviceRequest,
    ) -> Result<DeviceInfo, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// set-device-access (request/response).
    fn set_device_access(
        &self,
        ctx: &Self::Context,
        input: SetDeviceAccessRequest,
    ) -> Result<DeviceInfo, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-groups (request/response).
    fn list_groups(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<ListGroupsResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// create-group (request/response).
    fn create_group(
        &self,
        ctx: &Self::Context,
        input: CreateGroupRequest,
    ) -> Result<GroupInfo, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// set-group-members (request/response).
    fn set_group_members(
        &self,
        ctx: &Self::Context,
        input: SetGroupMembersRequest,
    ) -> Result<GroupInfo, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// delete-group (request/response).
    fn delete_group(
        &self,
        ctx: &Self::Context,
        input: DeleteGroupRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-satellite-tokens (request/response).
    fn list_satellite_tokens(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<ListSatelliteTokensResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// create-node-token (request/response).
    fn create_node_token(
        &self,
        ctx: &Self::Context,
        input: CreateNodeTokenRequest,
    ) -> Result<NodeTokenResult, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// revoke-satellite-token (request/response).
    fn revoke_satellite_token(
        &self,
        ctx: &Self::Context,
        input: RevokeSatelliteTokenRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// import-track (request/response).
    fn import_track(
        &self,
        ctx: &Self::Context,
        input: ImportTrackRequest,
    ) -> Result<ImportResult, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// begin-import (request/response).
    fn begin_import(
        &self,
        ctx: &Self::Context,
        input: BeginImportRequest,
    ) -> Result<BeginImportResult, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// import-chunk (request/response).
    fn import_chunk(
        &self,
        ctx: &Self::Context,
        input: ImportChunkRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// finish-import (request/response).
    fn finish_import(
        &self,
        ctx: &Self::Context,
        input: FinishImportRequest,
    ) -> Result<ImportResult, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// cancel-import (request/response).
    fn cancel_import(
        &self,
        ctx: &Self::Context,
        input: CancelImportRequest,
    ) -> Result<Ok, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-settings (request/response).
    fn get_settings(&self, ctx: &Self::Context, input: Page) -> Result<Settings, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// set-setting (request/response).
    fn set_setting(
        &self,
        ctx: &Self::Context,
        input: SetSettingRequest,
    ) -> Result<Settings, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// resync-library (request/response).
    fn resync_library(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<LibraryResyncStatus, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// get-resync-status (request/response).
    fn get_resync_status(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<LibraryResyncStatus, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// list-content-reports (request/response).
    fn list_content_reports(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<ListContentReportsResponse, ServiceError>;
    #[allow(clippy::result_large_err)]
    /// update-content-report-status (request/response).
    fn update_content_report_status(
        &self,
        ctx: &Self::Context,
        input: UpdateContentReportStatusRequest,
    ) -> Result<ContentReport, ServiceError>;
}

/// ChangeService service trait
pub trait ChangeService {
    type Context;
    /// watch (channel inbound (bidirectional)).
    fn watch(&self, ctx: &Self::Context, msg: WatchChangesRequest) -> Result<(), ServiceError>;
}

/// Decode one inbound channel frame for ChangeService (with the generated
/// per-type codec) and dispatch to the matching trait method. The implementer
/// feeds raw bytes from its connection here; we never own the wire.
pub fn route_change_service_channel<H>(
    handlers: &H,
    ctx: &H::Context,
    method: &str,
    bytes: &[u8],
) -> Result<(), ChannelRouteError>
where
    H: ChangeService,
{
    match method {
        "watch" => {
            let msg = decode_watch_changes_request(bytes)?;
            handlers.watch(ctx, msg)
        }
        _ => return Err(ChannelRouteError::UnknownMethod(method.to_owned())),
    }
    .map_err(ChannelRouteError::Handler)
}

/// Encode a `watch` message pushed from ChangeService's server
/// side; the implementer frames `(method, bytes)` onto its connection.
pub fn encode_change_service_watch(msg: &DataChange) -> (String, Vec<u8>) {
    ("watch".to_string(), encode_data_change(msg))
}
