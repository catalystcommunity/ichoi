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
import { useToast } from "./toasts.tsx";
import { satelliteOutput, satelliteToken } from "../lib/satellite-mode.ts";
import { finishUpdateReload, updateReloadInProgress } from "../lib/app-update.ts";
import { onFirstGesture, probeAutoplay } from "../lib/audio-unlock.ts";
import { planVolumeChange } from "../lib/volume.ts";
import { parseOwnedTargetStore, resolveOutputTarget } from "../lib/output-target.ts";

export const LOCAL_TARGET = "local";
export type RepeatMode = "off" | "all" | "one";

const PREF_KEY = "ichoi.streamPref";
const OWNED_KEY = "ichoi.ownedDevices";
const REPEAT_KEY = "ichoi.repeatMode";
const SHUFFLE_KEY = "ichoi.shuffle";
const VOLUME_KEY = "ichoi.localVolume";

function loadPref(): StreamPref {
  try {
    const raw = localStorage.getItem(PREF_KEY);
    if (raw) return JSON.parse(raw) as StreamPref;
  } catch {
    /* ignore */
  }
  return { transcode_codec: "aac" };
}

function loadOwned() {
  try {
    return parseOwnedTargetStore(localStorage.getItem(OWNED_KEY));
  } catch {
    return { servers: {}, legacy: [] };
  }
}

function loadRepeatMode(): RepeatMode {
  try {
    const value = localStorage.getItem(REPEAT_KEY);
    return value === "all" || value === "one" ? value : "off";
  } catch {
    return "off";
  }
}

function loadShuffle(): boolean {
  try {
    return localStorage.getItem(SHUFFLE_KEY) === "true";
  } catch {
    return false;
  }
}

function loadVolume(): number {
  try {
    return planVolumeChange(LOCAL_TARGET, Number(localStorage.getItem(VOLUME_KEY) ?? 100)).volume;
  } catch {
    return 100;
  }
}

type Status = PlaybackSnapshot["status"];
function mapStatus(s: string): Status {
  return s === "playing" ? "playing" : s === "paused" ? "paused" : "idle";
}

function qiToTrack(qi: QueueItem): Track {
  return {
    id: qi.track_id,
    library: qi.library ?? "music",
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
  playNow: (tracks: Track[], startIndex?: number, startMs?: number) => Promise<void>;
  enqueue: (tracks: Track[]) => void;
  enqueueAndPlay: (track: Track, startMs?: number) => Promise<void>;
  playIndex: (index: number) => Promise<void>;
  togglePlay: () => void;
  next: () => Promise<void>;
  previous: () => Promise<void>;
  seek: (ms: number) => void;
  volume: Accessor<number>;
  setVolume: (volume: number) => void;
  stop: () => void;
  removeAt: (index: number) => void;
  move: (from: number, to: number) => void;
  saveQueueAsPlaylist: (name: string) => Promise<void>;
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
  /** Satisfy browser autoplay policy and bind the selected satellite audio sink. */
  enableOutputAudio: () => Promise<void>;
  outputAudioReady: Accessor<boolean>;
  /** Satellite mode: this browser is connected but cannot make sound until somebody touches
   * it. Reported to the server so controllers can say so before sending music here. */
  audioBlocked: Accessor<boolean>;
  repeatMode: Accessor<RepeatMode>;
  cycleRepeatMode: () => void;
  shuffle: Accessor<boolean>;
  toggleShuffle: () => void;
}

const PlaybackContext = createContext<PlaybackContextValue>();

export function PlaybackProvider(props: ParentProps): JSX.Element {
  const servers = useServers();
  const toast = useToast();
  const satelliteMode = Boolean(satelliteToken());
  const [queue, setQueue] = createStore<Track[]>([]);
  const [currentIndex, setCurrentIndex] = createSignal(-1);
  const [snapshot, setSnapshot] = createSignal<PlaybackSnapshot>({
    status: "idle",
    positionMs: 0,
    decoderMissing: false,
  });
  const [pref, setPrefSignal] = createSignal<StreamPref>(loadPref());
  const loadedOwned = loadOwned();
  const [ownedByServer, setOwnedByServer] = createSignal<Record<string, string[]>>(loadedOwned.servers);
  const [legacyOwned, setLegacyOwned] = createSignal(loadedOwned.legacy);
  const [target, setTargetSignal] = createSignal<string>(
    satelliteMode ? "satellite-pending" : LOCAL_TARGET,
  );
  const [outputAudioReady, setOutputAudioReady] = createSignal(false);
  const [audioBlocked, setAudioBlocked] = createSignal(false);
  const [repeatMode, setRepeatMode] = createSignal<RepeatMode>(loadRepeatMode());
  const [shuffle, setShuffle] = createSignal(loadShuffle());
  const [localVolume, setLocalVolume] = createSignal(loadVolume());
  const [remoteVolume, setRemoteVolume] = createSignal(100);
  let lastProgressTrack = "";
  let lastProgressPosition = -1;
  let lastSatelliteReportSecond = -1;

  const satellitePlayerId = () => servers.active()?.satellitePlayerId;
  const owned = () => ownedByServer()[servers.activeId() ?? ""] ?? [];
  const isLocal = () => !satelliteMode && target() === LOCAL_TARGET;
  const volume = () => isLocal() ? localVolume() : remoteVolume();

  function reportSatellite(status: "stopped" | "playing" | "paused", positionMs?: number): void {
    const playerId = satellitePlayerId();
    const api = servers.api();
    if (!satelliteMode || !playerId || !api) return;
    // Every report carries the current sound state, so the server's view stays right without
    // a separate channel — and a satellite that was blocked corrects itself the moment it
    // plays anything.
    api.node.report({
      player_id: playerId,
      status,
      position_ms: positionMs,
      audio_blocked: audioBlocked(),
    });
  }

  /** Push the blocked flag out of band, when it changed without playback changing. */
  function reportAudioBlocked(): void {
    const status = snapshot().status;
    reportSatellite(
      status === "playing" ? "playing" : status === "paused" ? "paused" : "stopped",
      Math.round(snapshot().positionMs),
    );
  }

  function setBlocked(blocked: boolean): void {
    if (audioBlocked() === blocked) return;
    setAudioBlocked(blocked);
    reportAudioBlocked();
  }

  function reportAudiobookProgress(completed = false): void {
    const track = current();
    const session = servers.active()?.session;
    const api = servers.api();
    if (!track || track.library !== "audiobook" || !session || !api) {
      return;
    }
    const position = completed ? track.duration_ms : Math.max(0, Math.round(snapshot().positionMs));
    if (
      !completed &&
      track.id === lastProgressTrack &&
      Math.abs(position - lastProgressPosition) < 10_000
    ) {
      return;
    }
    lastProgressTrack = track.id;
    lastProgressPosition = position;
    void api.library
      .updateAudiobookProgress({
        track_id: track.id,
        position_ms: position,
        completed,
      })
      .catch((e) => console.warn("[playback] audiobook progress failed", e));
  }

  const isOwned = (id: string) =>
    owned().includes(id) || (satelliteMode && id === satellitePlayerId());

  function persistOwned(next: Record<string, string[]>): void {
    try {
      localStorage.setItem(OWNED_KEY, JSON.stringify({ version: 2, servers: next }));
    } catch {
      /* ignore */
    }
  }

  function markOwned(id: string): void {
    const serverId = servers.activeId();
    if (!serverId) return;
    setOwnedByServer((current) => {
      const ids = current[serverId] ?? [];
      const next = ids.includes(id) ? current : { ...current, [serverId]: [...ids, id] };
      persistOwned(next);
      return next;
    });
  }

  function unmarkOwned(id: string): void {
    const serverId = servers.activeId();
    if (!serverId) return;
    setOwnedByServer((current) => {
      const ids = current[serverId] ?? [];
      const next = { ...current, [serverId]: ids.filter((value) => value !== id) };
      persistOwned(next);
      return next;
    });
  }

  // Older versions stored one global list. Assign it once to the first active server so it
  // cannot claim matching player IDs on every connected instance.
  createEffect(() => {
    const serverId = servers.activeId();
    const legacy = legacyOwned();
    if (!serverId || legacy.length === 0) return;
    setOwnedByServer((current) => {
      const existing = current[serverId] ?? [];
      const next = { ...current, [serverId]: [...new Set([...existing, ...legacy])] };
      persistOwned(next);
      return next;
    });
    setLegacyOwned([]);
  });

  // A shared id is `share:<owner>:<suffix>`; enable-share re-claims by suffix.
  function suffixOf(id: string): string {
    const parts = id.split(":");
    return parts.length > 2 ? parts.slice(2).join(":") : "";
  }

  // --- Audio engine (HTTP /media + native <audio>; §5 bridge) ---------------
  const audio = typeof Audio !== "undefined" ? new Audio() : undefined;
  if (audio) audio.volume = localVolume() / 100;

  async function applySatelliteSink(): Promise<void> {
    if (!satelliteMode || !audio || !("setSinkId" in audio)) return;
    await audio.setSinkId(satelliteOutput().id);
  }

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
    audio.addEventListener("timeupdate", () => {
      setSnapshot((s): PlaybackSnapshot => ({ ...s, positionMs: audio.currentTime * 1000 }));
      reportAudiobookProgress();
      const second = Math.floor(audio.currentTime);
      if (second !== lastSatelliteReportSecond) {
        lastSatelliteReportSecond = second;
        reportSatellite("playing", second * 1000);
      }
    });
    audio.addEventListener("loadedmetadata", () =>
      setSnapshot((s): PlaybackSnapshot => ({
        ...s,
        durationMs: Number.isFinite(audio.duration) ? audio.duration * 1000 : undefined,
      })),
    );
    audio.addEventListener("play", () => {
      setOutputAudioReady(true);
      finishUpdateReload();
      setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "playing" }));
      reportSatellite("playing", Math.round(audio.currentTime * 1000));
    });
    audio.addEventListener("pause", () => {
      setSnapshot((s): PlaybackSnapshot => (s.status === "ended" ? s : { ...s, status: "paused" }));
      reportAudiobookProgress();
      if (!audio.ended && !updateReloadInProgress()) {
        reportSatellite("paused", Math.round(audio.currentTime * 1000));
      }
    });
    audio.addEventListener("ended", () => {
      setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "ended" }));
      reportAudiobookProgress(true);
      reportSatellite("stopped", 0);
    });
    audio.addEventListener("error", () =>
      setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "error", error: "playback error" })),
    );
  }

  // --- Shared targets -------------------------------------------------------
  const [playersRes, { refetch: refetchPlayers }] = createResource(
    () =>
      `${servers.activeId() ?? ""}:${servers.active()?.state ?? ""}:${
        servers.active()?.satellitePlayerId ?? ""
      }`,
    async () => {
      const a = servers.api();
      if (!a) return [] as Player[];
      try {
        const r = await a.player.listPlayers();
        return r.players.filter((p) => p.kind === "shared");
      } catch {
        return [] as Player[];
      }
    },
  );
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
    if (satelliteMode) {
      const playerId = satellitePlayerId();
      if (playerId && sharedTargets().some((player) => player.id === playerId)) {
        setTarget(playerId);
      }
      return;
    }
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
          const startMs = state.position_ms ?? 0;
          if (startMs > 0) {
            const restorePosition = () => {
              audio.currentTime = startMs / 1000;
              audio.removeEventListener("loadedmetadata", restorePosition);
            };
            audio.addEventListener("loadedmetadata", restorePosition);
          }
          void applySatelliteSink()
            .then(() => audio.play())
            .catch(async (e) => {
              // The server told this output to play and the browser refused. Say why on the
              // satellite, and tell the server so other people see it too.
              setBlocked(!(await probeAutoplay()));
              setSnapshot((snapshot): PlaybackSnapshot => ({
                ...snapshot,
                status: "error",
                error: `Enable browser audio and try again: ${String(e)}`,
              }));
            });
        }
      } else if (audio.paused) {
        const requestedSeconds = (state.position_ms ?? 0) / 1000;
        if (Math.abs(audio.currentTime - requestedSeconds) > 1) {
          audio.currentTime = requestedSeconds;
        }
        void applySatelliteSink().then(() => audio.play()).catch(() => undefined);
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
    setRemoteVolume(state.volume);
    const idx = state.current_index ?? -1;
    const cur = idx >= 0 && idx < state.queue.length ? state.queue[idx] : undefined;
    if (isOwned(t)) {
      if (audio) audio.volume = state.volume / 100;
      setSnapshot((current): PlaybackSnapshot => ({
        ...current,
        status: mapStatus(state.status),
        positionMs: state.position_ms ?? current.positionMs,
        durationMs: cur?.duration_ms,
      }));
      driveOwnerAudio(state);
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

  let controlChain: Promise<PlayerState | undefined> = Promise.resolve(undefined);
  let volumeTimer: ReturnType<typeof setTimeout> | undefined;

  function control(command: PlayerCommand, t = target()): Promise<PlayerState | undefined> {
    if (t === LOCAL_TARGET || t === "satellite-pending") return Promise.resolve(undefined);
    controlChain = controlChain
      .then(async () => {
        const a = servers.api();
        if (!a) return;
        const state = await a.player.control({ player_id: t, command });
        applyRemote(t, state);
        return state;
      })
      .catch((e) => {
        console.warn("[playback] control failed", e);
        return undefined;
      });
    return controlChain;
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
    id = resolveOutputTarget(id, sharedTargets(), owned());
    if (satelliteMode && id !== satellitePlayerId()) return;
    const prev = target();
    if (prev === LOCAL_TARGET && id !== LOCAL_TARGET) {
      savedLocal = { tracks: queue.slice(), index: currentIndex() };
      audio?.pause();
      ownerTrackId = undefined;
    }
    setTargetSignal(id);
    if (id === LOCAL_TARGET) {
      ownerTrackId = undefined;
      if (audio) audio.volume = localVolume() / 100;
      if (savedLocal) {
        setQueue(savedLocal.tracks);
        setCurrentIndex(savedLocal.index);
      }
      setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "paused" }));
    }
  }

  createEffect(() => {
    const t = target();
    if (t === LOCAL_TARGET || t === "satellite-pending") return;
    const loaded = playersRes() !== undefined;
    if (!loaded) return;
    if (sharedTargets().some((p) => p.id === t)) return;
    if (satelliteMode) return;

    const remoteQueue = queue.slice();
    const remoteIndex = currentIndex();
    const localQueue = savedLocal?.tracks ?? [];
    if (localQueue.length === 0) {
      savedLocal = { tracks: remoteQueue, index: remoteIndex };
    } else {
      toast.show("Target device has left");
    }
    setTarget(LOCAL_TARGET);
    audio?.pause();
    setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "paused" }));
  });

  const current = () => {
    const i = currentIndex();
    return i >= 0 && i < queue.length ? queue[i] : undefined;
  };

  // Shared targets report position through PlayerState instead of native audio events.
  createEffect(() => {
    snapshot().positionMs;
    current()?.id;
    reportAudiobookProgress(snapshot().status === "ended");
  });

  const setPref = (p: StreamPref) => {
    setPrefSignal(p);
    try {
      localStorage.setItem(PREF_KEY, JSON.stringify(p));
    } catch {
      /* ignore */
    }
  };

  function cycleRepeatMode(): void {
    setRepeatMode((current) => {
      const next = current === "off" ? "all" : current === "all" ? "one" : "off";
      try {
        localStorage.setItem(REPEAT_KEY, next);
      } catch {
        /* ignore */
      }
      return next;
    });
  }

  function toggleShuffle(): void {
    setShuffle((current) => {
      const next = !current;
      try {
        localStorage.setItem(SHUFFLE_KEY, String(next));
      } catch {
        /* ignore */
      }
      return next;
    });
  }

  function randomQueueIndex(): number {
    if (queue.length <= 1) return currentIndex();
    const current = currentIndex();
    const candidate = Math.floor(Math.random() * (queue.length - 1));
    return candidate >= current ? candidate + 1 : candidate;
  }

  // --- Local playback -------------------------------------------------------
  async function openIndex(index: number, startMs = 0): Promise<void> {
    const track = queue[index];
    if (!track || !audio) return;
    reportAudiobookProgress();
    setSnapshot((s): PlaybackSnapshot => ({ ...s, positionMs: startMs }));
    setCurrentIndex(index);
    const url = mediaUrl(track.id);
    if (!url) return;
    audio.src = url;
    if (startMs > 0) {
      const seek = () => {
        audio.currentTime = startMs / 1000;
        audio.removeEventListener("loadedmetadata", seek);
      };
      audio.addEventListener("loadedmetadata", seek);
    }
    try {
      await audio.play();
    } catch (e) {
      setSnapshot((s): PlaybackSnapshot => ({ ...s, status: "error", error: String(e) }));
    }
  }

  async function enableOutputAudio(): Promise<void> {
    if (!audio) throw new Error("Browser audio is unavailable");
    try {
      await applySatelliteSink();
      const AudioContextClass = globalThis.AudioContext;
      if (AudioContextClass) {
        const context = new AudioContextClass();
        await context.resume();
        await context.close();
      }
      setOutputAudioReady(true);
      setBlocked(false);
      // Start whatever the server asked for while the page was still blocked.
      if (audio.currentSrc && audio.paused) await audio.play();
    } catch (cause) {
      // Ask the browser what the real state is rather than reading it out of the failure:
      // a rejected play() can also mean a bad sink or a media error.
      setBlocked(!(await probeAutoplay()));
      throw cause;
    }
  }

  // --- Satellite autoplay unlock (§6.4) --------------------------------------
  // A satellite plays what somebody else queued, so the browser must already trust this page
  // by the time a directive arrives. Probe once on load, take the FIRST gesture anywhere on
  // the page as the unlock (no particular button needed), and keep the server informed so
  // controllers can warn instead of playing into silence.
  if (satelliteMode) {
    void probeAutoplay().then((allowed) => {
      if (allowed) setOutputAudioReady(true);
      setBlocked(!allowed);
    });
    onCleanup(
      onFirstGesture(() => {
        void enableOutputAudio().catch((error) => console.warn("[playback] unlock failed", error));
      }),
    );
    // The first report can only leave once the socket is up and the satellite knows its own
    // player id. This also re-reports after a reconnect, where registration clears the
    // server's previous view. `on` keeps the effect off the position/status signals that
    // reportAudioBlocked reads, which would otherwise re-fire it every second.
    createEffect(
      on(
        () => [Boolean(servers.api()), satellitePlayerId()] as const,
        ([hasApi, playerId]) => {
          if (hasApi && playerId) reportAudioBlocked();
        },
      ),
    );
  }

  async function playNow(tracks: Track[], startIndex = 0, startMs = 0): Promise<void> {
    if (isLocal()) {
      setQueue(tracks.slice());
      if (tracks.length) await openIndex(startIndex, startMs);
    } else {
      await control({ op: "clear" });
      if (!tracks.length) return;
      await control({ op: "enqueue", track_ids: tracks.map((t) => t.id) });
      await control({ op: "play", index: startIndex });
      if (startMs > 0) await control({ op: "seek", position_ms: startMs });
    }
  }

  function enqueue(tracks: Track[]): void {
    if (!tracks.length) return;
    const startQueue = queue.length === 0;
    if (isLocal()) {
      setQueue((q) => [...q, ...tracks]);
      if (startQueue) void openIndex(0);
    } else {
      void (async () => {
        const state = await control({ op: "enqueue", track_ids: tracks.map((t) => t.id) });
        const previousLength = state ? state.queue.length - tracks.length : queue.length;
        if (previousLength === 0) await control({ op: "play", index: 0 });
      })();
    }
  }

  async function enqueueAndPlay(track: Track, startMs = 0): Promise<void> {
    const appendedIndex = queue.length;
    if (isLocal()) {
      setQueue((current) => [...current, track]);
      await openIndex(appendedIndex, startMs);
    } else {
      const state = await control({ op: "enqueue", track_ids: [track.id] });
      const newIndex = state ? state.queue.length - 1 : appendedIndex;
      await control({ op: "play", index: newIndex });
      if (startMs > 0) await control({ op: "seek", position_ms: startMs });
    }
  }

  async function playIndex(index: number): Promise<void> {
    if (isLocal()) await openIndex(index);
    else await control({ op: "play", index });
  }

  function togglePlay(): void {
    if (!isLocal()) {
      void control(snapshot().status === "playing" ? { op: "pause" } : { op: "play" });
      return;
    }
    if (!audio) return;
    if (!audio.paused) audio.pause();
    else if (audio.currentSrc) void audio.play();
    else if (current()) void openIndex(currentIndex());
  }

  async function next(): Promise<void> {
    if (!queue.length) return;
    if (shuffle() && queue.length > 1) {
      await playIndex(randomQueueIndex());
      return;
    }
    const i = currentIndex();
    if (i + 1 < queue.length) {
      await playIndex(i + 1);
    } else if (repeatMode() === "all") {
      await playIndex(0);
    } else if (isLocal()) {
      stop();
    }
  }

  async function advanceAfterEnd(): Promise<void> {
    if (repeatMode() === "one" && currentIndex() >= 0) {
      await playIndex(currentIndex());
      return;
    }
    await next();
  }

  createEffect(
    on(
      () => snapshot().status,
      (status, prevStatus) => {
        if (status === "ended" && prevStatus !== "ended") void advanceAfterEnd();
      },
    ),
  );

  async function previous(): Promise<void> {
    if (!isLocal()) {
      await control({ op: "previous" });
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
      void control({ op: "seek", position_ms: ms });
    }
  }

  function setVolume(value: number): void {
    const change = planVolumeChange(target(), value);
    if (change.mediaVolume !== undefined) {
      setLocalVolume(change.volume);
      if (audio) audio.volume = change.mediaVolume;
      try {
        localStorage.setItem(VOLUME_KEY, String(change.volume));
      } catch {
        /* ignore */
      }
      return;
    }
    setRemoteVolume(change.volume);
    if (audio && isOwned(target())) audio.volume = change.volume / 100;
    if (volumeTimer !== undefined) clearTimeout(volumeTimer);
    const volumeTarget = target();
    const command = change.command;
    if (command) {
      volumeTimer = setTimeout(() => {
        volumeTimer = undefined;
        void control(command, volumeTarget);
      }, 50);
    }
  }

  function stop(): void {
    if (!isLocal()) {
      void control({ op: "clear" });
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

  async function saveQueueAsPlaylist(name: string): Promise<void> {
    const base = mediaBase();
    if (!base) throw new Error("Connect a server first.");
    const session = servers.active()?.session;
    const visibility = session && session.role !== "guest" ? "private" : "public";
    const res = await fetch(`${base}/api/playlists/from-queue`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        name,
        visibility,
        owner: visibility === "private" ? session?.account_id : undefined,
        track_ids: queue.map((track) => track.id),
      }),
    });
    if (!res.ok) throw new Error(await res.text());
    toast.show("Queue saved as playlist");
  }

  function removeAt(index: number): void {
    if (!isLocal()) {
      void control({ op: "remove", index });
      return;
    }
    if (index < 0 || index >= queue.length) return;
    const cur = currentIndex();
    const wasCurrent = index === cur;
    const beforeLen = queue.length;
    setQueue((q) => q.filter((_, i) => i !== index));
    if (wasCurrent) {
      if (beforeLen <= 1) stop();
      else void openIndex(Math.min(index, beforeLen - 2));
    } else if (index < cur) {
      setCurrentIndex(cur - 1);
    }
  }

  function move(from: number, to: number): void {
    if (!isLocal()) {
      void control({ op: "reorder", from_index: from, to_index: to });
      return;
    }
    if (from === to || from < 0 || from >= queue.length || to < 0 || to >= queue.length) return;
    const cur = currentIndex();
    setQueue((q) => {
      const arr = [...q];
      const [item] = arr.splice(from, 1);
      if (item) arr.splice(to, 0, item);
      return arr;
    });
    if (from === cur) setCurrentIndex(to);
    else if (from < cur && to >= cur) setCurrentIndex(cur - 1);
    else if (from > cur && to <= cur) setCurrentIndex(cur + 1);
  }

  onCleanup(() => {
    if (volumeTimer !== undefined) clearTimeout(volumeTimer);
    reportAudiobookProgress();
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
    enqueueAndPlay,
    playIndex,
    togglePlay,
    next,
    previous,
    seek,
    volume,
    setVolume,
    stop,
    removeAt,
    move,
    saveQueueAsPlaylist,
    target,
    setTarget,
    sharedTargets,
    owned,
    markOwned,
    claimOutput,
    releaseDevice,
    enableOutputAudio,
    outputAudioReady,
    audioBlocked,
    repeatMode,
    cycleRepeatMode,
    shuffle,
    toggleShuffle,
  };

  return <PlaybackContext.Provider value={value}>{props.children}</PlaybackContext.Provider>;
}

export function usePlayback(): PlaybackContextValue {
  const ctx = useContext(PlaybackContext);
  if (!ctx) throw new Error("usePlayback must be used within <PlaybackProvider>");
  return ctx;
}
