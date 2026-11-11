import { BaseComponent, h, button, heading, request, select, modal, askText, empty, note, requestError } from "./dom.ts";
import { useServers } from "../stores/servers.ts";
import { usePlayback } from "../stores/playback.ts";
import { check } from "./settings.ts";
import type { Player, PlayerState, PlayerCommand, ShareAccess } from "../lib/schema.ts";
import { formatDuration } from "../lib/format.ts";

function access(player: Player) {
  const api = useServers().api()!;
  const panel = request(() => api.player.getShareAccess({ player_id: player.id }), info => {
    let ids = info.accounts.filter(account => account.selected).map(account => account.account_id);
    const mode = select("Who can control this device", [["everyone", "Everyone"], ["selected", "Selected accounts"]], info.access, () => undefined);
    return h("div", { class: "stack" }, mode, ...info.accounts.map(account => check(account.display_name ?? account.handle, ids.includes(account.account_id), on => { ids = on ? [...ids, account.account_id] : ids.filter(id => id !== account.account_id); })),
      button("Save access", async () => { await api.player.setShareAccess({ player_id: player.id, access: mode.value as ShareAccess, account_ids: ids }); dialog.close(); }, "primary"));
  });
  const dialog = modal(`Access to ${player.name}`, panel);
}
class DeviceCard extends BaseComponent {
  player!: Player;
  private state?: PlayerState;
  private titleNode = h("strong");
  private status = h("p", { class: "muted" });
  private now = h("p");
  private failure = h("div");
  private play = button("Play", () => this.send({ op: this.state?.status === "playing" ? "pause" : "play" }));
  private volume = h("input", { type: "range", min: 0, max: 100, "aria-label": "Device volume" });
  private async send(command: PlayerCommand) {
    if (command.op === "play" && this.player.audio_blocked) note("Tap Enable audio on the output device first.");
    await useServers().data()?.playerStates.control({ player_id: this.player.id, command });
  }
  connectedCallback(): void {
    super.connectedCallback(); this.className = "device-card";
    const pb = usePlayback();
    this.replaceChildren(this.titleNode, this.status, this.failure, this.now, h("div", { class: "actions" }, button("Previous", () => this.send({ op: "previous" }), "flat"), this.play, button("Next", () => this.send({ op: "next" }), "flat")),
      h("label", {}, "Volume", this.volume), h("div", { class: "actions" }, button("Play on this device", () => pb.setTarget(this.player.id), "primary"),
        pb.owned().includes(this.player.id) ? button("Stop sharing", () => pb.releaseDevice(this.player.id), "flat") : null,
        pb.owned().includes(this.player.id) && this.player.owner ? button("Manage access", () => access(this.player), "flat") : null));
    const off = useServers().data()?.playerStates.watch(this.player.id, state => { this.state = state; this.failure.replaceChildren(); this.invalidate(); }); if (off) this.own(off);
    void this.refresh();
    this.listen(this.volume, "change", () => { void this.send({ op: "volume", volume: Number(this.volume.value) }).catch(note); }); this.invalidate();
  }
  private async refresh(): Promise<void> {
    this.failure.replaceChildren();
    try { await useServers().data()?.playerStates.fetch(this.player.id); }
    catch (error) { if (this.isActive()) this.failure.replaceChildren(requestError(error, () => this.refresh())); }
  }
  render(): void {
    this.titleNode.textContent = this.player.name;
    this.status.textContent = this.player.audio_blocked ? "Tap this device to enable audio" : this.state?.status ?? "Connecting…";
    const item = this.state?.queue[this.state.current_index ?? -1];
    this.now.textContent = item ? `${item.title ?? item.track_id} · ${formatDuration(this.state?.position_ms ?? 0)}` : "No track queued";
    this.play.label = this.state?.status === "playing" ? "Pause" : "Play";
    if (document.activeElement !== this.volume) this.volume.value = String(this.state?.volume ?? 100);
  }
}
customElements.define("ichoi-device", DeviceCard);
class DeviceGrid extends BaseComponent {
  players: readonly Player[] = [];
  private cards = new Map<string, { signature: string; element: HTMLElement }>();
  connectedCallback(): void {
    super.connectedCallback(); this.className = "device-grid";
    const off = useServers().data()?.playerCatalog.watch(change => { this.players = change.players; this.invalidate(); });
    if (off) this.own(off);
    this.invalidate();
  }
  render(): void {
    const ids = new Set(this.players.map(player => player.id));
    for (const [id, card] of this.cards) if (!ids.has(id)) { card.element.remove(); this.cards.delete(id); }
    this.querySelector(".empty-state")?.remove();
    for (const player of this.players) {
      const signature = JSON.stringify(player), previous = this.cards.get(player.id);
      if (previous?.signature === signature) continue;
      const element = player.kind === "private"
        ? h("section", { class: "device-card" }, h("h3", {}, player.name), h("p", { class: "muted" }, "Private player"))
        : Object.assign(new DeviceCard(), { player });
      if (previous) previous.element.replaceWith(element); else this.append(element);
      this.cards.set(player.id, { signature, element });
    }
    if (!this.players.length) this.append(empty("No shared devices", "Share this browser or connect a satellite."));
  }
}
customElements.define("ichoi-device-grid", DeviceGrid);
export function devicesPage() {
  const servers = useServers(), pb = usePlayback(), api = servers.api();
  if (!api) return empty("Connect to a server first");
  const panel = request(() => api.player.listPlayers(), result => Object.assign(new DeviceGrid(), { players: result.players }));
  return h("div", { class: "page" }, heading("Devices", "Choose where your music plays. Each device has its own queue."), h("div", { class: "actions" },
    button("Share this browser", async () => {
      const name = await askText({ title: "Share this browser", label: "Device name", value: "Browser", required: true });
      if (!name) return;
      const result = await api.player.enableShare({ suffix: name }); pb.markOwned(result.player.id);
      await servers.data()?.playerCatalog.refresh(); pb.setTarget(result.player.id); await panel.refresh();
    }, "primary"), button("Refresh devices", async () => { await servers.data()?.playerCatalog.refresh(); await panel.refresh(); })), panel);
}
