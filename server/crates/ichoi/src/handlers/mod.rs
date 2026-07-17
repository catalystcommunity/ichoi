//! Handler implementations: the generated CSIL service traits wired to the store.
//!
//! Request/response operations are fully DB-backed. Channel operations (Player.subscribe,
//! Media.stream, Node.session) have working entry points but their streaming state machines
//! are pre-alpha stubs (§16) — the media/jukebox loops are the next implementation frontier.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use libichoi::csil::services::*;
use libichoi::csil::types::*;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::Config;
use crate::db::{models, store, SqlitePool};
use crate::{auth, media};

type Subscriber = (u64, UnboundedSender<Vec<u8>>);
type SubscriberMap = Arc<Mutex<HashMap<String, Vec<Subscriber>>>>;

/// Live pub/sub for shared-player state (§6.5): connections subscribe to a player and get a
/// pushed `PlayerState` frame whenever anyone controls it. Cheap in-memory fan-out.
#[derive(Clone, Default)]
pub struct SubHub {
    inner: SubscriberMap,
}

impl SubHub {
    pub fn new() -> SubHub {
        SubHub::default()
    }

    pub fn subscribe(&self, player_id: String, conn_id: u64, tx: UnboundedSender<Vec<u8>>) {
        let mut map = self.inner.lock().unwrap();
        let list = map.entry(player_id).or_default();
        list.retain(|(c, _)| *c != conn_id);
        list.push((conn_id, tx));
    }

    pub fn unsubscribe_conn(&self, conn_id: u64) {
        let mut map = self.inner.lock().unwrap();
        for list in map.values_mut() {
            list.retain(|(c, _)| *c != conn_id);
        }
    }

    pub fn publish(&self, player_id: &str, frame: &[u8]) {
        let map = self.inner.lock().unwrap();
        if let Some(list) = map.get(player_id) {
            for (_, tx) in list {
                let _ = tx.send(frame.to_vec());
            }
        }
    }
}

/// Live directive channels to satellite nodes. A satellite opens `NodeService.session` for
/// each player it owns; core controls publish encoded `NodeDirective` payloads here.
#[derive(Clone, Default)]
pub struct NodeHub {
    inner: SubscriberMap,
}

impl NodeHub {
    pub fn new() -> NodeHub {
        NodeHub::default()
    }

    pub fn subscribe(&self, player_id: String, conn_id: u64, tx: UnboundedSender<Vec<u8>>) {
        let mut map = self.inner.lock().unwrap();
        let list = map.entry(player_id).or_default();
        list.retain(|(c, _)| *c != conn_id);
        list.push((conn_id, tx));
    }

    pub fn unsubscribe_conn(&self, conn_id: u64) {
        let mut map = self.inner.lock().unwrap();
        for list in map.values_mut() {
            list.retain(|(c, _)| *c != conn_id);
        }
    }

    pub fn publish(&self, player_id: &str, payload: Vec<u8>) {
        let map = self.inner.lock().unwrap();
        if let Some(list) = map.get(player_id) {
            for (_, tx) in list {
                let _ = tx.send(payload.clone());
            }
        }
    }
}

/// Live output presence (§6): which connections are acting as the OUTPUT (speaker) for a
/// shared device. A shared device is "live" — listed and controllable — only while at least
/// one connection owns its output. This reconciles the persisted device rows against the
/// browsers actually attached to the server, so a device orphaned by a refresh stops
/// pretending to play until its owner reconnects and re-claims it.
#[derive(Clone, Default)]
pub struct Presence {
    inner: Arc<Mutex<HashMap<String, HashSet<u64>>>>,
}

impl Presence {
    pub fn new() -> Presence {
        Presence::default()
    }

    /// Register `conn_id` as an output for `player_id` (via a successful EnableShare claim).
    pub fn attach(&self, player_id: String, conn_id: u64) {
        self.inner
            .lock()
            .unwrap()
            .entry(player_id)
            .or_default()
            .insert(conn_id);
    }

    /// Drop a connection from every device it was outputting (on socket close).
    pub fn detach_conn(&self, conn_id: u64) {
        let mut map = self.inner.lock().unwrap();
        map.retain(|_, set| {
            set.remove(&conn_id);
            !set.is_empty()
        });
    }

    pub fn is_present(&self, player_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .get(player_id)
            .is_some_and(|s| !s.is_empty())
    }
}

/// Who is calling, resolved from the transport `$hello` (pre-alpha: usually Anonymous).
#[derive(Debug, Clone)]
pub enum Identity {
    Anonymous,
    User { account_id: String, role: String },
    Node { node_id: String },
}

/// Per-call context.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub identity: Identity,
}

/// The application: shared state behind every handler.
#[derive(Clone)]
pub struct App {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub subs: SubHub,
    pub presence: Presence,
    pub nodes: NodeHub,
    pub local_rp: Option<crate::auth::local_rp::DynBackend>,
}

impl App {
    pub fn new(pool: SqlitePool, config: Arc<Config>) -> App {
        let local_rp = if config.linkkeys_local_rp {
            Some(Arc::new(
                crate::auth::local_rp::SdkBackend::load(&pool)
                    .expect("validated local RP configuration must have an initialized identity"),
            ) as crate::auth::local_rp::DynBackend)
        } else {
            None
        };
        App {
            pool,
            config,
            subs: SubHub::new(),
            presence: Presence::new(),
            nodes: NodeHub::new(),
            local_rp,
        }
    }

    pub fn with_local_rp_backend(mut self, backend: crate::auth::local_rp::DynBackend) -> App {
        self.local_rp = Some(backend);
        self
    }

    fn conn(&self) -> Result<crate::db::PooledConn, ServiceError> {
        self.pool.get().map_err(internal)
    }
}

// ------------------------------------------------------------------ error helpers

fn err(code: i64, message: impl Into<String>) -> ServiceError {
    ServiceError {
        code,
        message: message.into(),
    }
}
fn internal<E: std::fmt::Display>(e: E) -> ServiceError {
    err(500, format!("internal: {e}"))
}
fn db<T>(r: diesel::QueryResult<T>) -> Result<T, ServiceError> {
    r.map_err(internal)
}

fn setting_enabled(
    conn: &mut diesel::sqlite::SqliteConnection,
    key: &str,
) -> Result<bool, ServiceError> {
    Ok(matches!(
        db(store::get_setting(conn, key))?.as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    ))
}

// ------------------------------------------------------------------ enum mapping

fn to_role(s: &str) -> Role {
    match s {
        "admin" => Role::Admin,
        "guest" => Role::Guest,
        _ => Role::Member,
    }
}
fn role_str(r: &Role) -> &'static str {
    match r {
        Role::Admin => "admin",
        Role::Member => "member",
        Role::Guest => "guest",
    }
}
fn to_codec(s: &str) -> Codec {
    match s {
        "aac" => Codec::Aac,
        "vorbis" => Codec::Vorbis,
        "flac" => Codec::Flac,
        "alac" => Codec::Alac,
        "opus" => Codec::Opus,
        "wav" => Codec::Wav,
        "wma" => Codec::Wma,
        _ => Codec::Mp3,
    }
}
fn to_status(s: &str) -> PlayerStatus {
    match s {
        "playing" => PlayerStatus::Playing,
        "paused" => PlayerStatus::Paused,
        _ => PlayerStatus::Stopped,
    }
}
fn status_str(s: &PlayerStatus) -> &'static str {
    match s {
        PlayerStatus::Playing => "playing",
        PlayerStatus::Paused => "paused",
        PlayerStatus::Stopped => "stopped",
    }
}
fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

// ------------------------------------------------------------------ model mapping

fn map_account(a: models::Account) -> Account {
    Account {
        id: a.id,
        handle: a.handle,
        display_name: a.display_name,
        role: to_role(&a.role),
        created_at: parse_dt(&a.created_at),
    }
}

fn map_track(t: &models::Track) -> Track {
    Track {
        id: t.id.clone(),
        title: t.title.clone(),
        artist_id: t.artist_id.clone(),
        album_id: t.album_id.clone(),
        track_no: t.track_no.map(|n| n as u64),
        disc_no: t.disc_no.map(|n| n as u64),
        duration_ms: t.duration_ms.max(0) as u64,
        codec: to_codec(&t.codec),
        bitrate_kbps: t.bitrate_kbps.map(|n| n as u64),
        sample_rate: t.sample_rate.max(0) as u64,
        channels: t.channels.max(0) as u64,
        bit_depth: t.bit_depth.map(|n| n as u64),
        root_relative_path: t.root_relative_path.clone(),
        content_hash: t.content_hash.clone(),
    }
}

fn map_album(
    conn: &mut diesel::SqliteConnection,
    a: &models::Album,
) -> Result<Album, ServiceError> {
    let track_count = db(store::count_tracks_for_album(conn, &a.id))?;
    Ok(Album {
        id: a.id.clone(),
        title: a.title.clone(),
        artist_id: a.artist_id.clone(),
        year: a.year.map(|y| y as u64),
        has_cover_art: a.has_cover_art != 0,
        track_count: track_count.max(0) as u64,
    })
}

fn map_artist(
    conn: &mut diesel::SqliteConnection,
    a: &models::Artist,
) -> Result<Artist, ServiceError> {
    let album_count = db(store::count_albums_for_artist(conn, &a.id))?;
    Ok(Artist {
        id: a.id.clone(),
        name: a.name.clone(),
        album_count: album_count.max(0) as u64,
    })
}

fn page(offset: Option<u64>, limit: Option<u64>) -> (i64, i64) {
    (
        offset.unwrap_or(0) as i64,
        limit.unwrap_or(100).min(1000) as i64,
    )
}

// ================================================================== SessionService

impl SessionService for App {
    type Context = Ctx;

    fn authenticate(&self, _ctx: &Ctx, input: AuthRequest) -> Result<SessionInfo, ServiceError> {
        let mut conn = self.conn()?;

        // First-admin bootstrap: valid only while zero accounts exist (§7.4).
        if let Some(bt) = input.bootstrap_token.as_ref() {
            if db(store::count_accounts(&mut conn))? == 0 {
                if self.config.admin_token.as_deref() != Some(bt.as_str()) {
                    return Err(err(403, "invalid bootstrap token"));
                }
                let acct = models::Account {
                    id: "admin@local".to_string(),
                    handle: "admin".to_string(),
                    display_name: None,
                    role: "admin".to_string(),
                    created_at: Utc::now().to_rfc3339(),
                };
                db(store::upsert_account(&mut conn, &acct))?;
                return self.mint_session(&mut conn, acct);
            }
        }

        // LinkKeys assertion path. TODO: verify via linkkeys-rpc-client and extract
        // uuid@domain + the `handle` claim (§7.1). Pre-alpha accepts a placeholder identity.
        if input.linkkeys_assertion.is_some() {
            if self.config.linkkeys_local_rp {
                return Err(err(
                    400,
                    "full-RP assertions are not accepted while local RP mode is enabled",
                ));
            }
            let acct = models::Account {
                id: "user@example.com".to_string(),
                handle: "user".to_string(),
                display_name: None,
                role: "member".to_string(),
                created_at: Utc::now().to_rfc3339(),
            };
            db(store::upsert_account(&mut conn, &acct))?;
            return self.mint_session(&mut conn, acct);
        }

        if let Some(code) = input.linkkeys_exchange_code.as_deref() {
            if !self.config.linkkeys_local_rp {
                return Err(err(401, "LinkKeys local RP mode is disabled"));
            }
            let exchange = db(store::consume_linkkeys_exchange(
                &mut conn,
                &auth::sha256_hex(code),
            ))?
            .filter(|row| !crate::auth::local_rp::is_expired(&row.expires_at))
            .ok_or_else(|| err(401, "invalid or expired LinkKeys login exchange"))?;
            let acct = db(store::get_account(&mut conn, &exchange.account_id))?
                .ok_or_else(|| err(401, "LinkKeys account no longer exists"))?;
            return self.mint_session(&mut conn, acct);
        }

        Err(err(401, "no credential; login-less connections are guests"))
    }

    fn whoami(&self, ctx: &Ctx, _input: Page) -> Result<SessionInfo, ServiceError> {
        match &ctx.identity {
            Identity::User { account_id, .. } => {
                let mut conn = self.conn()?;
                let a = db(store::get_account(&mut conn, account_id))?
                    .ok_or_else(|| err(404, "account not found"))?;
                Ok(SessionInfo {
                    account_id: a.id,
                    handle: a.handle,
                    display_name: a.display_name,
                    role: to_role(&a.role),
                    token: None,
                })
            }
            _ => Err(err(401, "not authenticated")),
        }
    }

    fn logout(&self, _ctx: &Ctx, _input: Page) -> Result<Ok, ServiceError> {
        // Token revocation happens at the transport (it holds the presented token). No-op here.
        Ok(Ok { ok: true })
    }
}

impl App {
    fn mint_session(
        &self,
        conn: &mut diesel::SqliteConnection,
        acct: models::Account,
    ) -> Result<SessionInfo, ServiceError> {
        let minted = auth::mint_token();
        let expires = (Utc::now() + chrono::Duration::days(30)).to_rfc3339();
        db(store::create_session(
            conn,
            &minted.sha256_hex,
            &acct.id,
            &expires,
        ))?;
        Ok(SessionInfo {
            account_id: acct.id,
            handle: acct.handle,
            display_name: acct.display_name,
            role: to_role(&acct.role),
            token: Some(minted.token),
        })
    }
}

// ================================================================== LibraryService

impl LibraryService for App {
    type Context = Ctx;

    fn list_albums(
        &self,
        _ctx: &Ctx,
        input: BrowseRequest,
    ) -> Result<AlbumsResponse, ServiceError> {
        let mut conn = self.conn()?;
        let (off, lim) = page(input.offset, input.limit);
        let rows = db(store::list_albums(&mut conn, off, lim))?;
        let total = db(store::count_albums(&mut conn))?.max(0) as u64;
        let albums = rows
            .iter()
            .map(|a| map_album(&mut conn, a))
            .collect::<Result<Vec<_>, _>>()?;
        std::result::Result::Ok(AlbumsResponse { albums, total })
    }

    fn list_artists(
        &self,
        _ctx: &Ctx,
        input: BrowseRequest,
    ) -> Result<ArtistsResponse, ServiceError> {
        let mut conn = self.conn()?;
        let (off, lim) = page(input.offset, input.limit);
        let rows = db(store::list_artists(&mut conn, off, lim))?;
        let total = db(store::count_artists(&mut conn))?.max(0) as u64;
        let artists = rows
            .iter()
            .map(|a| map_artist(&mut conn, a))
            .collect::<Result<Vec<_>, _>>()?;
        std::result::Result::Ok(ArtistsResponse { artists, total })
    }

    fn get_album(&self, _ctx: &Ctx, input: AlbumRequest) -> Result<AlbumDetail, ServiceError> {
        let mut conn = self.conn()?;
        let a = db(store::get_album(&mut conn, &input.album_id))?
            .ok_or_else(|| err(404, "album not found"))?;
        let album = map_album(&mut conn, &a)?;
        let tracks = db(store::tracks_for_album(&mut conn, &input.album_id))?
            .iter()
            .map(map_track)
            .collect();
        std::result::Result::Ok(AlbumDetail { album, tracks })
    }

    fn get_artist(&self, _ctx: &Ctx, input: ArtistRequest) -> Result<ArtistDetail, ServiceError> {
        let mut conn = self.conn()?;
        let a = db(store::get_artist(&mut conn, &input.artist_id))?
            .ok_or_else(|| err(404, "artist not found"))?;
        let artist = map_artist(&mut conn, &a)?;
        let album_rows = db(store::albums_for_artist(&mut conn, &input.artist_id))?;
        let albums = album_rows
            .iter()
            .map(|al| map_album(&mut conn, al))
            .collect::<Result<Vec<_>, _>>()?;
        std::result::Result::Ok(ArtistDetail { artist, albums })
    }

    fn search(&self, _ctx: &Ctx, input: SearchRequest) -> Result<SearchResponse, ServiceError> {
        let mut conn = self.conn()?;
        let lim = input.limit.unwrap_or(50).min(500) as i64;
        let artist_rows = db(store::search_artists(&mut conn, &input.query, lim))?;
        let album_rows = db(store::search_albums(&mut conn, &input.query, lim))?;
        let tracks = db(store::search_tracks(&mut conn, &input.query, lim))?
            .iter()
            .map(map_track)
            .collect();
        let artists = artist_rows
            .iter()
            .map(|a| map_artist(&mut conn, a))
            .collect::<Result<Vec<_>, _>>()?;
        let albums = album_rows
            .iter()
            .map(|a| map_album(&mut conn, a))
            .collect::<Result<Vec<_>, _>>()?;
        std::result::Result::Ok(SearchResponse {
            artists,
            albums,
            tracks,
        })
    }

    fn list_playlists(
        &self,
        _ctx: &Ctx,
        _input: BrowseRequest,
    ) -> Result<PlaylistsResponse, ServiceError> {
        let mut conn = self.conn()?;
        let playlists = db(store::list_playlists(&mut conn))?
            .into_iter()
            .map(|p| Playlist {
                id: p.id,
                name: p.name,
                owner: p.owner,
                entry_count: 0,
                root_relative_path: p.root_relative_path,
            })
            .collect();
        std::result::Result::Ok(PlaylistsResponse { playlists })
    }

    fn get_playlist(
        &self,
        _ctx: &Ctx,
        input: PlaylistRequest,
    ) -> Result<PlaylistDetail, ServiceError> {
        let mut conn = self.conn()?;
        let p = db(store::get_playlist(&mut conn, &input.playlist_id))?
            .ok_or_else(|| err(404, "playlist not found"))?;
        let tracks: Vec<Track> = self
            .config
            .music_dir
            .as_ref()
            .and_then(|root| std::fs::read_to_string(root.join(&p.root_relative_path)).ok())
            .map(|text| {
                libichoi::m3u::parse(&text)
                    .into_iter()
                    .filter_map(|path| {
                        db(store::track_by_root_path(&mut conn, &path))
                            .ok()
                            .flatten()
                    })
                    .map(|t| map_track(&t))
                    .collect()
            })
            .unwrap_or_default();
        std::result::Result::Ok(PlaylistDetail {
            playlist: Playlist {
                id: p.id,
                name: p.name,
                owner: p.owner,
                entry_count: tracks.len() as u64,
                root_relative_path: p.root_relative_path,
            },
            tracks,
        })
    }

    fn get_cover_art(&self, _ctx: &Ctx, input: CoverArtRequest) -> Result<CoverArt, ServiceError> {
        let mut conn = self.conn()?;
        let a = db(store::get_album(&mut conn, &input.album_id))?
            .ok_or_else(|| err(404, "album not found"))?;

        // Prefer a folder cover file; otherwise pull embedded art from a track (§ cover art).
        if let Some(path) = a.cover_art_path {
            if let std::result::Result::Ok(data) = std::fs::read(&path) {
                let content_type = if path.ends_with(".png") {
                    "image/png"
                } else {
                    "image/jpeg"
                };
                return std::result::Result::Ok(CoverArt {
                    content_type: content_type.to_string(),
                    data,
                });
            }
        }
        for track in db(store::tracks_for_album(&mut conn, &input.album_id))? {
            if let std::result::Result::Ok(Some(lib)) =
                store::get_library(&mut conn, &track.library_id)
            {
                let file = std::path::Path::new(&lib.path).join(&track.root_relative_path);
                if let Some((mime, data)) = crate::scan::extract_embedded_cover(&file) {
                    return std::result::Result::Ok(CoverArt {
                        content_type: mime,
                        data,
                    });
                }
            }
        }
        Err(err(404, "no cover art"))
    }
}

// ================================================================== PlayerService

impl App {
    pub fn load_player_state(
        &self,
        conn: &mut diesel::SqliteConnection,
        player_id: &str,
    ) -> Result<PlayerState, ServiceError> {
        let st = db(store::get_state(conn, player_id))?;
        let items = db(store::queue_items(conn, player_id))?;
        let queue = items
            .iter()
            .map(|it| {
                let t = store::get_track(conn, &it.track_id).ok().flatten();
                QueueItem {
                    track_id: it.track_id.clone(),
                    title: t.as_ref().map(|t| t.title.clone()),
                    artist: None,
                    duration_ms: t.as_ref().map(|t| t.duration_ms.max(0) as u64),
                }
            })
            .collect();
        let st = st.unwrap_or(models::PlayerStateRow {
            player_id: player_id.to_string(),
            status: "stopped".to_string(),
            current_index: None,
            position_ms: None,
            volume: 100,
        });
        std::result::Result::Ok(PlayerState {
            player_id: player_id.to_string(),
            status: to_status(&st.status),
            current_index: st.current_index.map(|n| n as u64),
            position_ms: st.position_ms.map(|n| n.max(0) as u64),
            volume: st.volume.clamp(0, 100) as u64,
            queue,
        })
    }

    pub fn record_node_report(&self, report: NodeReport) -> Result<PlayerState, ServiceError> {
        let mut conn = self.conn()?;
        let row = models::PlayerStateRow {
            player_id: report.player_id.clone(),
            status: status_str(&report.status).to_string(),
            current_index: db(store::get_state(&mut conn, &report.player_id))?
                .and_then(|s| s.current_index),
            position_ms: report.position_ms.map(|p| p as i64),
            volume: db(store::get_state(&mut conn, &report.player_id))?
                .map(|s| s.volume)
                .unwrap_or(100),
        };
        db(store::upsert_state(&mut conn, &row))?;
        let state = self.load_player_state(&mut conn, &report.player_id)?;
        self.subs.publish(
            &report.player_id,
            &crate::transport::player_state_frame(&state),
        );
        Ok(state)
    }
}

impl PlayerService for App {
    type Context = Ctx;

    fn list_players(
        &self,
        _ctx: &Ctx,
        input: ListPlayersRequest,
    ) -> Result<ListPlayersResponse, ServiceError> {
        let mut conn = self.conn()?;
        let kind = input.kind.as_ref().map(|k| match k {
            PlayerKind::Shared => "shared",
            PlayerKind::Private => "private",
        });
        let server_output_enabled = setting_enabled(&mut conn, "server_output_enabled")?;
        let mut players: Vec<Player> = db(store::list_players(&mut conn, kind))?
            .into_iter()
            // Reconcile against live connections: a shared device is only listed while a
            // connection is actually acting as its output (§6). Private players are unaffected.
            .filter(|p| {
                if p.kind != "shared" {
                    return true;
                }
                if p.id.starts_with("player:core:") {
                    return server_output_enabled;
                }
                p.output_device_id.is_some() || self.presence.is_present(&p.id)
            })
            .map(|p| Player {
                id: p.id,
                kind: if p.kind == "private" {
                    PlayerKind::Private
                } else {
                    PlayerKind::Shared
                },
                name: p.name,
                node_id: None,
                device_id: p.output_device_id,
                owner: p.owner_account_id,
            })
            .collect();
        players.sort_by(|a, b| a.name.cmp(&b.name));
        std::result::Result::Ok(ListPlayersResponse { players })
    }

    fn subscribe(&self, _ctx: &Ctx, _msg: SubscribeRequest) -> Result<(), ServiceError> {
        // The subscription push loop lives in the transport; this inbound entry acknowledges.
        // TODO: register the subscriber and stream PlayerState snapshots (§6.5).
        std::result::Result::Ok(())
    }

    fn control(&self, ctx: &Ctx, input: CommandRequest) -> Result<PlayerState, ServiceError> {
        let mut conn = self.conn()?;
        let pid = &input.player_id;
        if let Some(player) = db(store::get_player(&mut conn, pid))? {
            if player.kind == "shared" && db(store::count_accounts(&mut conn))? > 0 {
                match &ctx.identity {
                    Identity::User { .. } => {}
                    _ => return Err(err(401, "must be signed in to control shared devices")),
                }
            }
        }
        let mut st = db(store::get_state(&mut conn, pid))?.unwrap_or(models::PlayerStateRow {
            player_id: pid.clone(),
            status: "stopped".to_string(),
            current_index: None,
            position_ms: None,
            volume: 100,
        });
        let mut queue: Vec<String> = db(store::queue_items(&mut conn, pid))?
            .into_iter()
            .map(|q| q.track_id)
            .collect();

        apply_command(&input.command, &mut st, &mut queue);

        db(store::set_queue(&mut conn, pid, &queue))?;
        db(store::upsert_state(&mut conn, &st))?;
        let state = self.load_player_state(&mut conn, pid)?;
        if let Some(player) = db(store::get_player(&mut conn, pid))? {
            if player.output_device_id.is_some() && !pid.starts_with("player:core:") {
                if let Some(dir) = directive_for(&input.command, pid, &st, &queue) {
                    self.nodes
                        .publish(pid, libichoi::csil_channel::encode_node_directive(&dir));
                }
            }
        }
        // Push the new state to everyone subscribed to this player (§6.5).
        self.subs
            .publish(pid, &crate::transport::player_state_frame(&state));
        std::result::Result::Ok(state)
    }

    fn enable_share(
        &self,
        ctx: &Ctx,
        input: EnableShareRequest,
    ) -> Result<ShareResult, ServiceError> {
        let mut conn = self.conn()?;
        // Signed-in users share under their handle. On a login-less instance (no accounts)
        // anyone may share a device — it becomes "Guest's <suffix>" (§6.3, §7.3).
        let (handle, owner, id_seed) = match &ctx.identity {
            Identity::User { account_id, .. } => {
                let acct = db(store::get_account(&mut conn, account_id))?
                    .ok_or_else(|| err(404, "account not found"))?;
                (acct.handle, Some(acct.id.clone()), acct.id)
            }
            _ => {
                if db(store::count_accounts(&mut conn))? == 0 {
                    ("Guest".to_string(), None, "guest".to_string())
                } else {
                    return Err(err(401, "must be signed in to share a device"));
                }
            }
        };
        let suffix = input
            .suffix
            .unwrap_or_else(|| libichoi::share::DEFAULT_SUFFIX.to_string());
        libichoi::share::validate_suffix(&suffix).map_err(|e| err(409, e.to_string()))?;
        let name = libichoi::share::share_name(&handle, &suffix);
        let id = format!("share:{id_seed}:{suffix}");
        // Idempotent re-claim: if this exact device already exists and belongs to the same
        // owner, return it (the caller becomes its output again — see Presence). Only a genuine
        // cross-owner name collision is a conflict. This lets a client re-attach after a refresh
        // instead of 409ing on its own device.
        if let Some(existing) = db(store::get_player(&mut conn, &id))? {
            if existing.owner_account_id == owner {
                return std::result::Result::Ok(ShareResult {
                    player: Player {
                        id: existing.id,
                        kind: PlayerKind::Shared,
                        name: existing.name,
                        node_id: None,
                        device_id: existing.output_device_id,
                        owner: existing.owner_account_id,
                    },
                });
            }
            return Err(err(409, "that device name is already taken on this server"));
        }
        if db(store::player_name_taken(&mut conn, &name))? {
            return Err(err(409, "that device name is already taken on this server"));
        }
        let player = models::Player {
            id,
            kind: "shared".to_string(),
            output_device_id: None,
            owner_account_id: owner.clone(),
            name: name.clone(),
            name_suffix: Some(suffix),
        };
        db(store::create_player(&mut conn, &player))?;
        std::result::Result::Ok(ShareResult {
            player: Player {
                id: player.id,
                kind: PlayerKind::Shared,
                name,
                node_id: None,
                device_id: None,
                owner,
            },
        })
    }

    fn disable_share(&self, _ctx: &Ctx, input: DisableShareRequest) -> Result<Ok, ServiceError> {
        let mut conn = self.conn()?;
        db(store::delete_player(&mut conn, &input.player_id))?;
        std::result::Result::Ok(Ok { ok: true })
    }
}

fn apply_command(cmd: &PlayerCommand, st: &mut models::PlayerStateRow, queue: &mut Vec<String>) {
    match cmd {
        PlayerCommand::Variant0(enq) => {
            let at = enq.at_index.map(|i| i as usize).unwrap_or(queue.len());
            let at = at.min(queue.len());
            for (i, tid) in enq.track_ids.iter().enumerate() {
                queue.insert((at + i).min(queue.len()), tid.clone());
            }
        }
        PlayerCommand::Variant1(rem) => {
            let i = rem.index as usize;
            if i < queue.len() {
                queue.remove(i);
            }
        }
        PlayerCommand::Variant2(reorder) => {
            let (from, to) = (reorder.from_index as usize, reorder.to_index as usize);
            if from < queue.len() {
                let item = queue.remove(from);
                queue.insert(to.min(queue.len()), item);
            }
        }
        PlayerCommand::Variant3(_clear) => {
            queue.clear();
            st.current_index = None;
            st.status = "stopped".to_string();
        }
        PlayerCommand::Variant4(play) => {
            st.status = "playing".to_string();
            if let Some(i) = play.index {
                st.current_index = Some(i as i32);
            } else if st.current_index.is_none() && !queue.is_empty() {
                st.current_index = Some(0);
            }
            st.position_ms = Some(0);
        }
        PlayerCommand::Variant5(_pause) => st.status = "paused".to_string(),
        PlayerCommand::Variant6(_next) => {
            let cur = st.current_index.unwrap_or(-1);
            let next = cur + 1;
            if (next as usize) < queue.len() {
                st.current_index = Some(next);
                st.position_ms = Some(0);
            }
        }
        PlayerCommand::Variant7(_prev) => {
            let cur = st.current_index.unwrap_or(0);
            if cur > 0 {
                st.current_index = Some(cur - 1);
                st.position_ms = Some(0);
            }
        }
        PlayerCommand::Variant8(seek) => st.position_ms = Some(seek.position_ms as i64),
        PlayerCommand::Variant9(vol) => st.volume = (vol.volume.min(100)) as i32,
    }
}

fn directive_for(
    cmd: &PlayerCommand,
    player_id: &str,
    st: &models::PlayerStateRow,
    queue: &[String],
) -> Option<NodeDirective> {
    match cmd {
        PlayerCommand::Variant3(_) => Some(NodeDirective::Variant3(DirStop {
            op: "stop".to_string(),
            player_id: player_id.to_string(),
        })),
        PlayerCommand::Variant4(_) | PlayerCommand::Variant6(_) | PlayerCommand::Variant7(_) => {
            let idx = st.current_index? as usize;
            let track_id = queue.get(idx)?.clone();
            Some(NodeDirective::Variant0(DirLoad {
                op: "load".to_string(),
                player_id: player_id.to_string(),
                track_id,
                pref: StreamPref {
                    max_bitrate_kbps: None,
                    prefer_original: Some(false),
                    transcode_codec: Some(TranscodeCodec::Aac),
                },
                position_ms: st.position_ms.map(|p| p.max(0) as u64),
            }))
        }
        PlayerCommand::Variant5(_) => Some(NodeDirective::Variant1(DirPause {
            op: "pause".to_string(),
            player_id: player_id.to_string(),
        })),
        PlayerCommand::Variant8(seek) => {
            let idx = st.current_index? as usize;
            let track_id = queue.get(idx)?.clone();
            Some(NodeDirective::Variant0(DirLoad {
                op: "load".to_string(),
                player_id: player_id.to_string(),
                track_id,
                pref: StreamPref {
                    max_bitrate_kbps: None,
                    prefer_original: Some(false),
                    transcode_codec: Some(TranscodeCodec::Aac),
                },
                position_ms: Some(seek.position_ms),
            }))
        }
        PlayerCommand::Variant9(vol) => Some(NodeDirective::Variant4(DirVolume {
            op: "volume".to_string(),
            player_id: player_id.to_string(),
            volume: vol.volume.min(100),
        })),
        _ => None,
    }
}

// ================================================================== MediaService

impl MediaService for App {
    type Context = Ctx;

    fn stream(&self, _ctx: &Ctx, msg: MediaControl) -> Result<(), ServiceError> {
        // Inbound control entry. The demux/transcode → chunk push loop is driven by the
        // transport (§5); this validates the control and (for `open`) that the track exists.
        if let MediaControl::Variant0(open) = &msg {
            let mut conn = self.conn()?;
            let track = db(store::get_track(&mut conn, &open.track_id))?
                .ok_or_else(|| err(404, "track not found"))?;
            let plan = media::plan_stream(&self.config, &track, &open.pref);
            log::debug!(
                "media open {}: {}",
                track.id,
                if plan.transcode.is_some() {
                    "transcode"
                } else {
                    "direct"
                }
            );
        }
        std::result::Result::Ok(())
    }
}

// ================================================================== NodeService

impl NodeService for App {
    type Context = Ctx;

    fn register(
        &self,
        ctx: &Ctx,
        input: RegisterNodeRequest,
    ) -> Result<RegisterNodeResponse, ServiceError> {
        if !matches!(ctx.identity, Identity::Node { .. }) {
            return Err(err(401, "node token required"));
        }
        let mut conn = self.conn()?;
        let node_id = format!("sat:{}", input.hostname);
        let audio = if input.outputs.is_empty() {
            "none"
        } else {
            "some"
        };
        let node = models::Node {
            id: node_id.clone(),
            kind: "satellite".to_string(),
            hostname: input.hostname.clone(),
            friendly_name: input.hostname.clone(),
            token_sha256: match &ctx.identity {
                Identity::Node { node_id } if node_id.starts_with("pending:") => {
                    Some(node_id.trim_start_matches("pending:").to_string())
                }
                _ => None,
            },
            platform: input.platform,
            arch: input.arch,
            audio_outputs: audio.to_string(),
            last_seen: Some(Utc::now().to_rfc3339()),
        };
        db(store::upsert_node(&mut conn, &node))?;

        let mut players = Vec::new();
        for out in &input.outputs {
            let dev = models::OutputDevice {
                id: format!("{}:{}", node_id, out.os_device_id),
                node_id: node_id.clone(),
                os_device_id: out.os_device_id.clone(),
                friendly_name: out
                    .friendly_name
                    .clone()
                    .unwrap_or_else(|| out.os_device_id.clone()),
                is_default: i32::from(out.is_default),
            };
            db(store::upsert_device(&mut conn, &dev))?;
            let player = models::Player {
                id: format!("player:{}", dev.id),
                kind: "shared".to_string(),
                output_device_id: Some(dev.id.clone()),
                owner_account_id: None,
                name: format!("{} · {}", node.friendly_name, dev.friendly_name),
                name_suffix: None,
            };
            db(store::create_player(&mut conn, &player))?;
            players.push(Player {
                id: player.id,
                kind: PlayerKind::Shared,
                name: player.name,
                node_id: Some(node_id.clone()),
                device_id: Some(dev.id),
                owner: None,
            });
        }

        std::result::Result::Ok(RegisterNodeResponse { node_id, players })
    }

    fn session(&self, ctx: &Ctx, msg: NodeReport) -> Result<(), ServiceError> {
        if !matches!(ctx.identity, Identity::Node { .. }) {
            return Err(err(401, "node token required"));
        }
        self.record_node_report(msg)?;
        std::result::Result::Ok(())
    }
}

// ================================================================== AdminService

impl AdminService for App {
    type Context = Ctx;

    fn list_accounts(&self, _ctx: &Ctx, input: Page) -> Result<ListAccountsResponse, ServiceError> {
        let mut conn = self.conn()?;
        let (off, lim) = page(input.offset, input.limit);
        let accounts = db(store::list_accounts(&mut conn, off, lim))?
            .into_iter()
            .map(map_account)
            .collect();
        std::result::Result::Ok(ListAccountsResponse { accounts })
    }

    fn set_role(&self, _ctx: &Ctx, input: SetRoleRequest) -> Result<Account, ServiceError> {
        let mut conn = self.conn()?;
        db(store::set_role(
            &mut conn,
            &input.account_id,
            role_str(&input.role),
        ))?;
        let a = db(store::get_account(&mut conn, &input.account_id))?
            .ok_or_else(|| err(404, "account not found"))?;
        std::result::Result::Ok(map_account(a))
    }

    fn trust_domain(
        &self,
        _ctx: &Ctx,
        input: TrustDomainRequest,
    ) -> Result<TrustedDomains, ServiceError> {
        let mut conn = self.conn()?;
        let selector = crate::auth::local_rp::parse_selector(&input.domain)
            .map_err(|e| err(400, e.to_string()))?;
        if selector.handle.is_some() {
            return Err(err(400, "trust-domain accepts a domain, not a handle"));
        }
        db(store::add_trusted_domain(&mut conn, &selector.domain))?;
        db(store::add_linkkeys_trust(
            &mut conn,
            &selector.domain,
            None,
            "admin",
        ))?;
        std::result::Result::Ok(TrustedDomains {
            domains: db(store::list_trusted_domains(&mut conn))?,
        })
    }

    fn list_trusted_domains(
        &self,
        _ctx: &Ctx,
        _input: Page,
    ) -> Result<TrustedDomains, ServiceError> {
        let mut conn = self.conn()?;
        std::result::Result::Ok(TrustedDomains {
            domains: db(store::list_trusted_domains(&mut conn))?,
        })
    }

    fn list_nodes(&self, _ctx: &Ctx, _input: Page) -> Result<ListNodesResponse, ServiceError> {
        let mut conn = self.conn()?;
        let mut nodes = Vec::new();
        for n in db(store::list_nodes(&mut conn))? {
            let devices = db(store::devices_for_node(&mut conn, &n.id))?
                .into_iter()
                .map(|d| DeviceInfo {
                    id: d.id,
                    os_device_id: d.os_device_id,
                    friendly_name: d.friendly_name,
                    is_default: d.is_default != 0,
                })
                .collect();
            nodes.push(NodeInfo {
                id: n.id,
                kind: match n.kind.as_str() {
                    "core" => NodeKind::Core,
                    "client" => NodeKind::Client,
                    _ => NodeKind::Satellite,
                },
                hostname: n.hostname,
                friendly_name: n.friendly_name,
                platform: n.platform,
                arch: n.arch,
                last_seen: n.last_seen.as_deref().map(parse_dt),
                audio_outputs: if n.audio_outputs == "some" {
                    AudioOutputsState::Some
                } else {
                    AudioOutputsState::None
                },
                devices,
            });
        }
        std::result::Result::Ok(ListNodesResponse { nodes })
    }

    fn rename_node(&self, _ctx: &Ctx, input: RenameNodeRequest) -> Result<NodeInfo, ServiceError> {
        let mut conn = self.conn()?;
        if db(store::rename_node(
            &mut conn,
            &input.node_id,
            &input.friendly_name,
        ))? == 0
        {
            return Err(err(404, "node not found"));
        }
        // Re-read via list (small N); simpler than a dedicated getter for pre-alpha.
        self.list_nodes(
            _ctx,
            Page {
                offset: None,
                limit: None,
            },
        )?
        .nodes
        .into_iter()
        .find(|n| n.id == input.node_id)
        .ok_or_else(|| err(404, "node not found"))
    }

    fn rename_device(
        &self,
        _ctx: &Ctx,
        input: RenameDeviceRequest,
    ) -> Result<DeviceInfo, ServiceError> {
        let mut conn = self.conn()?;
        if db(store::rename_device(
            &mut conn,
            &input.device_id,
            &input.friendly_name,
        ))? == 0
        {
            return Err(err(404, "device not found"));
        }
        std::result::Result::Ok(DeviceInfo {
            id: input.device_id,
            os_device_id: String::new(),
            friendly_name: input.friendly_name,
            is_default: false,
        })
    }

    fn create_node_token(
        &self,
        _ctx: &Ctx,
        _input: CreateNodeTokenRequest,
    ) -> Result<NodeTokenResult, ServiceError> {
        let minted = auth::mint_token();
        let mut conn = self.conn()?;
        db(store::set_setting(
            &mut conn,
            &format!("node_token:{}", minted.sha256_hex),
            &Utc::now().to_rfc3339(),
        ))?;
        std::result::Result::Ok(NodeTokenResult {
            token: minted.token,
            fingerprints: Vec::new(),
        })
    }

    fn import_track(
        &self,
        _ctx: &Ctx,
        input: ImportTrackRequest,
    ) -> Result<ImportResult, ServiceError> {
        // Cross-server copy destination (§7): write the file under the music root, let the
        // scanner index it. Admin-only by policy (enforced at the transport for pre-alpha).
        let root = self
            .config
            .music_dir
            .as_ref()
            .ok_or_else(|| err(503, "no music directory configured"))?;
        let dest = root.join(&input.root_relative_path);
        if dest.exists() {
            return std::result::Result::Ok(ImportResult {
                imported: false,
                track_id: None,
                skipped_existing: true,
            });
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(internal)?;
        }
        std::fs::write(&dest, &input.data).map_err(internal)?;
        std::result::Result::Ok(ImportResult {
            imported: true,
            track_id: None,
            skipped_existing: false,
        })
    }

    fn get_settings(&self, _ctx: &Ctx, _input: Page) -> Result<Settings, ServiceError> {
        let mut conn = self.conn()?;
        let entries = db(store::all_settings(&mut conn))?
            .into_iter()
            .map(|s| (s.key, s.value))
            .collect();
        std::result::Result::Ok(Settings { entries })
    }

    fn set_setting(&self, _ctx: &Ctx, input: SetSettingRequest) -> Result<Settings, ServiceError> {
        let mut conn = self.conn()?;
        db(store::set_setting(&mut conn, &input.key, &input.value))?;
        self.get_settings(
            _ctx,
            Page {
                offset: None,
                limit: None,
            },
        )
    }
}
