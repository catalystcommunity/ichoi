//! Generated service traits from CSIL specification

use super::codec::*;
use super::types::*;

/// SessionService service trait
pub trait SessionService {
    type Context;
    /// authenticate (request/response).
    fn authenticate(
        &self,
        ctx: &Self::Context,
        input: AuthRequest,
    ) -> Result<SessionInfo, ServiceError>;
    /// whoami (request/response).
    fn whoami(&self, ctx: &Self::Context, input: Page) -> Result<SessionInfo, ServiceError>;
    /// logout (request/response).
    fn logout(&self, ctx: &Self::Context, input: Page) -> Result<Ok, ServiceError>;
}

/// LibraryService service trait
pub trait LibraryService {
    type Context;
    /// list-libraries (request/response).
    fn list_libraries(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<LibrariesResponse, ServiceError>;
    /// list-albums (request/response).
    fn list_albums(
        &self,
        ctx: &Self::Context,
        input: BrowseRequest,
    ) -> Result<AlbumsResponse, ServiceError>;
    /// list-artists (request/response).
    fn list_artists(
        &self,
        ctx: &Self::Context,
        input: BrowseRequest,
    ) -> Result<ArtistsResponse, ServiceError>;
    /// get-album (request/response).
    fn get_album(
        &self,
        ctx: &Self::Context,
        input: AlbumRequest,
    ) -> Result<AlbumDetail, ServiceError>;
    /// get-artist (request/response).
    fn get_artist(
        &self,
        ctx: &Self::Context,
        input: ArtistRequest,
    ) -> Result<ArtistDetail, ServiceError>;
    /// search (request/response).
    fn search(
        &self,
        ctx: &Self::Context,
        input: SearchRequest,
    ) -> Result<SearchResponse, ServiceError>;
    /// list-playlists (request/response).
    fn list_playlists(
        &self,
        ctx: &Self::Context,
        input: BrowseRequest,
    ) -> Result<PlaylistsResponse, ServiceError>;
    /// get-playlist (request/response).
    fn get_playlist(
        &self,
        ctx: &Self::Context,
        input: PlaylistRequest,
    ) -> Result<PlaylistDetail, ServiceError>;
    /// get-cover-art (request/response).
    fn get_cover_art(
        &self,
        ctx: &Self::Context,
        input: CoverArtRequest,
    ) -> Result<CoverArt, ServiceError>;
    /// get-audiobook-progress (request/response).
    fn get_audiobook_progress(
        &self,
        ctx: &Self::Context,
        input: AudiobookProgressRequest,
    ) -> Result<AudiobookProgressResponse, ServiceError>;
    /// update-audiobook-progress (request/response).
    fn update_audiobook_progress(
        &self,
        ctx: &Self::Context,
        input: UpdateAudiobookProgressRequest,
    ) -> Result<AudiobookProgress, ServiceError>;
}

/// PlayerService service trait
pub trait PlayerService {
    type Context;
    /// list-players (request/response).
    fn list_players(
        &self,
        ctx: &Self::Context,
        input: ListPlayersRequest,
    ) -> Result<ListPlayersResponse, ServiceError>;
    /// subscribe (channel inbound (bidirectional)).
    fn subscribe(&self, ctx: &Self::Context, msg: SubscribeRequest) -> Result<(), ServiceError>;
    /// control (request/response).
    fn control(
        &self,
        ctx: &Self::Context,
        input: CommandRequest,
    ) -> Result<PlayerState, ServiceError>;
    /// enable-share (request/response).
    fn enable_share(
        &self,
        ctx: &Self::Context,
        input: EnableShareRequest,
    ) -> Result<ShareResult, ServiceError>;
    /// disable-share (request/response).
    fn disable_share(
        &self,
        ctx: &Self::Context,
        input: DisableShareRequest,
    ) -> Result<Ok, ServiceError>;
}

/// Decode one inbound channel frame for PlayerService (with the generated
/// per-type codec) and dispatch to the matching trait method. The implementer
/// feeds raw bytes from its connection here; we never own the wire.
pub fn route_player_service_channel<H>(
    handlers: &H,
    ctx: &H::Context,
    method: &str,
    bytes: &[u8],
) -> Result<(), ServiceError>
where
    H: PlayerService,
{
    match method {
        "subscribe" => {
            let msg = decode_subscribe_request(bytes).map_err(|err| ServiceError {
                code: 400,
                message: err.to_string(),
            })?;
            handlers.subscribe(ctx, msg)
        }
        other => Err(ServiceError {
            code: 404,
            message: format!("unknown channel {other}"),
        }),
    }
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
) -> Result<(), ServiceError>
where
    H: MediaService,
{
    Err(ServiceError {
        code: 404,
        message: format!("unknown channel {method}"),
    })
}


/// NodeService service trait
pub trait NodeService {
    type Context;
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
) -> Result<(), ServiceError>
where
    H: NodeService,
{
    match method {
        "session" => {
            let msg = decode_node_report(bytes).map_err(|err| ServiceError {
                code: 400,
                message: err.to_string(),
            })?;
            handlers.session(ctx, msg)
        }
        other => Err(ServiceError {
            code: 404,
            message: format!("unknown channel {other}"),
        }),
    }
}


/// AdminService service trait
pub trait AdminService {
    type Context;
    /// list-accounts (request/response).
    fn list_accounts(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<ListAccountsResponse, ServiceError>;
    /// set-role (request/response).
    fn set_role(&self, ctx: &Self::Context, input: SetRoleRequest)
        -> Result<Account, ServiceError>;
    /// trust-domain (request/response).
    fn trust_domain(
        &self,
        ctx: &Self::Context,
        input: TrustDomainRequest,
    ) -> Result<TrustedDomains, ServiceError>;
    /// list-trusted-domains (request/response).
    fn list_trusted_domains(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<TrustedDomains, ServiceError>;
    /// list-nodes (request/response).
    fn list_nodes(
        &self,
        ctx: &Self::Context,
        input: Page,
    ) -> Result<ListNodesResponse, ServiceError>;
    /// rename-node (request/response).
    fn rename_node(
        &self,
        ctx: &Self::Context,
        input: RenameNodeRequest,
    ) -> Result<NodeInfo, ServiceError>;
    /// rename-device (request/response).
    fn rename_device(
        &self,
        ctx: &Self::Context,
        input: RenameDeviceRequest,
    ) -> Result<DeviceInfo, ServiceError>;
    /// create-node-token (request/response).
    fn create_node_token(
        &self,
        ctx: &Self::Context,
        input: CreateNodeTokenRequest,
    ) -> Result<NodeTokenResult, ServiceError>;
    /// import-track (request/response).
    fn import_track(
        &self,
        ctx: &Self::Context,
        input: ImportTrackRequest,
    ) -> Result<ImportResult, ServiceError>;
    /// get-settings (request/response).
    fn get_settings(&self, ctx: &Self::Context, input: Page) -> Result<Settings, ServiceError>;
    /// set-setting (request/response).
    fn set_setting(
        &self,
        ctx: &Self::Context,
        input: SetSettingRequest,
    ) -> Result<Settings, ServiceError>;
}
