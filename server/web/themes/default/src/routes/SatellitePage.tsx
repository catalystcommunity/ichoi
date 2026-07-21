import { createSignal, onCleanup, onMount, Show, type JSX } from "solid-js";
import { CsilConnection } from "../lib/csil.ts";
import { ServerApi } from "../lib/services.ts";
import type { Player, PlayerState } from "../lib/schema.ts";
import {
  enterSatelliteMode,
  leaveSatelliteMode,
  SATELLITE_OUTPUT_KEY,
  satelliteToken,
} from "../lib/satellite-mode.ts";

interface OutputPicker extends MediaDevices {
  selectAudioOutput?: () => Promise<MediaDeviceInfo>;
}

export function SatellitePage(): JSX.Element {
  const [enteredToken, setEnteredToken] = createSignal("");
  const [status, setStatus] = createSignal("Waiting for a satellite token");
  const [error, setError] = createSignal<string>();
  const [player, setPlayer] = createSignal<Player>();
  const [state, setState] = createSignal<PlayerState>();
  const [output, setOutput] = createSignal(localStorage.getItem(SATELLITE_OUTPUT_KEY) ?? "default");
  const [outputName, setOutputName] = createSignal("Default audio output");
  const [audioReady, setAudioReady] = createSignal(false);
  let conn: CsilConnection | undefined;
  let api: ServerApi | undefined;
  let unsubscribe: (() => void) | undefined;
  let activeTrack: string | undefined;
  let lastReportSecond = -1;
  let reconnectReady = false;
  let sessionAttached = false;
  const audio = new Audio();

  const mediaUrl = (trackId: string) => `/media/${encodeURIComponent(trackId)}?format=aac`;

  async function applySink(): Promise<void> {
    if ("setSinkId" in audio) await audio.setSinkId(output());
  }

  function report(statusValue: "stopped" | "playing" | "paused", position?: number): void {
    const current = player();
    if (!api || !current) return;
    api.node.report({ player_id: current.id, status: statusValue, position_ms: position });
  }

  async function drive(next: PlayerState): Promise<void> {
    setState(next);
    const index = next.current_index ?? -1;
    const item = index >= 0 ? next.queue[index] : undefined;
    if (next.status === "playing" && item) {
      if (activeTrack !== item.track_id) {
        activeTrack = item.track_id;
        audio.src = mediaUrl(item.track_id);
        if (next.position_ms) audio.currentTime = next.position_ms / 1000;
      }
      try {
        await applySink();
        await audio.play();
      } catch {
        setAudioReady(false);
        setStatus("Select Enable audio before remote playback can begin");
      }
    } else if (next.status === "paused") {
      audio.pause();
    } else {
      audio.pause();
      activeTrack = undefined;
      audio.removeAttribute("src");
    }
  }

  async function provision(token: string): Promise<void> {
    if (!api) return;
    setError(undefined);
    setStatus("Registering audio output…");
    const response = await api.node.register({
      hostname: "chromebook-pwa",
      platform: "chromeos",
      arch: navigator.platform || "browser",
      outputs: [{
        os_device_id: output(),
        friendly_name: outputName(),
        channels: 2,
        sample_rates: [48000],
        is_default: output() === "default",
      }],
    });
    const registered = response.players[0];
    if (!registered) throw new Error("This output has been disabled by the administrator");
    setPlayer(registered);
    const wasAlreadySatellite = Boolean(satelliteToken());
    enterSatelliteMode(token);
    unsubscribe?.();
    sessionAttached = false;
    unsubscribe = api.player.subscribe({ player_id: registered.id }, (next) => {
      void drive(next);
      if (!sessionAttached) {
        sessionAttached = true;
        report(next.status, next.position_ms);
      }
    });
    setStatus("Connected");
    reconnectReady = true;
    if (!wasAlreadySatellite) location.replace("/satellite");
  }

  async function connect(token: string): Promise<void> {
    const clean = token.trim();
    if (!clean) return;
    unsubscribe?.();
    conn?.close("switching satellite credential");
    setStatus("Connecting…");
    setError(undefined);
    conn = new CsilConnection({
      url: `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/ws`,
      nodeToken: clean,
      onState: (next, detail) => {
        if (next === "error" || next === "closed") setStatus(detail ?? next);
        if (next === "ready" && reconnectReady) void provision(clean);
      },
    });
    api = new ServerApi(conn);
    try {
      await conn.connect();
      await provision(clean);
    } catch (cause) {
      setPlayer(undefined);
      setError(cause instanceof Error ? cause.message : String(cause));
      setStatus("Satellite token was not accepted");
    }
  }

  async function enableAudio(): Promise<void> {
    try {
      await applySink();
      const context = new AudioContext();
      await context.resume();
      await context.close();
      setAudioReady(true);
      setStatus(player() ? "Connected and ready for playback" : status());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function chooseOutput(): Promise<void> {
    const media = navigator.mediaDevices as OutputPicker | undefined;
    if (!media?.selectAudioOutput) {
      setError("This Chrome version does not expose an output chooser; the system default will be used.");
      return;
    }
    try {
      const selected = await media.selectAudioOutput();
      setOutput(selected.deviceId);
      setOutputName(selected.label || "Selected audio output");
      localStorage.setItem(SATELLITE_OUTPUT_KEY, selected.deviceId);
      const token = satelliteToken();
      if (token) await connect(token);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  function exitSatellite(): void {
    if (!confirm("Leave satellite mode? You will need to sign in again for normal Ichoi access.")) return;
    unsubscribe?.();
    conn?.close("leaving satellite mode");
    leaveSatelliteMode();
    location.assign("/");
  }

  function forgetRejectedToken(): void {
    conn?.close("forgetting satellite credential");
    leaveSatelliteMode();
    setEnteredToken("");
    setError(undefined);
    setStatus("Waiting for a satellite token");
  }

  audio.addEventListener("play", () => report("playing", Math.round(audio.currentTime * 1000)));
  audio.addEventListener("pause", () => {
    if (!audio.ended) report("paused", Math.round(audio.currentTime * 1000));
  });
  audio.addEventListener("ended", () => report("stopped", 0));
  audio.addEventListener("timeupdate", () => {
    const second = Math.floor(audio.currentTime);
    if (second !== lastReportSecond) {
      lastReportSecond = second;
      report("playing", second * 1000);
    }
  });

  onMount(() => {
    const saved = satelliteToken();
    if (saved) void connect(saved);
  });
  onCleanup(() => {
    unsubscribe?.();
    conn?.close("satellite page closing");
    audio.pause();
  });

  const current = () => {
    const snapshot = state();
    const index = snapshot?.current_index ?? -1;
    return index >= 0 ? snapshot?.queue[index] : undefined;
  };

  return (
    <main class="satellite-page">
      <section class="satellite-console panel">
        <div class="eyebrow">Ichoi satellite</div>
        <h1 class="page-title">{player()?.name ?? "Connect this Chromebook"}</h1>
        <p class="hint">{status()}</p>

        <Show when={!player()}>
          <form onSubmit={(event) => { event.preventDefault(); void connect(enteredToken()); }}>
            <div class="field">
              <label for="satellite-token">Satellite token</label>
              <input
                id="satellite-token"
                class="input"
                type="password"
                autocomplete="off"
                required
                value={enteredToken()}
                onInput={(event) => setEnteredToken(event.currentTarget.value)}
              />
            </div>
            <button class="btn btn-primary" type="submit">Connect satellite</button>
          </form>
          <Show when={satelliteToken()}>
            <button class="btn btn-ghost" type="button" onClick={forgetRejectedToken}>Forget saved satellite token</button>
          </Show>
        </Show>

        <Show when={player()}>
          <div class="satellite-now-playing">
            <div class="eyebrow">Now playing</div>
            <h2>{current()?.title ?? "Nothing playing"}</h2>
            <p class="hint">{current()?.artist ?? player()!.name}</p>
          </div>
          <div class="row satellite-actions">
            <button class="btn btn-primary" type="button" onClick={() => void enableAudio()}>
              {audioReady() ? "Audio enabled" : "Enable audio"}
            </button>
            <button class="btn" type="button" onClick={() => void chooseOutput()}>Choose output</button>
            <button class="btn" type="button" onClick={() => void document.documentElement.requestFullscreen?.()}>
              Fullscreen
            </button>
          </div>
          <p class="hint">Output: {outputName()}</p>
          <button class="btn btn-ghost" type="button" onClick={exitSatellite}>Leave satellite mode</button>
        </Show>

        <Show when={error()}>{(message) => <p class="error">{message()}</p>}</Show>
      </section>
    </main>
  );
}
