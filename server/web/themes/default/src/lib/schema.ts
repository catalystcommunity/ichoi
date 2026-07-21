// TypeScript mirror of the CSIL records in `schema/*.csil`. Hand-written to stay
// readable; kept in lockstep with the schema (the source of truth). Optional CSIL
// fields (`?`) are optional here; `.default` values are applied by the server, so
// we treat them as optional on read and omit them on write unless set.
//
// These are plain data shapes. The wire codec (`csil.ts`) turns them into the
// canonical CBOR maps `codec.gen.rs` expects — keyed by these exact field names.

// --- common.csil ---------------------------------------------------------

export type Role = "admin" | "member" | "guest";
export type PlayerStatus = "stopped" | "playing" | "paused";
export type Codec = "mp3" | "aac" | "vorbis" | "flac" | "alac" | "opus" | "wav";
export type TranscodeCodec = "aac" | "mp3";

export interface StreamPref {
  max_bitrate_kbps?: number;
  prefer_original?: boolean;
  transcode_codec?: TranscodeCodec;
}

export interface Page {
  offset?: number;
  limit?: number;
}

export interface Ok {
  ok: boolean;
}

export interface ServiceError {
  code: number;
  message: string;
}

// --- session.csil --------------------------------------------------------

export interface AuthRequest {
  linkkeys_assertion?: Uint8Array;
  linkkeys_exchange_code?: string;
  bootstrap_token?: string;
}

export interface SessionInfo {
  account_id: string;
  handle: string;
  display_name?: string;
  role: Role;
  can_admin?: boolean;
  /** Minted session token, returned once on `authenticate`. */
  token?: string;
}

// --- library.csil --------------------------------------------------------

export type Library = "music" | "audiobook";

export interface Track {
  id: string;
  library: Library;
  title: string;
  artist_id?: string;
  album_id?: string;
  track_no?: number;
  disc_no?: number;
  duration_ms: number;
  codec: Codec;
  bitrate_kbps?: number;
  sample_rate: number;
  channels: number;
  bit_depth?: number;
  root_relative_path: string;
  content_hash?: string;
}

export interface Album {
  id: string;
  title: string;
  artist_id?: string;
  artist_name?: string;
  year?: number;
  has_cover_art?: boolean;
  track_count: number;
}

export interface Artist {
  id: string;
  name: string;
  album_count: number;
}

export interface Playlist {
  id: string;
  name: string;
  owner?: string;
  entry_count: number;
  root_relative_path: string;
}

export interface BrowseRequest {
  library?: Library;
  offset?: number;
  limit?: number;
}

export interface LibraryInfo {
  kind: Library;
}

export interface LibrariesResponse {
  libraries: LibraryInfo[];
}

export interface AlbumsResponse {
  albums: Album[];
  total: number;
}
export interface ArtistsResponse {
  artists: Artist[];
  total: number;
}
export interface AlbumRequest {
  album_id: string;
}
export interface AlbumDetail {
  album: Album;
  tracks: Track[];
}
export interface ArtistRequest {
  artist_id: string;
}
export interface ArtistDetail {
  artist: Artist;
  albums: Album[];
}
export interface SearchRequest {
  query: string;
  library?: Library;
  limit?: number;
}
export interface SearchResponse {
  artists: Artist[];
  albums: Album[];
  tracks: Track[];
}

export interface AudiobookProgress {
  track_id: string;
  position_ms: number;
  completed: boolean;
  updated_at: Date;
}

export interface AudiobookProgressRequest {
  track_ids: string[];
}

export interface AudiobookProgressResponse {
  progress: AudiobookProgress[];
}

export interface UpdateAudiobookProgressRequest {
  track_id: string;
  position_ms: number;
  completed: boolean;
}
export interface PlaylistsResponse {
  playlists: Playlist[];
}
export interface PlaylistRequest {
  playlist_id: string;
}
export interface PlaylistDetail {
  playlist: Playlist;
  tracks: Track[];
}
export interface CoverArtRequest {
  album_id: string;
  max_size?: number;
}
export interface CoverArt {
  content_type: string;
  data: Uint8Array;
}

// --- player.csil ---------------------------------------------------------

export type PlayerKind = "shared" | "private";

export interface Player {
  id: string;
  kind: PlayerKind;
  name: string;
  node_id?: string;
  device_id?: string;
  owner?: string;
}

export interface QueueItem {
  track_id: string;
  library?: Library;
  title?: string;
  artist?: string;
  duration_ms?: number;
}

export interface PlayerState {
  player_id: string;
  status: PlayerStatus;
  current_index?: number;
  position_ms?: number;
  volume: number;
  queue: QueueItem[];
}

export interface ListPlayersRequest {
  kind?: PlayerKind;
}
export interface ListPlayersResponse {
  players: Player[];
}
export interface SubscribeRequest {
  player_id: string;
}

// Transport/queue commands, discriminated on `op`.
export type PlayerCommand =
  | { op: "enqueue"; track_ids: string[]; at_index?: number }
  | { op: "remove"; index: number }
  | { op: "reorder"; from_index: number; to_index: number }
  | { op: "clear" }
  | { op: "play"; index?: number }
  | { op: "pause" }
  | { op: "next" }
  | { op: "previous" }
  | { op: "seek"; position_ms: number }
  | { op: "volume"; volume: number };

export interface CommandRequest {
  player_id: string;
  command: PlayerCommand;
}

export interface EnableShareRequest {
  suffix?: string;
}
export interface DisableShareRequest {
  player_id: string;
}
export interface ShareResult {
  player: Player;
}

// --- media.csil ----------------------------------------------------------

// Client -> server control, discriminated on `kind`.
export type MediaControl =
  | { kind: "open"; track_id: string; pref: StreamPref }
  | { kind: "seek"; position_ms: number }
  | { kind: "pause" }
  | { kind: "resume" }
  | { kind: "stop" };

export type MediaEndReason = "eos" | "stopped";

export interface MediaHeader {
  kind: "header";
  codec: Codec;
  transcoded: boolean;
  sample_rate: number;
  channels: number;
  duration_ms?: number;
  trim_start_samples?: number;
  trim_end_samples?: number;
  codec_config?: Uint8Array;
}
export interface MediaChunk {
  kind: "chunk";
  seq: number;
  timestamp_ms?: number;
  data: Uint8Array;
}
export interface MediaEnd {
  kind: "end";
  reason?: MediaEndReason;
}
export interface MediaFail {
  kind: "error";
  error: ServiceError;
}
export type MediaEvent = MediaHeader | MediaChunk | MediaEnd | MediaFail;

// --- node.csil (admin/jukebox surface types reused in the UI) ------------

export interface AudioOutput {
  os_device_id: string;
  friendly_name?: string;
  channels: number;
  sample_rates: number[];
  is_default?: boolean;
}
export interface RegisterNodeRequest {
  hostname: string;
  platform: string;
  arch: string;
  outputs: AudioOutput[];
}
export interface RegisterNodeResponse {
  node_id: string;
  players: Player[];
}
export interface NodeReport {
  player_id: string;
  status: PlayerStatus;
  position_ms?: number;
}

// --- admin.csil ----------------------------------------------------------

export interface Account {
  id: string;
  handle: string;
  display_name?: string;
  role: Role;
  created_at: Date;
}
export interface ListAccountsResponse {
  accounts: Account[];
}
export interface SetRoleRequest {
  account_id: string;
  role: Role;
}
export interface TrustDomainRequest {
  domain: string;
}
export interface TrustedDomains {
  domains: string[];
}

export type NodeKind = "core" | "satellite" | "client";
export type AudioOutputsState = "none" | "some";

export interface DeviceInfo {
  id: string;
  os_device_id: string;
  friendly_name: string;
  is_default?: boolean;
  enabled?: boolean;
  group_ids: string[];
}
export interface NodeInfo {
  id: string;
  kind: NodeKind;
  hostname: string;
  friendly_name: string;
  platform: string;
  arch: string;
  last_seen?: Date;
  audio_outputs: AudioOutputsState;
  devices: DeviceInfo[];
}
export interface ListNodesResponse {
  nodes: NodeInfo[];
}
export interface RenameNodeRequest {
  node_id: string;
  friendly_name: string;
}
export interface RenameDeviceRequest {
  device_id: string;
  friendly_name: string;
}
export interface CreateNodeTokenRequest {
  label?: string;
  default_enabled?: boolean;
  default_group_ids: string[];
}
export interface NodeTokenResult {
  token: string;
  fingerprints: string[];
  satellite: SatelliteTokenInfo;
}
export interface SetDeviceAccessRequest {
  device_id: string;
  enabled: boolean;
  group_ids: string[];
}
export interface GroupInfo {
  id: string;
  name: string;
  member_account_ids: string[];
}
export interface ListGroupsResponse { groups: GroupInfo[] }
export interface SatelliteTokenInfo {
  id: string;
  name: string;
  default_enabled?: boolean;
  default_group_ids: string[];
  created_at: Date;
}
export interface ListSatelliteTokensResponse { satellites: SatelliteTokenInfo[] }
export interface ImportTrackRequest {
  root_relative_path: string;
  content_type: string;
  content_hash?: string;
  data: Uint8Array;
}
export interface ImportResult {
  imported: boolean;
  track_id?: string;
  skipped_existing?: boolean;
}
export interface Settings {
  entries: Record<string, string>;
}
export interface SetSettingRequest {
  key: string;
  value: string;
}
export interface LibraryResyncStatus {
  running: boolean;
  started?: boolean;
}
