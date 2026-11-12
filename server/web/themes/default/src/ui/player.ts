import { BaseComponent, h, button, select, askText, askConfirm, field, note, modal } from "./dom.ts";
import { usePlayback } from "../stores/playback.ts";
import { useServers } from "../stores/servers.ts";
import { formatDuration } from "../lib/format.ts";
import { TrackRows } from "./tracks.ts";
import { cover } from "./artwork.ts";
import { CsilConnection } from "../lib/csil.ts";
import { ServerApi } from "../lib/services.ts";
import { satelliteOutput, setSatelliteOutput, enterSatelliteMode, leaveSatelliteMode, satelliteToken } from "../lib/satellite-mode.ts";
import { acceptTerms, hasAcceptedTerms, TERMS_URL } from "../lib/compliance.ts";

function syncTargets(input: HTMLSelectElement): void {
  const pb = usePlayback(), targets = pb.sharedTargets();
  const key = JSON.stringify([pb.target(), targets.map(target => [target.id, target.name, target.audio_blocked])]);
  if (input.dataset.targets === key) return;
  input.dataset.targets = key;
  const options = satelliteToken() ? [] : [h("option", { value: "local" }, "This browser")];
  for (const target of targets) options.push(h("option", { value: target.id }, `${target.name}${target.audio_blocked ? " · Enable audio on device" : ""}`));
  input.replaceChildren(...options); input.value = pb.target();
}

export class PlayerBar extends BaseComponent {
  openQueue: () => void = () => undefined;
  private titleNode = h("strong");
  private subtitle = h("span", { class: "muted" });
  private artwork = h("div", { class: "player-art" });
  private play = button("Play", () => usePlayback().togglePlay(), "primary");
  private target = select("Playback device", [], "local", value => usePlayback().setTarget(value));
  private seek = h("input", { type: "range", min: 0, max: 0, value: "0", "aria-label": "Playback position" });
  private volume = h("input", { type: "range", min: 0, max: 100, value: "100", "aria-label": "Volume" });
  private elapsed = h("span", { class: "time" });
  private duration = h("span", { class: "time" });
  private queueButton = button("Queue", () => this.openQueue());
  private repeat = button("Repeat off", () => usePlayback().cycleRepeatMode(), "flat");
  private shuffle = button("Shuffle", () => usePlayback().toggleShuffle(), "flat");
  private error = h("div", { class: "player-error", role: "status" });
  private coverKey = "";
  connectedCallback(): void {
    super.connectedCallback(); this.className = "player-bar";
    this.setAttribute("aria-label", "Playback controls");
    this.shuffle.toggle = true;
    this.replaceChildren(this.error, h("div", { class: "player-main" },
      h("div", { class: "player-track" }, this.artwork, h("div", { class: "track-copy" }, this.titleNode, this.subtitle)),
      h("div", { class: "player-controls" }, button("Previous", () => usePlayback().previous(), "flat"), this.play, button("Next", () => usePlayback().next())),
      h("div", { class: "player-destination" }, h("label", {}, h("span", { class: "eyebrow" }, "Playing on"), this.target), this.queueButton,
        button("More", () => this.moreControls()))),
      h("div", { class: "player-progress" }, this.elapsed, this.seek, this.duration, this.shuffle, this.repeat, this.volume));
    this.listen(this.seek, "change", () => usePlayback().seek(Number(this.seek.value)));
    this.listen(this.volume, "input", () => usePlayback().setVolume(Number(this.volume.value)));
    this.own(usePlayback().subscribe(() => this.invalidate())); this.own(useServers().subscribe(() => this.invalidate())); this.invalidate();
  }
  private moreControls(): void {
    const pb = usePlayback();
    const volume = h("input", { type: "range", min: 0, max: 100, value: String(pb.volume()), "aria-label": "Device volume" });
    volume.addEventListener("input", () => pb.setVolume(Number(volume.value)));
    const shuffle = button("Shuffle", () => pb.toggleShuffle()); shuffle.toggle = true; shuffle.pressed = pb.shuffle();
    const repeat = button(`Repeat ${pb.repeatMode()}`, () => { pb.cycleRepeatMode(); repeat.label = `Repeat ${pb.repeatMode()}`; });
    modal("Playback options", h("div", { class: "stack" },
      button("Previous track", () => pb.previous()), h("div", { class: "actions" }, shuffle, repeat), h("label", {}, "Volume", volume)));
  }
  render(): void {
    const pb = usePlayback(), servers = useServers(), track = pb.current(), snap = pb.snapshot();
    this.titleNode.textContent = track?.title || "Ready when you are";
    this.subtitle.textContent = track?.artist_name || (track ? servers.active()?.name ?? "" : "Find a track and press Play");
    this.play.label = snap.status === "playing" ? "Pause" : "Play"; this.play.disabled = !track && !pb.queue.length;
    this.queueButton.label = `Queue${pb.queue.length ? ` · ${pb.queue.length}` : ""}`;
    this.elapsed.textContent = formatDuration(snap.positionMs); this.duration.textContent = formatDuration(snap.durationMs ?? track?.duration_ms ?? 0);
    this.seek.max = String(snap.durationMs ?? track?.duration_ms ?? 0);
    if (document.activeElement !== this.seek) this.seek.value = String(snap.positionMs);
    this.seek.disabled = !track;
    if (document.activeElement !== this.volume) this.volume.value = String(pb.volume());
    this.repeat.label = `Repeat ${pb.repeatMode()}`; this.shuffle.pressed = pb.shuffle();
    syncTargets(this.target);
    const targets = pb.sharedTargets();
    const blocked = targets.find(target => target.id === pb.target())?.audio_blocked;
    this.error.textContent = snap.error ? `Playback failed: ${snap.error}` : blocked ? "Enable audio on the selected device to hear playback." : "";
    this.error.hidden = !this.error.textContent;
    const coverKey = `${servers.activeId()}|${track?.album_id}`;
    if (coverKey !== this.coverKey) {
      this.coverKey = coverKey; const api = servers.api();
      this.artwork.replaceChildren(api && track?.album_id ? cover(api, track.album_id, track.album_title ?? track.title) : h("span", { "aria-hidden": "true" }, "♫"));
    }
  }
}
customElements.define("ichoi-player", PlayerBar);

export class QueuePanel extends BaseComponent {
  close: () => void = () => undefined;
  private count = h("p", { class: "muted" });
  private undo = button("Undo", () => usePlayback().undoQueue());
  private rows = new TrackRows();
  private target = select("Queue playback device", [], "local", value => usePlayback().setTarget(value));
  private play = button("Play", () => usePlayback().togglePlay(), "primary");
  connectedCallback(): void {
    super.connectedCallback(); this.className = "queue-panel";
    this.rows.queueMode = true;
    this.replaceChildren(h("header", { class: "queue-head" }, h("div", {}, h("h2", {}, "Your queue"), this.count), button("Close queue", this.close)),
      h("div", { class: "queue-playback" }, this.target, this.play),
      h("div", { class: "queue-toolbar" }, this.undo,
        button("Save playlist", async () => {
          if (!hasAcceptedTerms()) {
            if (!await askConfirm({ title: "Terms of use", text: `Accept the terms at ${TERMS_URL} before you create a playlist.`, okLabel: "Accept terms" })) return;
            acceptTerms();
          }
          const name = await askText({ title: "Save queue as playlist", label: "Playlist name", required: true });
          if (name) await usePlayback().saveQueueAsPlaylist(name);
        }), button("Clear", async () => {
          if (await askConfirm({ title: "Clear queue?", text: "Remove all tracks from this device’s queue? You can undo this change.", okLabel: "Clear queue" })) usePlayback().clearQueue();
        })), this.rows);
    this.own(usePlayback().subscribe(() => this.invalidate())); this.invalidate();
  }
  render(): void {
    const pb = usePlayback(); this.count.textContent = `${pb.queue.length} tracks · ${pb.target() === "local" ? "This browser" : pb.sharedTargets().find(p => p.id === pb.target())?.name ?? "Selected device"}`;
    this.undo.disabled = !pb.canUndoQueue();
    syncTargets(this.target);
    this.play.label = pb.snapshot().status === "playing" ? "Pause" : "Play";
    this.play.disabled = !pb.current() && !pb.queue.length;
  }
}
customElements.define("ichoi-queue", QueuePanel);

class SatellitePage extends BaseComponent {
  private validation?: CsilConnection;
  connectedCallback(): void {
    super.connectedCallback(); this.className = "page stack";
    this.own(() => this.validation?.close("satellite page closed"));
    const token = field("Satellite token", "", "password");
    this.replaceChildren(h("h1", {}, "Satellite player"));
    if (!satelliteToken()) this.append(h("p", {}, "Enter the satellite token from your server administrator."), token,
      button("Connect satellite", async () => {
        const clean = token.value.trim(); if (!clean) throw new Error("Enter a satellite token.");
        const output = satelliteOutput();
        const conn = new CsilConnection({ url: `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/ws`, nodeToken: clean });
        this.validation?.close("replaced credential"); this.validation = conn;
        try {
          await conn.connect(); const api = new ServerApi(conn);
          const result = await api.node.register({ hostname: location.hostname, platform: "chromeos", arch: navigator.platform || "browser",
            outputs: [{ os_device_id: output.id, friendly_name: output.name, channels: 2, sample_rates: [48000], is_default: output.id === "default" }] });
          if (!result.players.length) throw new Error("This output is disabled by the administrator.");
          await api.session.whoami(); if (!this.isActive()) return;
          enterSatelliteMode(clean); location.replace("/now-playing");
        } finally { conn.close("validation finished"); }
      }, "primary"));
    else this.append(h("p", {}, `Output: ${satelliteOutput().name}`),
      h("p", {}, "Keep this page open. Select this device from another Ichoi client to control its queue."),
      button("Enable audio", async () => { await usePlayback().enableOutputAudio(); note("Audio is enabled."); }, "primary"),
      button("Choose output", async () => {
        const devices = navigator.mediaDevices as MediaDevices & { selectAudioOutput?: () => Promise<MediaDeviceInfo> };
        if (!devices?.selectAudioOutput) throw new Error("This browser uses the system audio output. Change the output in your system settings.");
        const output = await devices.selectAudioOutput(); setSatelliteOutput(output.deviceId, output.label || "Selected output");
        const id = useServers().activeId(); if (id) await useServers().reconnect(id);
        await usePlayback().enableOutputAudio();
      }), button("Fullscreen", () => document.documentElement.requestFullscreen()),
      button("Leave satellite mode", async () => {
        if (await askConfirm({ title: "Leave satellite mode?", text: "You must sign in again for normal access." })) { leaveSatelliteMode(); location.assign("/"); }
      }, "flat"));
  }
}
customElements.define("ichoi-satellite", SatellitePage);
export function satellitePage() { return new SatellitePage(); }
