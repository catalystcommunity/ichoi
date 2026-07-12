// Playback store. Drives either LOCAL playback (this browser's private queue + <audio>) or a
// shared target (§6): when a target is selected, the queue is the target's SERVER queue —
// controllers send commands and the OWNER (the client that shared the device) plays it. The
// server pushes PlayerState over PlayerService.subscribe, so every client stays in sync.

import {
  createContext,
  createEffect,
  createMemo,
  createResource,
  createSignal,
  on,
  onCleanup,
  useContext,
  type Accessor,
  type JSX,
  type ParentProps,
} from "solid-js";
import { createStore } from "solid-js/store";
import { type PlaybackSnapshot } from "../lib/audio/player.ts";
import type {
  Player,
  PlayerCommand,
  PlayerState,
  QueueItem,
  StreamPref,
  Track,
} from "../lib/schema.ts";
import { useServers } from "./servers.tsx";

export const LOCAL_TARGET = "local";

const PREF_KEY = "ichoi.streamPref";
const OWNED_KEY = "ichoi.ownedDevices";

function loadPref(): StreamPref {
  try {
    const raw = localStorage.getItem(PREF_KEY);
    if (raw) return JSON.parse(raw) as StreamPref;
  } catch {
    /* ignore */
  }
  return { transcode_codec: "aac" };
}

function loadOwned(): string[] {
  try {
    return JSON.parse(localStorage.getItem(OWNED_KEY) ?? "[]") as string[];
  } catch {
    return [];
  }
}

type Status = PlaybackSnapshot["status"];
function mapStatus(s: string): Status {
  return s === "playing" ? "playing" : s === "paused" ? "paused" : "idle";
}

function qiToTrack(qi: QueueItem): Track {
  return {
    id: qi.track_id,
    title: qi.title ?? "",
    duration_ms: qi.duration_ms ?? 0,
    codec: "mp3",
    sample_rate: 0,
    channels: 0,
    root_relative_path: "",
  } as Track;
}

interface PlaybackContextValue {
  snapshot: Accessor<PlaybackSnapshot>;
  queue: Track[];
  currentIndex: Accessor<number>;
  current: Accessor<Track | undefined>;
  pref: Accessor<StreamPref>;
  setPref: (p: StreamPref) => void;
  playNow: (tracks: Track[], startIndex?: number) => Promise<void>;
  enqueue: (tracks: Track[]) => void;
  playIndex: (index: number) => Promise<void>;
  togglePlay: () => void;
  next: () => Promise<void>;
  previous: () => Promise<void>;
  seek: (ms: number) => void;
  stop: () => void;
  removeAt: (index: number) => void;
  move: (from: number, to: number) => void;
  /** Output target: `LOCAL_TARGET` (this browser) or a shared player id. */
  target: Accessor<string>;
  setTarget: (id: string) => void;
  /** Shared targets you can send playback to. */
  sharedTargets: Accessor<Player[]>;
  /** Shared-device ids this browser is the output (speaker) for. */
  owned: Accessor<string[]>;
  /** Remember that this client owns (is the output for) a shared device. */
  markOwned: (id: string) => void;
  /** Claim this browser as a device's output — a user gesture, so it also unlocks mobile
   * audio. Marks it owned, re-asserts the share (server presence), and targets it. */
  claimOutput: (id: string) => Promise<void>;
  /** Stop being a device's output and remove the share entirely. */
  releaseDevice: (id: string) => Promise<void>;
}

const PlaybackContext = createContext<PlaybackContextValue>();

export function PlaybackProvider(props: ParentProps): JSX.Element {
  const servers = useServers();
  const [queue, setQueue] = createStore<Track[]>([]);
  const [currentIndex, setCurrentIndex] = createSignal(-1);
  const [snapshot, setSnapshot] = createSignal<PlaybackSnapshot>({
    status: "idle",
    positionMs: 0,
    decoderMissing: false,
  });
  const [pref, setPrefSignal] = createSignal<StreamPref>(loadPref());
  const [owned, setOwned] = createSignal<string[]>(loadOwned());
  const [target, setTargetSignal] = createSignal<string>(LOCAL_TARGET);

  const isOwned = (id: string) => owned().includes(id);

  function persistOwned(next: string[]): void {
    try {
      localStorage.setItem(OWNED_KEY, JSON.stringify(next));
    } catch {
      /* ignore */
    }
  }

  function markOwned(id: string): void {
    setOwned((o) => {
      const next = o.includes(id) ? o : [...o, id];
      persistOwned(next);
      return next;
    });
  }

  function unmarkOwned(id: string): void {
    setOwned((o) => {
      const next = o.filter((x) => x !== id);
      persistOwned(next);
      return next;
    });
  }

  // A shared id is `share:<owner>:<suffix>`; enable-share re-claims by suffix.
  function suffixOf(id: string): string {
    const parts = id.split(":");
    return parts.length > 2 ? parts.slice(2).join(":") : "";
  }

  // --- Audio engine (HTTP /media + native <audio>; §5 bridge) ---------------
  const audio = typeof Audio !== "undefined" ? new Audio() : undefined;

  function mediaBase(): string | undefined {
    const url = servers.active()?.url;
    if (!url) return undefined;
    return url.replace(/^ws/, "http").replace(/\/ws$/, "");
  }

  function mediaUrl(id: string): string | undefined {
    const base = mediaBase();
    if (!base) return undefined;
    const p = pref();
    const params = new URLSearchParams();
    if (p.max_bitrate_kbps) params.set("bitrate", String(p.max_bitrate_kbps));
    if (p.transcode_codec) params.set("format", p.transcode_codec);
    const qs = params.toString();
    return `${base}/media/${encodeURIComponent(id)}${qs ? `?${qs}` : ""}`;
  }

  if (audio) {
    audio.addEventListener("timeupdate", () =>
      setSnapshot((s): PlaybackSnapshot => ({ ...s, positionMs: audio.currentTime * 1000 })),
    );
    audio.addEventListener("loadedmetadata", () =>
      setSnapshot((s): PlaybackSnapshot => ({
        ...s,
        durationMs: Number.isFinite(audio.duration) ? audio.duration * 1000 : undefined,
      })),
    );
    audio.addEventListener("play", () => setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "playing" })));
    audio.addEventListener("pause", () =>
      setSnapshot((s): PlaybackSnapshot => (s.status === "ended" ? s : { ...s, status: "paused" })),
    );
    audio.addEventListener("ended", () => setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "ended" })));
    audio.addEventListener("error", () =>
      setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "error", error: "playback error" })),
    );
  }

  // --- Shared targets -------------------------------------------------------
  const [playersRes, { refetch: refetchPlayers }] = createResource(servers.activeId, async () => {
    const a = servers.api();
    if (!a) return [] as Player[];
    try {
      const r = await a.player.listPlayers();
      return r.players.filter((p) => p.kind === "shared");
    } catch {
      return [] as Player[];
    }
  });
  // Stable reference across polls when the id set is unchanged, so the <select> options don't
  // churn (which would drop the current selection).
  const sharedTargets = createMemo<Player[]>((prev) => {
    const players = playersRes() ?? [];
    const ids = players.map((p) => p.id).join("");
    const prevIds = (prev ?? []).map((p) => p.id).join("");
    return ids === prevIds ? (prev ?? []) : players;
  }, []);
  const playersPoll = setInterval(() => void refetchPlayers(), 4000);
  onCleanup(() => clearInterval(playersPoll));

  // Auto-output a device this client owns, once, when it first appears (so the phone that
  // shared "TodPhone" resumes playing its queue on load).
  let autoSelected = false;
  createEffect(() => {
    if (autoSelected) return;
    const ids = sharedTargets().map((p) => p.id);
    const mine = owned().find((id) => ids.includes(id));
    if (mine) {
      autoSelected = true;
      setTarget(mine);
    }
  });

  // --- Remote (shared-target) state ----------------------------------------
  let ownerTrackId: string | undefined;

  function driveOwnerAudio(state: PlayerState): void {
    if (!audio) return;
    const idx = state.current_index ?? -1;
    const item = idx >= 0 && idx < state.queue.length ? state.queue[idx] : undefined;
    if (state.status === "playing" && item) {
      if (item.track_id !== ownerTrackId) {
        ownerTrackId = item.track_id;
        const url = mediaUrl(item.track_id);
        if (url) {
          audio.src = url;
          void audio.play().catch(() => undefined);
        }
      } else if (audio.paused) {
        void audio.play().catch(() => undefined);
      }
    } else if (state.status === "paused") {
      audio.pause();
    } else {
      audio.pause();
      ownerTrackId = undefined;
    }
  }

  function applyRemote(t: string, state: PlayerState): void {
    // One channel fans out every subscribed player's pushes; ignore states for other players
    // (a stale server-side subscription from a previous target can still deliver here).
    if (state.player_id !== t || target() !== t) return;
    setQueue(state.queue.map(qiToTrack));
    setCurrentIndex(state.current_index ?? -1);
    const idx = state.current_index ?? -1;
    const cur = idx >= 0 && idx < state.queue.length ? state.queue[idx] : undefined;
    if (isOwned(t)) {
      driveOwnerAudio(state);
      // Status/position come from the local <audio> (we are the output); take duration + queue.
      setSnapshot((s): PlaybackSnapshot => ({ ...s, durationMs: cur?.duration_ms }));
    } else {
      setSnapshot({
        status: mapStatus(state.status),
        positionMs: state.position_ms ?? 0,
        durationMs: cur?.duration_ms,
        decoderMissing: false,
      });
    }
  }

  // Subscribe to the active shared target for live PlayerState pushes. Re-runs when the target
  // OR the connection readiness changes, so it re-subscribes after a reconnect (the server
  // forgets subscriptions when the socket drops). Only sends once the socket is actually ready.
  let unsub: (() => void) | undefined;
  createEffect(
    on([target, () => servers.active()?.state], () => {
      unsub?.();
      unsub = undefined;
      const t = target();
      if (t === LOCAL_TARGET) return;
      if (servers.active()?.state !== "ready") return;
      const a = servers.api();
      if (!a) return;
      try {
        unsub = a.player.subscribe({ player_id: t }, (state) => applyRemote(t, state));
      } catch (e) {
        console.warn("[playback] subscribe failed", e);
      }
    }),
  );
  onCleanup(() => unsub?.());

  function control(command: PlayerCommand): void {
    const t = target();
    if (t === LOCAL_TARGET) return;
    const a = servers.api();
    if (!a) return;
    void a.player
      .control({ player_id: t, command })
      .then((st) => applyRemote(t, st))
      .catch((e) => console.warn("[playback] control failed", e));
  }

  // Re-assert output ownership on every (re)connect: the server forgets device presence when a
  // socket drops, so a client re-shares the devices it owns to make them live again and resume
  // driving their audio. This is what "reconciles devices with what's actually connected".
  createEffect(
    on(
      () => servers.active()?.state,
      (state) => {
        if (state !== "ready") return;
        const a = servers.api();
        if (!a) return;
        for (const id of owned()) {
          const suffix = suffixOf(id);
          if (suffix) void a.player.enableShare({ suffix }).catch(() => undefined);
        }
      },
    ),
  );

  async function claimOutput(id: string): Promise<void> {
    markOwned(id);
    setTarget(id);
    const a = servers.api();
    const suffix = suffixOf(id);
    if (a && suffix) {
      try {
        await a.player.enableShare({ suffix });
      } catch (e) {
        console.warn("[playback] claim output failed", e);
      }
    }
  }

  async function releaseDevice(id: string): Promise<void> {
    const a = servers.api();
    if (a) {
      try {
        await a.player.disableShare({ player_id: id });
      } catch (e) {
        console.warn("[playback] release device failed", e);
      }
    }
    unmarkOwned(id);
    if (target() === id) setTarget(LOCAL_TARGET);
  }

  // --- Target switching -----------------------------------------------------
  let savedLocal: { tracks: Track[]; index: number } | undefined;

  function setTarget(id: string): void {
    const prev = target();
    if (prev === LOCAL_TARGET && id !== LOCAL_TARGET) {
      savedLocal = { tracks: queue.slice(), index: currentIndex() };
      audio?.pause();
      ownerTrackId = undefined;
    }
    setTargetSignal(id);
    if (id === LOCAL_TARGET) {
      ownerTrackId = undefined;
      if (savedLocal) {
        setQueue(savedLocal.tracks);
        setCurrentIndex(savedLocal.index);
      }
      setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "paused" }));
    }
  }

  // --- Auto-advance ---------------------------------------------------------
  createEffect(
    on(
      () => snapshot().status,
      (status, prevStatus) => {
        if (status === "ended" && prevStatus !== "ended") void next();
      },
    ),
  );

  const current = () => {
    const i = currentIndex();
    return i >= 0 && i < queue.length ? queue[i] : undefined;
  };

  const setPref = (p: StreamPref) => {
    setPrefSignal(p);
    try {
      localStorage.setItem(PREF_KEY, JSON.stringify(p));
    } catch {
      /* ignore */
    }
  };

  // --- Local playback -------------------------------------------------------
  async function openIndex(index: number): Promise<void> {
    const track = queue[index];
    if (!track || !audio) return;
    setCurrentIndex(index);
    const url = mediaUrl(track.id);
    if (!url) return;
    audio.src = url;
    try {
      await audio.play();
    } catch (e) {
      setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "error", error: String(e) }));
    }
  }

  const isLocal = () => target() === LOCAL_TARGET;

  async function playNow(tracks: Track[], startIndex = 0): Promise<void> {
    if (isLocal()) {
      setQueue(tracks.slice());
      if (tracks.length) await openIndex(startIndex);
    } else {
      control({ op: "clear" });
      control({ op: "enqueue", track_ids: tracks.map((t) => t.id) });
      control({ op: "play", index: startIndex });
    }
  }

  function enqueue(tracks: Track[]): void {
    if (isLocal()) {
      setQueue((q) => [...q, ...tracks]);
      if (currentIndex() < 0 && tracks.length) void openIndex(0);
    } else {
      control({ op: "enqueue", track_ids: tracks.map((t) => t.id) });
    }
  }

  async function playIndex(index: number): Promise<void> {
    if (isLocal()) await openIndex(index);
    else control({ op: "play", index });
  }

  function togglePlay(): void {
    if (!isLocal()) {
      control(snapshot().status === "playing" ? { op: "pause" } : { op: "play" });
      return;
    }
    if (!audio) return;
    if (!audio.paused) audio.pause();
    else if (audio.currentSrc) void audio.play();
    else if (current()) void openIndex(currentIndex());
  }

  async function next(): Promise<void> {
    if (!isLocal()) {
      control({ op: "next" });
      return;
    }
    const i = currentIndex();
    if (i + 1 < queue.length) await openIndex(i + 1);
    else stop();
  }

  async function previous(): Promise<void> {
    if (!isLocal()) {
      control({ op: "previous" });
      return;
    }
    if (snapshot().positionMs > 3000 && audio) {
      audio.currentTime = 0;
      return;
    }
    const i = currentIndex();
    if (i > 0) await openIndex(i - 1);
    else if (audio) audio.currentTime = 0;
  }

  function seek(ms: number): void {
    if (isLocal()) {
      if (audio) audio.currentTime = ms / 1000;
    } else {
      if (audio && isOwned(target())) audio.currentTime = ms / 1000;
      control({ op: "seek", position_ms: ms });
    }
  }

  function stop(): void {
    if (!isLocal()) {
      control({ op: "clear" });
      return;
    }
    if (audio) {
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    }
    setCurrentIndex(-1);
    setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "idle", positionMs: 0 }));
  }

  function removeAt(index: number): void {
    if (!isLocal()) {
      control({ op: "remove", index });
      return;
    }
    if (index < 0 || index >= queue.length) return;
    const cur = currentIndex();
    const currentId = current()?.id;
    const wasCurrent = index === cur;
    setQueue((q) => q.filter((_, i) => i !== index));
    if (wasCurrent) {
      if (queue.length === 0) stop();
      else void openIndex(Math.min(index, queue.length - 1));
    } else if (currentId) {
      const ni = queue.findIndex((tr) => tr.id === currentId);
      if (ni >= 0) setCurrentIndex(ni);
    }
  }

  function move(from: number, to: number): void {
    if (!isLocal()) {
      control({ op: "reorder", from_index: from, to_index: to });
      return;
    }
    if (from === to || from < 0 || from >= queue.length || to < 0 || to >= queue.length) return;
    const currentId = current()?.id;
    setQueue((q) => {
      const arr = [...q];
      const [item] = arr.splice(from, 1);
      if (item) arr.splice(to, 0, item);
      return arr;
    });
    if (currentId) {
      const ni = queue.findIndex((tr) => tr.id === currentId);
      if (ni >= 0) setCurrentIndex(ni);
    }
  }

  onCleanup(() => {
    audio?.pause();
  });

  const value: PlaybackContextValue = {
    snapshot,
    queue,
    currentIndex,
    current,
    pref,
    setPref,
    playNow,
    enqueue,
    playIndex,
    togglePlay,
    next,
    previous,
    seek,
    stop,
    removeAt,
    move,
    target,
    setTarget,
    sharedTargets,
    owned,
    markOwned,
    claimOutput,
    releaseDevice,
  };

  return <PlaybackContext.Provider value={value}>{props.children}</PlaybackContext.Provider>;
}

export function usePlayback(): PlaybackContextValue {
  const ctx = useContext(PlaybackContext);
  if (!ctx) throw new Error("usePlayback must be used within <PlaybackProvider>");
  return ctx;
}
