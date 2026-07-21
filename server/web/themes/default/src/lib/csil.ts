// CSIL-Events client for Ichoi's browser carrier (`/ws` on HTTP :4042).
//
// Wire model (see csilgen `docs/csil-events-transport.md` and the reference
// `transports/typescript/src/events.ts`): a single persistent WebSocket carries
// typed events in both directions. Each event is ONE binary WS frame whose bytes
// are the canonical-CBOR *envelope*. We use the **verbose profile** (the default;
// the compact profile needs `@wire-id` ordinals the schema does not yet pin —
// DESIGN §16.1). The connection is **multi-service**, so every envelope names its
// `service`.
//
// ┌──────────────────────── THE WIRE SEAM ────────────────────────┐
// │ `encodeEnvelope` / `decodeEnvelope` below are the ONLY place    │
// │ that knows the CSIL-Events framing. They are verified against   │
// │ csilgen's published conformance vectors. If the transport spec  │
// │ changes, this is the single file to touch.                      │
// └────────────────────────────────────────────────────────────────┘

import { CborTag, decode as cborDecode, encode as cborEncode } from "./cbor.ts";
import type { CborValue } from "./cbor.ts";
import type { ServiceError } from "./schema.ts";

const TRANSPORT_VERSION = 1;
const TAG_ENCODED_CBOR = 24;

/** A decoded inbound envelope (verbose profile). */
export interface Envelope {
  service?: string;
  event: string;
  id?: number;
  /** The opaque tag-24 payload bytes (CBOR of the event's record type). */
  payload: Uint8Array;
}

// --- The seam: envelope framing (verbose profile) ------------------------

/** Frame one event as a binary WS payload: a canonical-CBOR map
 * `{ event, payload: 24(bstr), service?, id? }`. Canonical key ordering is
 * handled by the CBOR encoder, so this is byte-exact with the reference. */
export function encodeEnvelope(env: Envelope): Uint8Array {
  const map: { [k: string]: CborValue } = {
    event: env.event,
    payload: new CborTag(TAG_ENCODED_CBOR, env.payload),
  };
  if (env.service !== undefined) map.service = env.service;
  if (env.id !== undefined) map.id = env.id;
  return cborEncode(map);
}

/** Parse one binary WS payload back into an envelope. */
export function decodeEnvelope(bytes: Uint8Array): Envelope {
  const v = cborDecode(bytes);
  if (typeof v !== "object" || v === null || Array.isArray(v) || v instanceof Uint8Array) {
    throw new Error("csil: envelope is not a map");
  }
  const rec = v as Record<string, unknown>;
  const event = rec.event;
  if (typeof event !== "string") throw new Error("csil: envelope missing 'event'");
  const payloadField = rec.payload;
  if (!(payloadField instanceof CborTag) || payloadField.tag !== TAG_ENCODED_CBOR) {
    throw new Error("csil: envelope 'payload' is not tag-24");
  }
  if (!(payloadField.value instanceof Uint8Array)) {
    throw new Error("csil: tag-24 payload is not a byte string");
  }
  const env: Envelope = { event, payload: payloadField.value };
  if (typeof rec.service === "string") env.service = rec.service;
  if (typeof rec.id === "number") env.id = rec.id;
  return env;
}

// --- Control plane (service ordinal 0, verbose `$`-prefixed names) --------

export const CONTROL = {
  HELLO: "$hello",
  HELLO_ACK: "$hello-ack",
  PING: "$ping",
  PONG: "$pong",
  CLOSE: "$close",
  ERROR: "$error",
} as const;

/** A ServiceError surfaced from a correlated reply. */
export class CsilServiceError extends Error {
  constructor(
    readonly code: number,
    message: string,
  ) {
    super(message);
    this.name = "CsilServiceError";
  }
}

/** A structural check for the `ServiceError` arm of an operation's response.
 *
 * ASSUMPTION the Rust side must honor: a correlated reply carries EITHER the
 * success record OR a `ServiceError` (`{ code:int, message:text }`) in the same
 * `payload` slot, with the same `id`. Events has no per-frame status, so we
 * discriminate structurally: a two-field map of exactly `{code:number,
 * message:string}` is the error arm. No success record in `schema/` has that
 * shape, so this is unambiguous. */
function isServiceError(obj: unknown): obj is ServiceError {
  if (typeof obj !== "object" || obj === null) return false;
  const keys = Object.keys(obj);
  const o = obj as Record<string, unknown>;
  return (
    keys.length === 2 &&
    typeof o.code === "number" &&
    typeof o.message === "string"
  );
}

/** Return the CSIL operation name exactly as written in `schema/*.csil`. */
export function wireOp(kebab: string): string {
  return kebab;
}

/** Canonicalize a service name for channel keying. The client names services in full
 * (`PlayerService`) but the server emits the normalized short form (`player`) on pushes;
 * folding both to the same key lets a subscription match its pushed events. */
export function serviceKey(service: string): string {
  return service.replace(/Service$/, "").toLowerCase();
}

// --- Connection ----------------------------------------------------------

export type ConnState = "idle" | "connecting" | "ready" | "closed" | "error";

/** A handler for server-pushed channel events (no correlation id): the bidi
 * `subscribe` / `stream` / `session` streams. */
type ChannelHandler = (payload: Uint8Array) => void;

interface Pending {
  resolve: (payload: Uint8Array) => void;
  reject: (err: unknown) => void;
  timer: ReturnType<typeof setTimeout>;
}

export interface CsilConnectionOptions {
  /** ws:// or wss:// URL of the server's `/ws` carrier. */
  url: string;
  /** Session token minted by `SessionService.authenticate`, sent as `Hello.auth`. */
  auth?: string;
  /** Device credential used only by the restricted satellite PWA. */
  nodeToken?: string;
  /** Per-request timeout (ms) for correlated calls. */
  callTimeoutMs?: number;
  onState?: (state: ConnState, detail?: string) => void;
}

/**
 * A single CSIL-Events connection to one Ichoi server. Owns the handshake, the
 * correlation table for request/response calls, heartbeat replies, and dispatch
 * of server-pushed channel events.
 */
export class CsilConnection {
  private ws?: WebSocket;
  private state: ConnState = "idle";
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();
  // Channel handlers keyed by `${service}:${event}`.
  private readonly channels = new Map<string, Set<ChannelHandler>>();
  private readyWaiters: { resolve: () => void; reject: (e: unknown) => void }[] = [];
  // Reconnect + keepalive: survive a backgrounded phone tab or a dropped LAN link.
  private closedByClient = false;
  private reconnectAttempts = 0;
  private reconnectTimer?: ReturnType<typeof setTimeout>;
  private heartbeatTimer?: ReturnType<typeof setInterval>;
  private readonly opts: Required<Pick<CsilConnectionOptions, "callTimeoutMs">> &
    CsilConnectionOptions;

  constructor(options: CsilConnectionOptions) {
    this.opts = { callTimeoutMs: 15000, ...options };
  }

  get connectionState(): ConnState {
    return this.state;
  }

  /** Open the socket and complete the `$hello` / `$hello-ack` handshake. Resolves
   * once the connection is ready for application events. */
  connect(): Promise<void> {
    if (this.state === "ready") return Promise.resolve();
    this.closedByClient = false;
    this.setState("connecting");
    return new Promise((resolve, reject) => {
      this.readyWaiters.push({ resolve, reject });
      let ws: WebSocket;
      try {
        ws = new WebSocket(this.opts.url);
      } catch (e) {
        this.fail(e);
        return;
      }
      ws.binaryType = "arraybuffer";
      this.ws = ws;

      ws.onopen = () => this.sendHello();
      ws.onmessage = (ev) => this.onMessage(ev);
      ws.onerror = () => this.setState("error", "socket error");
      ws.onclose = (ev) => this.onClose(ev.code, ev.reason);
    });
  }

  /** Orderly shutdown: send `$close` then close the socket. */
  close(reason = "client closing"): void {
    this.closedByClient = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = undefined;
    }
    this.stopHeartbeat();
    if (this.ws && this.state === "ready") {
      try {
        this.sendControl(CONTROL.CLOSE, { status: 0, reason });
      } catch {
        /* best effort */
      }
    }
    this.ws?.close();
    this.teardown("closed", reason);
  }

  // --- Application-facing send paths ---

  /**
   * A correlated request/response call. Encodes `request` bytes, assigns a fresh
   * `id`, sends the event, and resolves with the reply payload bytes (or rejects
   * with a `CsilServiceError` if the reply is the error arm).
   */
  async call<Res>(
    service: string,
    operation: string,
    requestBytes: Uint8Array,
    decodeResponse: (bytes: Uint8Array) => Res,
  ): Promise<Res> {
    await this.ensureReady();
    const id = this.nextId++;
    const event = wireOp(operation);
    const payload = await new Promise<Uint8Array>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`csil: call ${service}.${operation} timed out`));
      }, this.opts.callTimeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.rawSend({ service, event, id, payload: requestBytes });
    });
    // Peek for the ServiceError arm before handing to the typed decoder.
    const decoded = cborDecode(payload);
    if (isServiceError(decoded)) {
      throw new CsilServiceError(decoded.code, decoded.message);
    }
    return decodeResponse(payload);
  }

  /**
   * Send a fire-and-forget channel event (the client->server half of a bidi
   * `<->` operation: a `subscribe` request, a `MediaControl`, a `NodeReport`).
   * No correlation id — replies arrive as pushed channel events.
   */
  sendChannel(service: string, operation: string, payloadBytes: Uint8Array): void {
    this.rawSend({ service, event: wireOp(operation), payload: payloadBytes });
  }

  /**
   * Subscribe to server-pushed events for a bidi channel operation. Returns an
   * unsubscribe function. Multiple subscribers for one channel are fanned out;
   * the caller decodes the payload and demultiplexes (e.g. by `player_id`).
   */
  onChannel(service: string, operation: string, handler: ChannelHandler): () => void {
    const key = `${serviceKey(service)}:${wireOp(operation)}`;
    let set = this.channels.get(key);
    if (!set) {
      set = new Set();
      this.channels.set(key, set);
    }
    set.add(handler);
    return () => set!.delete(handler);
  }

  // --- Internals ---

  private setState(state: ConnState, detail?: string): void {
    this.state = state;
    this.opts.onState?.(state, detail);
  }

  private ensureReady(): Promise<void> {
    if (this.state === "ready") return Promise.resolve();
    if (this.state === "connecting") {
      return new Promise((resolve, reject) => this.readyWaiters.push({ resolve, reject }));
    }
    // Fail fast on a closed/errored connection rather than storming reconnects;
    // the store reconnects explicitly (user retry / server switch).
    return Promise.reject(
      new Error(`csil: connection not ready (state=${this.state})`),
    );
  }

  private sendHello(): void {
    // Multi-service connection: omit `service` in the hello so every event names
    // its own service. Offer verbose only (compact needs @wire-ids).
    const hello: { [k: string]: CborValue } = {
      versions: [TRANSPORT_VERSION],
      profiles: ["verbose"],
    };
    if (this.opts.auth !== undefined) hello.auth = this.opts.auth;
    if (this.opts.nodeToken !== undefined) hello.node_token = this.opts.nodeToken;
    this.sendControl(CONTROL.HELLO, hello);
  }

  private sendControl(event: string, payloadObj: CborValue): void {
    // Control frames omit `service` (implied service ordinal 0).
    this.rawSend({ event, payload: cborEncode(payloadObj) });
  }

  private rawSend(env: Envelope): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error("csil: socket not open");
    }
    this.ws.send(encodeEnvelope(env));
  }

  private onMessage(ev: MessageEvent): void {
    let env: Envelope;
    try {
      const bytes =
        ev.data instanceof ArrayBuffer
          ? new Uint8Array(ev.data)
          : new Uint8Array(0);
      env = decodeEnvelope(bytes);
    } catch (e) {
      console.error("[csil] undecodable frame", e);
      return;
    }

    // Control plane (no service, `$`-prefixed).
    if (env.event.startsWith("$")) {
      this.onControl(env);
      return;
    }

    // Correlated reply?
    if (env.id !== undefined) {
      const p = this.pending.get(env.id);
      if (p) {
        clearTimeout(p.timer);
        this.pending.delete(env.id);
        p.resolve(env.payload);
        return;
      }
      // A reply with no matching id: surface but do not crash.
      console.warn(`[csil] reply with unknown id ${env.id}`);
      return;
    }

    // Fire-and-forget channel event pushed from the server.
    const key = `${serviceKey(env.service ?? "")}:${env.event}`;
    const handlers = this.channels.get(key);
    if (handlers && handlers.size) {
      for (const h of handlers) {
        try {
          h(env.payload);
        } catch (e) {
          console.error(`[csil] channel handler for ${key} threw`, e);
        }
      }
    }
  }

  private onControl(env: Envelope): void {
    switch (env.event) {
      case CONTROL.HELLO_ACK: {
        // Handshake complete. (We offered only verbose, so we accept the ack.)
        this.reconnectAttempts = 0;
        this.startHeartbeat();
        this.setState("ready");
        const waiters = this.readyWaiters;
        this.readyWaiters = [];
        for (const w of waiters) w.resolve();
        break;
      }
      case CONTROL.PING: {
        const nonce = this.readNonce(env.payload);
        this.sendControl(CONTROL.PONG, { nonce, at: Date.now() });
        break;
      }
      case CONTROL.PONG:
        break;
      case CONTROL.CLOSE: {
        const detail = this.readCloseReason(env.payload);
        this.teardown("closed", detail);
        break;
      }
      case CONTROL.ERROR: {
        const detail = this.readCloseReason(env.payload);
        console.error(`[csil] transport error: ${detail}`);
        break;
      }
      default:
        console.warn(`[csil] unknown control event ${env.event}`);
    }
  }

  private readNonce(payload: Uint8Array): number {
    try {
      const v = cborDecode(payload) as Record<string, unknown>;
      return typeof v.nonce === "number" ? v.nonce : 0;
    } catch {
      return 0;
    }
  }

  private readCloseReason(payload: Uint8Array): string {
    try {
      const v = cborDecode(payload) as Record<string, unknown>;
      const status = typeof v.status === "number" ? v.status : "?";
      const reason = typeof v.reason === "string" ? v.reason : "";
      return `status ${status}${reason ? `: ${reason}` : ""}`;
    } catch {
      return "unparseable control payload";
    }
  }

  private onClose(code: number, reason: string): void {
    this.stopHeartbeat();
    this.teardown("closed", `ws close ${code}${reason ? `: ${reason}` : ""}`);
    if (!this.closedByClient) this.scheduleReconnect();
  }

  /** Reconnect after an unexpected drop, with capped exponential backoff. On success the
   * `$hello-ack` flips state back to "ready"; consumers observing `onState` re-subscribe
   * their channels (the server forgets subscriptions when the socket dies). */
  private scheduleReconnect(): void {
    if (this.closedByClient || this.reconnectTimer) return;
    const delay = Math.min(8000, 500 * 2 ** this.reconnectAttempts);
    this.reconnectAttempts += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = undefined;
      if (this.closedByClient) return;
      this.connect().catch(() => {
        /* onClose will schedule the next attempt */
      });
    }, delay);
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    // App-level ping keeps intermediaries from idling out the socket and lets us notice a
    // half-open connection. The server also pings at the WS layer (auto-ponged by the browser).
    this.heartbeatTimer = setInterval(() => {
      if (this.state !== "ready") return;
      try {
        this.sendControl(CONTROL.PING, { nonce: this.nextId++, at: Date.now() });
      } catch {
        /* socket wobble; onClose handles recovery */
      }
    }, 25000);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = undefined;
    }
  }

  private fail(e: unknown): void {
    this.setState("error", String(e));
    const waiters = this.readyWaiters;
    this.readyWaiters = [];
    for (const w of waiters) w.reject(e);
  }

  private teardown(state: ConnState, detail?: string): void {
    if (this.state === state && state === "closed") return;
    this.setState(state, detail);
    const err = new Error(`csil: connection ${state}${detail ? ` (${detail})` : ""}`);
    for (const [, p] of this.pending) {
      clearTimeout(p.timer);
      p.reject(err);
    }
    this.pending.clear();
    const waiters = this.readyWaiters;
    this.readyWaiters = [];
    for (const w of waiters) w.reject(err);
  }
}
