import { createSignal, onCleanup, Show, type JSX } from "solid-js";
import { CsilConnection } from "../lib/csil.ts";
import { ServerApi } from "../lib/services.ts";
import {
  enterSatelliteMode,
  leaveSatelliteMode,
  satelliteOutput,
  satelliteToken,
  setSatelliteOutput,
} from "../lib/satellite-mode.ts";
import { useServers } from "../stores/servers.tsx";

interface OutputPicker extends MediaDevices {
  selectAudioOutput?: () => Promise<MediaDeviceInfo>;
}

export function SatellitePage(): JSX.Element {
  const servers = useServers();
  const saved = Boolean(satelliteToken());
  const initialOutput = satelliteOutput();
  const [enteredToken, setEnteredToken] = createSignal("");
  const [status, setStatus] = createSignal(
    saved ? "This browser is connected as a satellite." : "Enter a satellite token once.",
  );
  const [error, setError] = createSignal<string>();
  const [outputName, setOutputName] = createSignal(initialOutput.name);
  let validationConnection: CsilConnection | undefined;

  async function registerWith(token: string): Promise<void> {
    const clean = token.trim();
    if (!clean) return;
    validationConnection?.close("switching satellite credential");
    setStatus("Validating satellite and registering this output…");
    setError(undefined);

    const output = satelliteOutput();
    const conn = new CsilConnection({
      url: `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/ws`,
      nodeToken: clean,
    });
    validationConnection = conn;
    try {
      await conn.connect();
      const api = new ServerApi(conn);
      const registered = await api.node.register({
        hostname: location.hostname,
        platform: "chromeos",
        arch: navigator.platform || "browser",
        outputs: [{
          os_device_id: output.id,
          friendly_name: output.name,
          channels: 2,
          sample_rates: [48_000],
          is_default: output.id === "default",
        }],
      });
      if (!registered.players[0]) {
        throw new Error("This satellite output has been disabled by the administrator");
      }
      await api.session.whoami();
      enterSatelliteMode(clean);
      conn.close("satellite registration complete");
      location.replace("/");
    } catch (cause) {
      setStatus("Satellite token was not accepted");
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function chooseOutput(): Promise<void> {
    const media = navigator.mediaDevices as OutputPicker | undefined;
    if (!media?.selectAudioOutput) {
      setError("This browser does not expose an output chooser; the system default will be used.");
      return;
    }
    try {
      const selected = await media.selectAudioOutput();
      const name = selected.label || "Selected audio output";
      setSatelliteOutput(selected.deviceId, name);
      setOutputName(name);
      setStatus("Output changed. Reconnecting…");
      const active = servers.activeId();
      if (active) await servers.reconnect(active);
      setStatus("Connected and ready.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  function leave(): void {
    if (!confirm("Leave satellite mode? You will need to sign in again for normal Ichoi access.")) {
      return;
    }
    leaveSatelliteMode();
    location.assign("/");
  }

  onCleanup(() => validationConnection?.close("satellite page closing"));

  return (
    <main class="satellite-page">
      <section class="satellite-console panel">
        <div class="eyebrow">Ichoi satellite</div>
        <h1 class="page-title">
          {servers.active()?.session?.handle ?? (saved ? "Connecting satellite…" : "Register this browser")}
        </h1>
        <p class="hint">{servers.active()?.detail ?? status()}</p>

        <Show
          when={saved}
          fallback={
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void registerWith(enteredToken());
              }}
            >
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
          }
        >
          <div class="satellite-now-playing">
            <div class="eyebrow">Local output</div>
            <h2>{outputName()}</h2>
            <p class="hint">
              This credential can browse the library and manage only this satellite’s queue.
            </p>
          </div>
          <div class="row satellite-actions">
            <button class="btn btn-primary" type="button" onClick={() => location.assign("/")}>
              Open library
            </button>
            <button class="btn" type="button" onClick={() => void chooseOutput()}>
              Choose output
            </button>
            <button
              class="btn"
              type="button"
              onClick={() => void document.documentElement.requestFullscreen?.()}
            >
              Fullscreen
            </button>
          </div>
          <button class="btn btn-ghost" type="button" onClick={leave}>Leave satellite mode</button>
        </Show>

        <Show when={error()}>{(message) => <p class="error">{message()}</p>}</Show>
      </section>
    </main>
  );
}
