// Multi-server connection store (DESIGN §7). A client holds one session per
// server, browses each, and switches the "active" server that the library/search/
// jukebox screens read from. Server definitions persist to localStorage; live
// connection objects do not.

import {
  createContext,
  createSignal,
  onCleanup,
  useContext,
  type Accessor,
  type JSX,
  type ParentProps,
} from "solid-js";
import { createStore, produce } from "solid-js/store";
import { CsilConnection, type ConnState } from "../lib/csil.ts";
import { ServerApi } from "../lib/services.ts";
import type { SessionInfo } from "../lib/schema.ts";
import { satelliteOutput, satelliteToken } from "../lib/satellite-mode.ts";
import { reloadSatelliteForUpdate } from "../lib/app-update.ts";
import { ServerDataStore } from "./server-data.ts";

export interface ServerRecord {
  id: string;
  name: string;
  url: string;
  state: ConnState;
  detail?: string;
  session?: SessionInfo;
  token?: string;
  /** The sole player this credential may control when connected as a satellite. */
  satellitePlayerId?: string;
}

interface LiveConn {
  conn: CsilConnection;
  api: ServerApi;
  data: ServerDataStore;
}

interface PersistedServer {
  id: string;
  name: string;
  url: string;
  token?: string;
}

const STORAGE_KEY = "ichoi.servers";

interface ServersContextValue {
  servers: ServerRecord[];
  activeId: Accessor<string | undefined>;
  active: Accessor<ServerRecord | undefined>;
  /** The service API for the active server, or undefined if none is connected. */
  api: Accessor<ServerApi | undefined>;
  /** Central domain data for the active server. */
  data: Accessor<ServerDataStore | undefined>;
  addServer: (name: string, url: string) => Promise<string>;
  removeServer: (id: string) => void;
  setActive: (id: string) => void;
  apiFor: (id: string) => ServerApi | undefined;
  dataFor: (id: string) => ServerDataStore | undefined;
  reconnect: (id: string) => Promise<void>;
  completeLinkkeysExchange: (code: string) => Promise<void>;
  signOut: () => Promise<void>;
}

const ServersContext = createContext<ServersContextValue>();

function loadPersisted(): PersistedServer[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as PersistedServer[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

// A random id that works in NON-secure contexts too. `crypto.randomUUID()` is only defined
// over HTTPS or on localhost, so on a plain-HTTP LAN IP (e.g. a phone hitting
// http://192.168.x.x:4042) it is `undefined` and would throw during init — blanking the app.
// `crypto.getRandomValues` IS available in insecure contexts; fall back further to Math.random.
function randomId(): string {
  const c = globalThis.crypto as Crypto | undefined;
  if (c && typeof c.randomUUID === "function") return c.randomUUID();
  if (c && typeof c.getRandomValues === "function") {
    const b = new Uint8Array(16);
    c.getRandomValues(b);
    return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  }
  return `id-${Math.random().toString(36).slice(2)}${Date.now().toString(36)}`;
}

function savePersisted(servers: ServerRecord[]): void {
  const persist: PersistedServer[] = servers.map((s) => ({
    id: s.id,
    name: s.name,
    url: s.url,
    token: s.token,
  }));
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(persist));
  } catch {
    /* storage may be unavailable */
  }
}

/** Default the URL to this origin's `/ws` when nothing is stored, so a browser
 * served by the Ichoi core connects back to it out of the box. */
function defaultServerUrl(): string {
  if (typeof location === "undefined") return "ws://localhost:4042/ws";
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/ws`;
}

function isThisOriginServer(url: string): boolean {
  if (typeof location === "undefined") return false;
  try {
    const parsed = new URL(url);
    return parsed.host === location.host;
  } catch {
    return false;
  }
}

function serverHttpUrl(websocketUrl: string, path: string): string {
  const url = new URL(websocketUrl);
  url.protocol = url.protocol === "wss:" ? "https:" : "http:";
  url.pathname = path;
  url.search = "";
  url.hash = "";
  return url.toString();
}

async function syncMediaSession(websocketUrl: string, token?: string): Promise<void> {
  await fetch(serverHttpUrl(websocketUrl, "/api/session"), {
    method: token ? "POST" : "DELETE",
    credentials: "include",
    headers: token ? { authorization: `Bearer ${token}` } : undefined,
  });
}

export function ServersProvider(props: ParentProps): JSX.Element {
  const [servers, setServers] = createStore<ServerRecord[]>([]);
  const [activeId, setActiveId] = createSignal<string | undefined>();
  const live = new Map<string, LiveConn>();

  const api = () => {
    const id = activeId();
    if (!id) return undefined;
    // Depend on the record's reactive `state` so consumers (resources) re-run
    // when a server (re)connects — the `live` map itself is not reactive.
    servers.find((s) => s.id === id)?.state;
    return live.get(id)?.api;
  };
  const data = () => {
    const id = activeId();
    if (!id) return undefined;
    servers.find((s) => s.id === id)?.state;
    return live.get(id)?.data;
  };
  const active = () => servers.find((s) => s.id === activeId());

  function patch(id: string, patchObj: Partial<ServerRecord>): void {
    setServers(
      produce((list) => {
        const rec = list.find((s) => s.id === id);
        if (rec) Object.assign(rec, patchObj);
      }),
    );
  }

  async function provisionSatellite(
    rec: ServerRecord,
    api: ServerApi,
    dataStore: ServerDataStore,
  ): Promise<void> {
    const output = satelliteOutput();
    const registered = await api.node.register({
      hostname: typeof location === "undefined" ? "browser-pwa" : location.hostname,
      platform: "chromeos",
      arch: typeof navigator === "undefined" ? "browser" : navigator.platform || "browser",
      outputs: [{
        os_device_id: output.id,
        friendly_name: output.name,
        channels: 2,
        sample_rates: [48_000],
        is_default: output.id === "default",
      }],
    });
    const player = registered.players[0];
    if (!player) throw new Error("This satellite output has been disabled by the administrator");
    const session = await api.session.whoami();
    patch(rec.id, { session, satellitePlayerId: player.id });

    // A node-session report is also the server-side presence claim. Mirror the persisted
    // player state instead of resetting it when this PWA reconnects.
    let off: (() => void) | undefined;
    off = dataStore.playerStates.watch(player.id, (state) => {
      api.node.report({
        player_id: player.id,
        status: state.status,
        position_ms: state.position_ms,
      });
      off?.();
    });
  }

  async function openConnection(rec: ServerRecord): Promise<void> {
    const nodeToken = satelliteToken();
    let initialProvisionComplete = false;
    let reprovisioning = false;
    let api: ServerApi;
    let dataStore: ServerDataStore;
    const conn = new CsilConnection({
      url: rec.url,
      auth: nodeToken ? undefined : rec.token,
      nodeToken,
      onState: (state, detail) => {
        patch(rec.id, { state, detail });
        if (state === "ready") dataStore?.connectionReady();
        if (state === "closed" || state === "error") dataStore?.connectionClosed();
        if (state === "ready" && nodeToken && initialProvisionComplete && !reprovisioning) {
          reprovisioning = true;
          void reloadSatelliteForUpdate(rec.url)
            .then((reloading) => reloading ? undefined : provisionSatellite(rec, api, dataStore))
            .catch((e) => patch(rec.id, { state: "error", detail: String(e) }))
            .finally(() => {
              reprovisioning = false;
            });
        }
      },
    });
    api = new ServerApi(conn);
    dataStore = new ServerDataStore(api);
    live.set(rec.id, { conn, api, data: dataStore });
    await conn.connect();
    if (nodeToken) {
      await provisionSatellite(rec, api, dataStore);
      initialProvisionComplete = true;
      await reloadSatelliteForUpdate(rec.url);
      return;
    }
    // Login-less default: identify as guest (§8). LinkKeys sign-in upgrades later.
    try {
      const session = await api.session.whoami();
      patch(rec.id, { session });
      if (rec.token) await syncMediaSession(rec.url, rec.token).catch(() => undefined);
    } catch (e) {
      // whoami may not be reachable on a bare server; stay a nameless guest.
      console.debug("[servers] whoami failed", e);
    }
  }

  async function addServer(name: string, url: string): Promise<string> {
    const id = randomId();
    const rec: ServerRecord = { id, name: name.trim() || url, url: url.trim(), state: "connecting" };
    setServers(produce((list) => list.push(rec)));
    savePersisted(servers);
    if (!activeId()) setActiveId(id);
    try {
      await openConnection(rec);
    } catch (e) {
      patch(id, { state: "error", detail: String(e) });
    }
    return id;
  }

  function removeServer(id: string): void {
    live.get(id)?.data.dispose();
    live.get(id)?.conn.close("removed by user");
    live.delete(id);
    setServers((list) => list.filter((s) => s.id !== id));
    savePersisted(servers);
    if (activeId() === id) setActiveId(servers[0]?.id);
  }

  function setActive(id: string): void {
    setActiveId(id);
  }

  function apiFor(id: string): ServerApi | undefined {
    return live.get(id)?.api;
  }

  function dataFor(id: string): ServerDataStore | undefined {
    return live.get(id)?.data;
  }

  async function reconnect(id: string): Promise<void> {
    const rec = servers.find((s) => s.id === id);
    if (!rec) return;
    live.get(id)?.data.dispose();
    live.get(id)?.conn.close("reconnecting");
    live.delete(id);
    await openConnection(rec);
  }

  async function completeLinkkeysExchange(code: string): Promise<void> {
    const id = activeId();
    if (!id) throw new Error("no active server");
    const liveConn = live.get(id);
    if (!liveConn) throw new Error("active server is not connected");
    const session = await liveConn.api.session.authenticate({ linkkeys_exchange_code: code });
    if (!session.token) throw new Error("server did not return a session token");
    patch(id, { session, token: session.token });
    savePersisted(servers);
    await reconnect(id);
  }

  async function signOut(): Promise<void> {
    const id = activeId();
    if (!id) return;
    try {
      await live.get(id)?.api.session.logout();
    } catch {
      /* clearing the local credential still completes the explicit sign-out */
    }
    const record = servers.find((server) => server.id === id);
    if (record) {
      await syncMediaSession(record.url).catch(() => undefined);
    }
    patch(id, { token: undefined, session: undefined });
    savePersisted(servers);
    await reconnect(id);
  }

  // Restore persisted servers on boot and auto-connect them.
  const satellite = satelliteToken();
  const persisted = satellite ? [] : loadPersisted();
  const seed: ServerRecord[] =
    satellite
      ? [{ id: randomId(), name: "This satellite", url: defaultServerUrl(), state: "idle" }]
      : persisted.length > 0
      ? persisted.map((p) => ({ ...p, state: "idle" as ConnState }))
      : [{ id: randomId(), name: "This server", url: defaultServerUrl(), state: "idle" }];
  setServers(seed);
  setActiveId(seed[0]?.id);
  const exchangeCode =
    typeof location === "undefined"
      ? null
      : new URLSearchParams(location.hash.replace(/^#/, "")).get("linkkeys_exchange");
  const exchangeServer = exchangeCode ? seed.find((server) => isThisOriginServer(server.url)) : undefined;
  if (exchangeServer) setActiveId(exchangeServer.id);
  for (const rec of seed) {
    const opening = openConnection(rec).catch((e) => {
      patch(rec.id, { state: "error", detail: String(e) });
      throw e;
    });
    if (exchangeCode && rec.id === exchangeServer?.id) {
      void opening
        .then(() => completeLinkkeysExchange(exchangeCode))
        .then(() => history.replaceState(null, "", `${location.pathname}${location.search}`))
        .catch((e) => patch(rec.id, { state: "error", detail: String(e) }));
    } else {
      void opening;
    }
  }

  onCleanup(() => {
    for (const { conn, data: dataStore } of live.values()) {
      dataStore.dispose();
      conn.close("app closing");
    }
    live.clear();
  });

  const value: ServersContextValue = {
    servers,
    activeId,
    active,
    api,
    data,
    addServer,
    removeServer,
    setActive,
    apiFor,
    dataFor,
    reconnect,
    completeLinkkeysExchange,
    signOut,
  };

  return <ServersContext.Provider value={value}>{props.children}</ServersContext.Provider>;
}

export function useServers(): ServersContextValue {
  const ctx = useContext(ServersContext);
  if (!ctx) throw new Error("useServers must be used within <ServersProvider>");
  return ctx;
}
