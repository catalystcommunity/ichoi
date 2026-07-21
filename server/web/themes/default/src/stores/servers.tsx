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

export interface ServerRecord {
  id: string;
  name: string;
  url: string;
  state: ConnState;
  detail?: string;
  session?: SessionInfo;
  token?: string;
}

interface LiveConn {
  conn: CsilConnection;
  api: ServerApi;
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
  addServer: (name: string, url: string) => Promise<string>;
  removeServer: (id: string) => void;
  setActive: (id: string) => void;
  apiFor: (id: string) => ServerApi | undefined;
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
  const active = () => servers.find((s) => s.id === activeId());

  function patch(id: string, patchObj: Partial<ServerRecord>): void {
    setServers(
      produce((list) => {
        const rec = list.find((s) => s.id === id);
        if (rec) Object.assign(rec, patchObj);
      }),
    );
  }

  async function openConnection(rec: ServerRecord): Promise<void> {
    const conn = new CsilConnection({
      url: rec.url,
      auth: rec.token,
      onState: (state, detail) => patch(rec.id, { state, detail }),
    });
    const api = new ServerApi(conn);
    live.set(rec.id, { conn, api });
    await conn.connect();
    // Login-less default: identify as guest (§8). LinkKeys sign-in upgrades later.
    try {
      const session = await api.session.whoami();
      patch(rec.id, { session });
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

  async function reconnect(id: string): Promise<void> {
    const rec = servers.find((s) => s.id === id);
    if (!rec) return;
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
    patch(id, { token: undefined, session: undefined });
    savePersisted(servers);
    await reconnect(id);
  }

  // Restore persisted servers on boot and auto-connect them.
  const persisted = loadPersisted();
  const seed: ServerRecord[] =
    persisted.length > 0
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
    for (const { conn } of live.values()) conn.close("app closing");
    live.clear();
  });

  const value: ServersContextValue = {
    servers,
    activeId,
    active,
    api,
    addServer,
    removeServer,
    setActive,
    apiFor,
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
