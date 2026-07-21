// Typed service clients over a `CsilConnection`. One method per CSIL operation in
// `schema/*.csil`. Request records are encoded to canonical CBOR and decoded on
// the way back; the connection handles correlation and the ServiceError arm.
//
// Every CSIL record is, on the wire, a canonical-CBOR map keyed by its field
// names. Our CBOR codec maps a plain JS object to exactly that, so we do not need
// a hand-written codec per type — `encodeRecord`/`decodeRecord` cover them all.

import { decode as cborDecode, encode as cborEncode } from "./cbor.ts";
import type { CborValue } from "./cbor.ts";
import type { CsilConnection } from "./csil.ts";
import type {
  Account,
  AudiobookProgress,
  AudiobookProgressRequest,
  AudiobookProgressResponse,
  AlbumDetail,
  AlbumRequest,
  AlbumsResponse,
  ArtistDetail,
  ArtistRequest,
  ArtistsResponse,
  AuthRequest,
  BrowseRequest,
  CommandRequest,
  CoverArt,
  CoverArtRequest,
  CreateNodeTokenRequest,
  GroupInfo,
  DeviceInfo,
  DisableShareRequest,
  EnableShareRequest,
  ImportResult,
  ImportTrackRequest,
  ListAccountsResponse,
  ListGroupsResponse,
  LibrariesResponse,
  LibraryResyncStatus,
  ListNodesResponse,
  ListSatelliteTokensResponse,
  ListPlayersRequest,
  ListPlayersResponse,
  MediaControl,
  MediaEvent,
  NodeInfo,
  NodeReport,
  NodeTokenResult,
  Ok,
  Page,
  RegisterNodeRequest,
  RegisterNodeResponse,
  PlayerState,
  PlaylistDetail,
  PlaylistRequest,
  PlaylistsResponse,
  SearchRequest,
  SearchResponse,
  SessionInfo,
  SetRoleRequest,
  SetDeviceAccessRequest,
  SetSettingRequest,
  Settings,
  ShareResult,
  SubscribeRequest,
  TrustDomainRequest,
  TrustedDomains,
  UpdateAudiobookProgressRequest,
} from "./schema.ts";

const SESSION = "SessionService";
const LIBRARY = "LibraryService";
const PLAYER = "PlayerService";
const MEDIA = "MediaService";
const ADMIN = "AdminService";
const NODE = "NodeService";

function encodeRecord(obj: object): Uint8Array {
  return cborEncode(obj as CborValue);
}

// CSIL encodes a type-choice union as a 2-element array `[variant_index, value]` (the Rust
// codec's `csil_dec_*` expect exactly this). The variant order matches the schema's
// `PlayerCommand = CmdEnqueue / CmdRemove / …`.
const PLAYER_CMD_INDEX: Record<string, number> = {
  enqueue: 0,
  remove: 1,
  reorder: 2,
  clear: 3,
  play: 4,
  pause: 5,
  next: 6,
  previous: 7,
  seek: 8,
  volume: 9,
};

function encodeCommandRequest(req: CommandRequest): Uint8Array {
  const idx = PLAYER_CMD_INDEX[req.command.op];
  if (idx === undefined) throw new Error(`unknown player command: ${req.command.op}`);
  return cborEncode({
    player_id: req.player_id,
    command: [idx, req.command as unknown as CborValue],
  } as CborValue);
}

function decodeRecord<T>(bytes: Uint8Array): T {
  return cborDecode(bytes) as unknown as T;
}

export class SessionService {
  constructor(private readonly conn: CsilConnection) {}

  authenticate(req: AuthRequest): Promise<SessionInfo> {
    return this.conn.call(SESSION, "authenticate", encodeRecord(req), decodeRecord<SessionInfo>);
  }
  whoami(page: Page = {}): Promise<SessionInfo> {
    return this.conn.call(SESSION, "whoami", encodeRecord(page), decodeRecord<SessionInfo>);
  }
  logout(page: Page = {}): Promise<Ok> {
    return this.conn.call(SESSION, "logout", encodeRecord(page), decodeRecord<Ok>);
  }
}

export class LibraryService {
  constructor(private readonly conn: CsilConnection) {}

  listLibraries(page: Page = {}): Promise<LibrariesResponse> {
    return this.conn.call(
      LIBRARY,
      "list-libraries",
      encodeRecord(page),
      decodeRecord<LibrariesResponse>,
    );
  }
  listAlbums(req: BrowseRequest = {}): Promise<AlbumsResponse> {
    return this.conn.call(LIBRARY, "list-albums", encodeRecord(req), decodeRecord<AlbumsResponse>);
  }
  listArtists(req: BrowseRequest = {}): Promise<ArtistsResponse> {
    return this.conn.call(LIBRARY, "list-artists", encodeRecord(req), decodeRecord<ArtistsResponse>);
  }
  getAlbum(req: AlbumRequest): Promise<AlbumDetail> {
    return this.conn.call(LIBRARY, "get-album", encodeRecord(req), decodeRecord<AlbumDetail>);
  }
  getArtist(req: ArtistRequest): Promise<ArtistDetail> {
    return this.conn.call(LIBRARY, "get-artist", encodeRecord(req), decodeRecord<ArtistDetail>);
  }
  search(req: SearchRequest): Promise<SearchResponse> {
    return this.conn.call(LIBRARY, "search", encodeRecord(req), decodeRecord<SearchResponse>);
  }
  listPlaylists(req: BrowseRequest = {}): Promise<PlaylistsResponse> {
    return this.conn.call(
      LIBRARY,
      "list-playlists",
      encodeRecord(req),
      decodeRecord<PlaylistsResponse>,
    );
  }
  getPlaylist(req: PlaylistRequest): Promise<PlaylistDetail> {
    return this.conn.call(LIBRARY, "get-playlist", encodeRecord(req), decodeRecord<PlaylistDetail>);
  }
  getCoverArt(req: CoverArtRequest): Promise<CoverArt> {
    return this.conn.call(LIBRARY, "get-cover-art", encodeRecord(req), decodeRecord<CoverArt>);
  }
  getAudiobookProgress(req: AudiobookProgressRequest): Promise<AudiobookProgressResponse> {
    return this.conn.call(
      LIBRARY,
      "get-audiobook-progress",
      encodeRecord(req),
      decodeRecord<AudiobookProgressResponse>,
    );
  }
  updateAudiobookProgress(req: UpdateAudiobookProgressRequest): Promise<AudiobookProgress> {
    return this.conn.call(
      LIBRARY,
      "update-audiobook-progress",
      encodeRecord(req),
      decodeRecord<AudiobookProgress>,
    );
  }
}

export class PlayerService {
  constructor(private readonly conn: CsilConnection) {}

  listPlayers(req: ListPlayersRequest = {}): Promise<ListPlayersResponse> {
    return this.conn.call(PLAYER, "list-players", encodeRecord(req), decodeRecord<ListPlayersResponse>);
  }
  control(req: CommandRequest): Promise<PlayerState> {
    return this.conn.call(PLAYER, "control", encodeCommandRequest(req), decodeRecord<PlayerState>);
  }
  enableShare(req: EnableShareRequest = {}): Promise<ShareResult> {
    return this.conn.call(PLAYER, "enable-share", encodeRecord(req), decodeRecord<ShareResult>);
  }
  disableShare(req: DisableShareRequest): Promise<Ok> {
    return this.conn.call(PLAYER, "disable-share", encodeRecord(req), decodeRecord<Ok>);
  }

  /** Open a subscription to a shared target's live `PlayerState`. Sends the
   * `subscribe` channel request, and fans decoded states to `onState`. Returns an
   * unsubscribe function. Server pushes carry `player_id`, so a single channel
   * handler can serve many players; the caller filters. */
  subscribe(req: SubscribeRequest, onState: (state: PlayerState) => void): () => void {
    const off = this.conn.onChannel(PLAYER, "subscribe", (payload) => {
      onState(decodeRecord<PlayerState>(payload));
    });
    this.conn.sendChannel(PLAYER, "subscribe", encodeRecord(req));
    return off;
  }
}

export class AdminService {
  constructor(private readonly conn: CsilConnection) {}

  listAccounts(page: Page = {}): Promise<ListAccountsResponse> {
    return this.conn.call(ADMIN, "list-accounts", encodeRecord(page), decodeRecord<ListAccountsResponse>);
  }
  setRole(req: SetRoleRequest): Promise<Account> {
    return this.conn.call(ADMIN, "set-role", encodeRecord(req), decodeRecord<Account>);
  }
  trustDomain(req: TrustDomainRequest): Promise<TrustedDomains> {
    return this.conn.call(ADMIN, "trust-domain", encodeRecord(req), decodeRecord<TrustedDomains>);
  }
  listTrustedDomains(page: Page = {}): Promise<TrustedDomains> {
    return this.conn.call(ADMIN, "list-trusted-domains", encodeRecord(page), decodeRecord<TrustedDomains>);
  }
  listNodes(page: Page = {}): Promise<ListNodesResponse> {
    return this.conn.call(ADMIN, "list-nodes", encodeRecord(page), decodeRecord<ListNodesResponse>);
  }
  renameNode(node_id: string, friendly_name: string): Promise<NodeInfo> {
    return this.conn.call(ADMIN, "rename-node", encodeRecord({ node_id, friendly_name }), decodeRecord<NodeInfo>);
  }
  renameDevice(device_id: string, friendly_name: string): Promise<DeviceInfo> {
    return this.conn.call(ADMIN, "rename-device", encodeRecord({ device_id, friendly_name }), decodeRecord<DeviceInfo>);
  }
  setDeviceAccess(req: SetDeviceAccessRequest): Promise<DeviceInfo> {
    return this.conn.call(ADMIN, "set-device-access", encodeRecord(req), decodeRecord<DeviceInfo>);
  }
  listGroups(page: Page = {}): Promise<ListGroupsResponse> {
    return this.conn.call(ADMIN, "list-groups", encodeRecord(page), decodeRecord<ListGroupsResponse>);
  }
  createGroup(name: string): Promise<GroupInfo> {
    return this.conn.call(ADMIN, "create-group", encodeRecord({ name }), decodeRecord<GroupInfo>);
  }
  setGroupMembers(group_id: string, member_account_ids: string[]): Promise<GroupInfo> {
    return this.conn.call(ADMIN, "set-group-members", encodeRecord({ group_id, member_account_ids }), decodeRecord<GroupInfo>);
  }
  deleteGroup(group_id: string): Promise<Ok> {
    return this.conn.call(ADMIN, "delete-group", encodeRecord({ group_id }), decodeRecord<Ok>);
  }
  listSatelliteTokens(page: Page = {}): Promise<ListSatelliteTokensResponse> {
    return this.conn.call(ADMIN, "list-satellite-tokens", encodeRecord(page), decodeRecord<ListSatelliteTokensResponse>);
  }
  createNodeToken(req: CreateNodeTokenRequest = { default_group_ids: ["everyone"] }): Promise<NodeTokenResult> {
    return this.conn.call(ADMIN, "create-node-token", encodeRecord(req), decodeRecord<NodeTokenResult>);
  }
  revokeSatelliteToken(satellite_id: string): Promise<Ok> {
    return this.conn.call(ADMIN, "revoke-satellite-token", encodeRecord({ satellite_id }), decodeRecord<Ok>);
  }
  importTrack(req: ImportTrackRequest): Promise<ImportResult> {
    return this.conn.call(ADMIN, "import-track", encodeRecord(req), decodeRecord<ImportResult>);
  }
  getSettings(page: Page = {}): Promise<Settings> {
    return this.conn.call(ADMIN, "get-settings", encodeRecord(page), decodeRecord<Settings>);
  }
  setSetting(req: SetSettingRequest): Promise<Settings> {
    return this.conn.call(ADMIN, "set-setting", encodeRecord(req), decodeRecord<Settings>);
  }
  resyncLibrary(page: Page = {}): Promise<LibraryResyncStatus> {
    return this.conn.call(
      ADMIN,
      "resync-library",
      encodeRecord(page),
      decodeRecord<LibraryResyncStatus>,
    );
  }
  getResyncStatus(page: Page = {}): Promise<LibraryResyncStatus> {
    return this.conn.call(
      ADMIN,
      "get-resync-status",
      encodeRecord(page),
      decodeRecord<LibraryResyncStatus>,
    );
  }
}

export class NodeService {
  constructor(private readonly conn: CsilConnection) {}
  register(req: RegisterNodeRequest): Promise<RegisterNodeResponse> {
    return this.conn.call(NODE, "register", encodeRecord(req), decodeRecord<RegisterNodeResponse>);
  }
  report(report: NodeReport): void {
    this.conn.sendChannel(NODE, "session", encodeRecord(report));
  }
}

/** The MediaService bidi stream (§5). One stream per connection: the client
 * sends `MediaControl`, the server pushes `MediaEvent`s (header, chunks, end,
 * error). The `PlayerController` drives this. */
export class MediaStream {
  private off?: () => void;

  constructor(private readonly conn: CsilConnection) {}

  /** Begin listening for server-pushed media events. */
  listen(onEvent: (event: MediaEvent) => void): void {
    this.off?.();
    this.off = this.conn.onChannel(MEDIA, "stream", (payload) => {
      onEvent(decodeRecord<MediaEvent>(payload));
    });
  }

  /** Send a control message up the stream (open/seek/pause/resume/stop). */
  send(control: MediaControl): void {
    this.conn.sendChannel(MEDIA, "stream", encodeRecord(control));
  }

  dispose(): void {
    this.off?.();
    this.off = undefined;
  }
}

/** All service clients bound to one server connection. */
export class ServerApi {
  readonly session: SessionService;
  readonly library: LibraryService;
  readonly player: PlayerService;
  readonly admin: AdminService;
  readonly node: NodeService;

  constructor(readonly conn: CsilConnection) {
    this.session = new SessionService(conn);
    this.library = new LibraryService(conn);
    this.player = new PlayerService(conn);
    this.admin = new AdminService(conn);
    this.node = new NodeService(conn);
  }

  mediaStream(): MediaStream {
    return new MediaStream(this.conn);
  }
}
