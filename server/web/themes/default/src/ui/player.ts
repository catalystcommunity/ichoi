import { BaseComponent, h, button, select, askText, askConfirm, field, note, modal, IconButton } from "./dom.ts";
import { icon, type IconName } from "./icons.ts";
import type { RepeatMode } from "../lib/schema.ts";
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

const REPEAT_UI: Record<RepeatMode, { icon: IconName; label: string }> = {
  off: { icon: "playThrough", label: "Play the queue once. Select to repeat the queue." },
  all: { icon: "repeat", label: "Repeat the queue. Select to repeat one track." },
  one: { icon: "repeatOne", label: "Repeat one track. Select to play the queue once." },
};

function volumeIcon(volume: number): IconName { return volume <= 0 ? "muted" : volume < 50 ? "volumeLow" : "volume"; }

export class PlayerBar extends BaseComponent {
  openQueue: () => void = () => undefined;
  private titleNode = h("strong", { class: "player-title" });
  private subtitle = h("span", { class: "muted player-subtitle" });
  private targetName = h("span", { class: "player-target-name" });
  private artwork = h("div", { class: "player-art" });
  private play = new IconButton("play", "Play", () => usePlayback().togglePlay(), "play-button");
  private stop = new IconButton("stop", "Stop", () => usePlayback().stop(), "stop-button");
  private previous = new IconButton("previous", "Previous track", () => usePlayback().previous());
  private next = new IconButton("next", "Next track", () => usePlayback().next());
  private shuffle = new IconButton("shuffle", "Shuffle", () => usePlayback().toggleShuffle(), "mode-button");
  private repeat = new IconButton("playThrough", REPEAT_UI.off.label, () => usePlayback().cycleRepeatMode(), "mode-button");
  private queueButton = new IconButton("queue", "Show queue", () => this.openQueue(), "queue-button");
  private queueCount = h("span", { class: "badge", "aria-hidden": "true" });
  private mute = new IconButton("volume", "Mute", () => this.toggleMute(), "mute-button");
  private more = new IconButton("more", "More playback options", () => this.moreControls(), "more-button");
  private target = select("Playback device", [], "local", value => usePlayback().setTarget(value));
  private seek = h("input", { type: "range", min: 0, max: 0, value: "0", class: "seek", "aria-label": "Playback position" });
  private volume = h("input", { type: "range", min: 0, max: 100, value: "100", class: "volume", "aria-label": "Volume" });
  private elapsed = h("span", { class: "time" });
  private duration = h("span", { class: "time" });
  private error = h("div", { class: "player-error", role: "status" });
  private coverKey = "";
  private unmutedVolume = 100;
  private queueShown = false;
  /** The application tells the bar when the queue panel opens or closes. */
  set queueOpen(open: boolean) { this.queueShown = open; this.invalidate(); }
  connectedCallback(): void {
    super.connectedCallback(); this.className = "player-bar";
    this.setAttribute("aria-label", "Playback controls");
    this.queueButton.element.append(this.queueCount);
    const device = h("label", { class: "device-picker" }, icon("speaker"), h("span", { class: "visually-hidden" }, "Playing on"), this.target);
    this.replaceChildren(this.error, h("div", { class: "player-main" },
      h("div", { class: "player-track" }, this.artwork, h("div", { class: "track-copy" }, this.titleNode, this.subtitle, this.targetName)),
      h("div", { class: "transport", role: "group", "aria-label": "Transport" },
        h("div", { class: "transport-side start" }, this.shuffle.element, this.stop.element, this.previous.element),
        this.play.element,
        h("div", { class: "transport-side end" }, this.next.element, this.repeat.element, this.more.element)),
      h("div", { class: "player-progress" }, this.elapsed, this.seek, this.duration),
      h("div", { class: "player-aside" }, device, this.queueButton.element,
        h("div", { class: "volume-control" }, this.mute.element, this.volume))));
    this.listen(this.seek, "input", () => this.paintRange(this.seek));
    this.listen(this.seek, "change", () => usePlayback().seek(Number(this.seek.value)));
    this.listen(this.volume, "input", () => { usePlayback().setVolume(Number(this.volume.value)); this.paintRange(this.volume); });
    this.own(usePlayback().subscribe(() => this.invalidate())); this.own(useServers().subscribe(() => this.invalidate())); this.invalidate();
  }
  private toggleMute(): void {
    const pb = usePlayback(), current = pb.volume();
    if (current > 0) { this.unmutedVolume = current; pb.setVolume(0); } else pb.setVolume(this.unmutedVolume || 50);
  }
  private paintRange(input: HTMLInputElement): void {
    const max = Number(input.max) || 0;
    input.style.setProperty("--fill", `${max ? Math.min(100, (Number(input.value) / max) * 100) : 0}%`);
  }
  /** Phones show the less frequent controls here. */
  private moreControls(): void {
    const pb = usePlayback();
    const volume = h("input", { type: "range", min: 0, max: 100, value: String(pb.volume()), class: "volume", "aria-label": "Device volume" });
    volume.addEventListener("input", () => { pb.setVolume(Number(volume.value)); this.paintRange(volume); });
    this.paintRange(volume);
    const stop = new IconButton("stop", "Stop", () => pb.stop());
    modal("Playback options", h("div", { class: "stack player-options" },
      h("div", { class: "option-row" }, stop.element, h("span", {}, "Stop and return to the start of the track")),
      h("label", { class: "option-row" }, icon("volume"), h("span", { class: "visually-hidden" }, "Volume"), volume)));
  }
  render(): void {
    const pb = usePlayback(), servers = useServers(), track = pb.current(), snap = pb.snapshot();
    const hasMedia = !!track || pb.queue.length > 0, playing = snap.status === "playing";
    this.titleNode.textContent = track?.title || "Nothing playing";
    this.subtitle.textContent = track ? [track.artist_name, track.album_title].filter(Boolean).join(" · ") || (servers.active()?.name ?? "") : "Choose a track to start";
    this.play.icon = playing ? "pause" : "play"; this.play.label = playing ? "Pause" : "Play";
    for (const control of [this.play, this.stop, this.previous, this.next]) control.disabled = !hasMedia;
    this.shuffle.pressed = pb.shuffle();
    const repeat = REPEAT_UI[pb.repeatMode()];
    this.repeat.icon = repeat.icon; this.repeat.label = repeat.label;
    this.repeat.element.classList.toggle("is-active", pb.repeatMode() !== "off");
    const count = pb.queue.length;
    this.queueCount.textContent = count ? String(count > 999 ? "999+" : count) : ""; this.queueCount.hidden = !count;
    this.queueButton.label = this.queueShown ? "Hide queue" : `Show queue (${count} ${count === 1 ? "track" : "tracks"})`;
    this.queueButton.pressed = this.queueShown;
    const durationMs = snap.durationMs ?? track?.duration_ms ?? 0;
    this.elapsed.textContent = formatDuration(snap.positionMs); this.duration.textContent = formatDuration(durationMs);
    this.seek.max = String(durationMs);
    if (document.activeElement !== this.seek) this.seek.value = String(snap.positionMs);
    this.seek.disabled = !track; this.paintRange(this.seek);
    if (document.activeElement !== this.volume) this.volume.value = String(pb.volume());
    this.paintRange(this.volume);
    this.mute.icon = volumeIcon(pb.volume()); this.mute.label = pb.volume() > 0 ? "Mute" : "Unmute";
    syncTargets(this.target);
    const targets = pb.sharedTargets(), selected = targets.find(target => target.id === pb.target());
    this.targetName.textContent = selected ? `Playing on ${selected.name}` : ""; this.targetName.hidden = !selected;
    this.classList.toggle("is-remote", !!selected);
    this.error.textContent = snap.error ? `Playback failed: ${snap.error}` : selected?.audio_blocked ? "Enable audio on the selected device to hear playback." : "";
    this.error.hidden = !this.error.textContent;
    const coverKey = `${servers.activeId()}|${track?.album_id}`;
    if (coverKey !== this.coverKey) {
      this.coverKey = coverKey; const api = servers.api();
      this.artwork.replaceChildren(api && track?.album_id ? cover(api, track.album_id, track.album_title ?? track.title) : h("span", { class: "player-art-empty", "aria-hidden": "true" }, "♫"));
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
  private play = new IconButton("play", "Play", () => usePlayback().togglePlay(), "play-button");
  connectedCallback(): void {
    super.connectedCallback(); this.className = "queue-panel";
    this.rows.queueMode = true;
    this.replaceChildren(h("header", { class: "queue-head" }, h("div", {}, h("h2", {}, "Your queue"), this.count), new IconButton("close", "Close queue", () => this.close()).element),
      h("div", { class: "queue-playback" }, this.target, this.play.element),
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
    const playing = pb.snapshot().status === "playing";
    this.play.icon = playing ? "pause" : "play"; this.play.label = playing ? "Pause" : "Play";
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
