import { EventRouter, EventScope, type SubscriptionOptions } from "../lib/events.ts";
import type { ServerApi } from "../lib/services.ts";
import type {
  ChangeTopic,
  CommandRequest,
  DataChange,
  Player,
  PlayerState,
} from "../lib/schema.ts";

export interface PlayerCatalogChange {
  players: readonly Player[];
  addedIds: readonly string[];
  removedIds: readonly string[];
  updatedIds: readonly string[];
  initial: boolean;
}

interface PlayerCatalogEvents {
  changed: PlayerCatalogChange;
}

type ChangeEvents = { [K in ChangeTopic]: DataChange };
type PlayerStateEvents = Record<string, PlayerState>;

function samePlayer(left: Player, right: Player): boolean {
  return left.id === right.id
    && left.kind === right.kind
    && left.name === right.name
    && left.node_id === right.node_id
    && left.device_id === right.device_id
    && left.owner === right.owner
    && left.audio_blocked === right.audio_blocked;
}

/** The authoritative, normalized player catalog for one server connection. */
export class PlayerCatalogStore {
  private readonly events = new EventRouter<PlayerCatalogEvents>();
  private readonly byId = new Map<string, Player>();
  private ordered: readonly Player[] = [];
  private refreshPromise?: Promise<void>;
  private refreshQueued = false;
  private loaded = false;
  private disposed = false;
  private readonly api: ServerApi;

  constructor(api: ServerApi) {
    this.api = api;
  }

  get isLoaded(): boolean {
    return this.loaded;
  }

  get players(): readonly Player[] {
    return this.ordered;
  }

  watch(
    handler: (change: PlayerCatalogChange) => void,
    options: SubscriptionOptions = {},
  ): () => void {
    const off = this.events.on("changed", handler, options);
    if (this.loaded && !options.signal?.aborted) {
      handler({
        players: this.ordered,
        addedIds: [],
        removedIds: [],
        updatedIds: [],
        initial: true,
      });
    }
    return off;
  }

  refresh(): Promise<void> {
    if (this.disposed) return Promise.resolve();
    if (this.refreshPromise) {
      this.refreshQueued = true;
      return this.refreshPromise;
    }
    const refresh = this.api.player
      .listPlayers()
      .then((response) => {
        this.reconcile(response.players);
      })
      .catch((error) => console.warn("[players] catalog refresh failed", error))
      .finally(() => {
        if (this.refreshPromise === refresh) this.refreshPromise = undefined;
        if (this.refreshQueued && !this.disposed) {
          this.refreshQueued = false;
          void this.refresh();
        }
      });
    this.refreshPromise = refresh;
    return refresh;
  }

  reconcile(incoming: readonly Player[]): PlayerCatalogChange | undefined {
    const initial = !this.loaded;
    const nextById = new Map<string, Player>();
    const addedIds: string[] = [];
    const updatedIds: string[] = [];
    for (const candidate of incoming) {
      const current = this.byId.get(candidate.id);
      if (!current) addedIds.push(candidate.id);
      if (current && !samePlayer(current, candidate)) updatedIds.push(candidate.id);
      nextById.set(candidate.id, current && samePlayer(current, candidate) ? current : candidate);
    }
    const removedIds = [...this.byId.keys()].filter((id) => !nextById.has(id));
    this.loaded = true;
    if (!initial && addedIds.length === 0 && removedIds.length === 0 && updatedIds.length === 0) {
      return undefined;
    }
    this.byId.clear();
    for (const [id, player] of nextById) this.byId.set(id, player);
    this.ordered = incoming.map((player) => nextById.get(player.id)!);
    const change: PlayerCatalogChange = {
      players: this.ordered,
      addedIds,
      removedIds,
      updatedIds,
      initial,
    };
    this.events.emit("changed", change);
    return change;
  }

  sweep(): number {
    return this.events.sweep();
  }

  dispose(): void {
    this.disposed = true;
    this.events.clear();
  }
}

/** One decoded player-state stream per server. Components reference-count individual players. */
export class PlayerStateStore {
  private readonly events = new EventRouter<PlayerStateEvents>();
  private readonly references = new Map<string, number>();
  private readonly cache = new Map<string, PlayerState>();
  private readonly offWire: () => void;
  private disposed = false;
  private readonly api: ServerApi;

  constructor(api: ServerApi) {
    this.api = api;
    this.offWire = api.player.onState((state) => this.accept(state));
  }

  watch(
    playerId: string,
    handler: (state: PlayerState) => void,
    options: SubscriptionOptions = {},
  ): () => void {
    if (this.disposed || options.signal?.aborted) return () => undefined;
    const prior = this.references.get(playerId) ?? 0;
    this.references.set(playerId, prior + 1);
    const offEvent = this.events.on(playerId, handler);
    if (prior === 0) this.setSubscription(playerId, true);
    const current = this.cache.get(playerId);
    if (current) handler(current);
    let active = true;
    const release = () => {
      if (!active) return;
      active = false;
      offEvent();
      options.signal?.removeEventListener("abort", release);
      const remaining = (this.references.get(playerId) ?? 1) - 1;
      if (remaining > 0) {
        this.references.set(playerId, remaining);
      } else {
        this.references.delete(playerId);
        this.cache.delete(playerId);
        this.setSubscription(playerId, false);
      }
    };
    options.signal?.addEventListener("abort", release, { once: true });
    return release;
  }

  accept(state: PlayerState): void {
    if (this.disposed) return;
    this.cache.set(state.player_id, state);
    this.events.emit(state.player_id, state);
  }

  async fetch(playerId: string): Promise<PlayerState> {
    const state = await this.api.player.getState({ player_id: playerId });
    this.accept(state);
    return state;
  }

  async control(request: CommandRequest): Promise<PlayerState> {
    const state = await this.api.player.control(request);
    this.accept(state);
    return state;
  }

  reconnect(): void {
    for (const playerId of this.references.keys()) this.setSubscription(playerId, true);
  }

  sweep(): number {
    return this.events.sweep();
  }

  private setSubscription(playerId: string, active: boolean): void {
    if (this.api.conn.connectionState !== "ready") return;
    try {
      this.api.player.setSubscription(playerId, active);
    } catch (error) {
      console.warn(`[players] ${active ? "subscribe" : "unsubscribe"} failed`, error);
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const playerId of this.references.keys()) this.setSubscription(playerId, false);
    this.offWire();
    this.references.clear();
    this.cache.clear();
    this.events.clear();
  }
}

/** Per-server domain data. Every CSIL invalidation is routed here before a view sees it. */
export class ServerDataStore {
  readonly playerCatalog: PlayerCatalogStore;
  readonly playerStates: PlayerStateStore;
  readonly changes = new EventRouter<ChangeEvents>();
  private readonly scope = new EventScope();
  private readonly sweepTimer: ReturnType<typeof setInterval>;
  private ready = false;
  private readonly api: ServerApi;

  constructor(api: ServerApi) {
    this.api = api;
    this.playerCatalog = new PlayerCatalogStore(api);
    this.playerStates = new PlayerStateStore(api);
    const off = api.changes.onChange((change) => {
      this.changes.emit(change.topic, change);
    });
    this.scope.signal.addEventListener("abort", off, { once: true });
    this.scope.on(this.changes, "players", () => void this.playerCatalog.refresh());
    this.sweepTimer = setInterval(() => {
      this.changes.sweep();
      this.playerCatalog.sweep();
      this.playerStates.sweep();
    }, 60_000);
  }

  connectionReady(): void {
    if (this.ready) return;
    this.ready = true;
    this.api.changes.setWatching(true);
    this.playerStates.reconnect();
    void this.playerCatalog.refresh();
  }

  connectionClosed(): void {
    this.ready = false;
  }

  dispose(): void {
    if (this.ready && this.api.conn.connectionState === "ready") {
      this.api.changes.setWatching(false);
    }
    this.ready = false;
    clearInterval(this.sweepTimer);
    this.scope.dispose();
    this.playerCatalog.dispose();
    this.playerStates.dispose();
    this.changes.clear();
  }
}
