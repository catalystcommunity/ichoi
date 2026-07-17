//! Generated transport-agnostic service clients from CSIL specification

#![allow(async_fn_in_trait)]

use super::client::ClientError;
use super::codec::*;
use super::types::*;

/// The caller-supplied byte carrier: it performs the call named by `(service, op)`
/// with the already-encoded request bytes and returns the response bytes, or an
/// error. The generated client owns (de)serialization via the codec; the carrier
/// only moves bytes, so it can be HTTP, a queue, or an in-process loop.
pub trait AsyncTransport {
    async fn call(&self, service: &str, op: &str, req: &[u8]) -> Result<Vec<u8>, ClientError>;
}

/// Typed client for the SessionService service.
pub struct SessionAsyncClient<T: AsyncTransport> {
    #[allow(dead_code)]
    transport: T,
}

impl<T: AsyncTransport> SessionAsyncClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// authenticate (request/response).
    pub async fn authenticate(&self, req: AuthRequest) -> Result<SessionInfo, ClientError> {
        let csil_resp = self
            .transport
            .call("SessionService", "authenticate", &encode_auth_request(&req))
            .await?;
        decode_session_info(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// whoami (request/response).
    pub async fn whoami(&self, req: Page) -> Result<SessionInfo, ClientError> {
        let csil_resp = self
            .transport
            .call("SessionService", "whoami", &encode_page(&req))
            .await?;
        decode_session_info(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// logout (request/response).
    pub async fn logout(&self, req: Page) -> Result<Ok, ClientError> {
        let csil_resp = self
            .transport
            .call("SessionService", "logout", &encode_page(&req))
            .await?;
        decode_ok(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }
}

/// Typed client for the LibraryService service.
pub struct LibraryAsyncClient<T: AsyncTransport> {
    #[allow(dead_code)]
    transport: T,
}

impl<T: AsyncTransport> LibraryAsyncClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// list-albums (request/response).
    pub async fn list_albums(&self, req: BrowseRequest) -> Result<AlbumsResponse, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "LibraryService",
                "list-albums",
                &encode_browse_request(&req),
            )
            .await?;
        decode_albums_response(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// list-artists (request/response).
    pub async fn list_artists(&self, req: BrowseRequest) -> Result<ArtistsResponse, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "LibraryService",
                "list-artists",
                &encode_browse_request(&req),
            )
            .await?;
        decode_artists_response(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// get-album (request/response).
    pub async fn get_album(&self, req: AlbumRequest) -> Result<AlbumDetail, ClientError> {
        let csil_resp = self
            .transport
            .call("LibraryService", "get-album", &encode_album_request(&req))
            .await?;
        decode_album_detail(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// get-artist (request/response).
    pub async fn get_artist(&self, req: ArtistRequest) -> Result<ArtistDetail, ClientError> {
        let csil_resp = self
            .transport
            .call("LibraryService", "get-artist", &encode_artist_request(&req))
            .await?;
        decode_artist_detail(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// search (request/response).
    pub async fn search(&self, req: SearchRequest) -> Result<SearchResponse, ClientError> {
        let csil_resp = self
            .transport
            .call("LibraryService", "search", &encode_search_request(&req))
            .await?;
        decode_search_response(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// list-playlists (request/response).
    pub async fn list_playlists(
        &self,
        req: BrowseRequest,
    ) -> Result<PlaylistsResponse, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "LibraryService",
                "list-playlists",
                &encode_browse_request(&req),
            )
            .await?;
        decode_playlists_response(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// get-playlist (request/response).
    pub async fn get_playlist(&self, req: PlaylistRequest) -> Result<PlaylistDetail, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "LibraryService",
                "get-playlist",
                &encode_playlist_request(&req),
            )
            .await?;
        decode_playlist_detail(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// get-cover-art (request/response).
    pub async fn get_cover_art(&self, req: CoverArtRequest) -> Result<CoverArt, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "LibraryService",
                "get-cover-art",
                &encode_cover_art_request(&req),
            )
            .await?;
        decode_cover_art(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }
}

/// Typed client for the PlayerService service.
pub struct PlayerAsyncClient<T: AsyncTransport> {
    #[allow(dead_code)]
    transport: T,
}

impl<T: AsyncTransport> PlayerAsyncClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// list-players (request/response).
    pub async fn list_players(
        &self,
        req: ListPlayersRequest,
    ) -> Result<ListPlayersResponse, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "PlayerService",
                "list-players",
                &encode_list_players_request(&req),
            )
            .await?;
        decode_list_players_response(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    // channel operation `subscribe` is not part of the RPC client

    /// control (request/response).
    pub async fn control(&self, req: CommandRequest) -> Result<PlayerState, ClientError> {
        let csil_resp = self
            .transport
            .call("PlayerService", "control", &encode_command_request(&req))
            .await?;
        decode_player_state(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// enable-share (request/response).
    pub async fn enable_share(&self, req: EnableShareRequest) -> Result<ShareResult, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "PlayerService",
                "enable-share",
                &encode_enable_share_request(&req),
            )
            .await?;
        decode_share_result(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// disable-share (request/response).
    pub async fn disable_share(&self, req: DisableShareRequest) -> Result<Ok, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "PlayerService",
                "disable-share",
                &encode_disable_share_request(&req),
            )
            .await?;
        decode_ok(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }
}

/// Typed client for the MediaService service.
pub struct MediaAsyncClient<T: AsyncTransport> {
    #[allow(dead_code)]
    transport: T,
}

impl<T: AsyncTransport> MediaAsyncClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    // channel operation `stream` is not part of the RPC client
}

/// Typed client for the NodeService service.
pub struct NodeAsyncClient<T: AsyncTransport> {
    #[allow(dead_code)]
    transport: T,
}

impl<T: AsyncTransport> NodeAsyncClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// register (request/response).
    pub async fn register(
        &self,
        req: RegisterNodeRequest,
    ) -> Result<RegisterNodeResponse, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "NodeService",
                "register",
                &encode_register_node_request(&req),
            )
            .await?;
        decode_register_node_response(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    // channel operation `session` is not part of the RPC client
}

/// Typed client for the AdminService service.
pub struct AdminAsyncClient<T: AsyncTransport> {
    #[allow(dead_code)]
    transport: T,
}

impl<T: AsyncTransport> AdminAsyncClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// list-accounts (request/response).
    pub async fn list_accounts(&self, req: Page) -> Result<ListAccountsResponse, ClientError> {
        let csil_resp = self
            .transport
            .call("AdminService", "list-accounts", &encode_page(&req))
            .await?;
        decode_list_accounts_response(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// set-role (request/response).
    pub async fn set_role(&self, req: SetRoleRequest) -> Result<Account, ClientError> {
        let csil_resp = self
            .transport
            .call("AdminService", "set-role", &encode_set_role_request(&req))
            .await?;
        decode_account(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// trust-domain (request/response).
    pub async fn trust_domain(
        &self,
        req: TrustDomainRequest,
    ) -> Result<TrustedDomains, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "AdminService",
                "trust-domain",
                &encode_trust_domain_request(&req),
            )
            .await?;
        decode_trusted_domains(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// list-trusted-domains (request/response).
    pub async fn list_trusted_domains(&self, req: Page) -> Result<TrustedDomains, ClientError> {
        let csil_resp = self
            .transport
            .call("AdminService", "list-trusted-domains", &encode_page(&req))
            .await?;
        decode_trusted_domains(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// list-nodes (request/response).
    pub async fn list_nodes(&self, req: Page) -> Result<ListNodesResponse, ClientError> {
        let csil_resp = self
            .transport
            .call("AdminService", "list-nodes", &encode_page(&req))
            .await?;
        decode_list_nodes_response(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// rename-node (request/response).
    pub async fn rename_node(&self, req: RenameNodeRequest) -> Result<NodeInfo, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "AdminService",
                "rename-node",
                &encode_rename_node_request(&req),
            )
            .await?;
        decode_node_info(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// rename-device (request/response).
    pub async fn rename_device(&self, req: RenameDeviceRequest) -> Result<DeviceInfo, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "AdminService",
                "rename-device",
                &encode_rename_device_request(&req),
            )
            .await?;
        decode_device_info(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// create-node-token (request/response).
    pub async fn create_node_token(
        &self,
        req: CreateNodeTokenRequest,
    ) -> Result<NodeTokenResult, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "AdminService",
                "create-node-token",
                &encode_create_node_token_request(&req),
            )
            .await?;
        decode_node_token_result(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// import-track (request/response).
    pub async fn import_track(&self, req: ImportTrackRequest) -> Result<ImportResult, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "AdminService",
                "import-track",
                &encode_import_track_request(&req),
            )
            .await?;
        decode_import_result(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// get-settings (request/response).
    pub async fn get_settings(&self, req: Page) -> Result<Settings, ClientError> {
        let csil_resp = self
            .transport
            .call("AdminService", "get-settings", &encode_page(&req))
            .await?;
        decode_settings(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }

    /// set-setting (request/response).
    pub async fn set_setting(&self, req: SetSettingRequest) -> Result<Settings, ClientError> {
        let csil_resp = self
            .transport
            .call(
                "AdminService",
                "set-setting",
                &encode_set_setting_request(&req),
            )
            .await?;
        decode_settings(&csil_resp).map_err(|e| ClientError::Transport(e.to_string()))
    }
}
