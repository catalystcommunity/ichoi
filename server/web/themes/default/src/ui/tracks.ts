import { ListView } from "@catalystcommunity/data-views";
import { BaseComponent, h, button } from "./dom.ts";
import type { Track } from "../lib/schema.ts";
import { formatDuration } from "../lib/format.ts";
import { usePlayback } from "../stores/playback.ts";
import { useServers } from "../stores/servers.ts";
import { copyTracksToInstance } from "../lib/federation.ts";

export async function localTracks(tracks: Track[], serverId?: string): Promise<Track[]> {
  const servers = useServers();
  const destination = servers.active();
  const source = serverId ? servers.servers.find(server => server.id === serverId) : destination;
  const from = source && servers.apiFor(source.id), to = destination && servers.apiFor(destination.id);
  if (!from || !to || !source || !destination) throw new Error("Connect to a server first.");
  if (source.id !== destination.id && !destination.session?.can_admin) throw new Error(`An administrator must import tracks to ${destination.name}.`);
  return copyTracksToInstance({ ...source, api: from }, { ...destination, api: to }, tracks);
}
export class TrackRows extends BaseComponent {
  tracks: Track[] = [];
  serverId?: string;
  queueMode = false;
  private view = new ListView<Track>();
  private lastQueue?: Track[];
  private lastCurrent = -2;
  connectedCallback(): void {
    super.connectedCallback();
    this.view.label = this.queueMode ? "Playback queue" : "Tracks";
    this.view.emptyText = this.queueMode ? "Your queue is empty. Add tracks from the library." : "No tracks";
    const resize = new ResizeObserver(() => {
      const compact = this.clientWidth < 520;
      this.classList.toggle("compact-tracks", compact);
      this.view.rowHeight = compact ? 116 : 76;
    });
    resize.observe(this); this.own(() => resize.disconnect());
    this.view.rowHeight = 76;
    // Queue duplicates have separate positions. The server owns queue item identities.
    this.view.keyOf = (track, index) => this.queueMode ? index : track.id;
    this.view.itemText = track => track.title;
    this.view.renderItem = (track, row, index) => {
      const pb = usePlayback();
      const now = this.queueMode ? index === pb.currentIndex() : track.id === pb.current()?.id;
      const name = h("div", { class: "track-copy" }, h("strong", {}, track.title || "Untitled track"),
        h("span", { class: "muted" }, [track.artist_name, formatDuration(track.duration_ms)].filter(Boolean).join(" · ")));
      const actions = h("div", { class: "track-actions" });
      const play = button("Play", async () => {
        if (this.queueMode) await pb.playIndex(index);
        else await pb.playNow(await localTracks([track], this.serverId));
      });
      play.setAttribute("aria-label", `Play ${track.title}`);
      actions.append(play);
      if (this.queueMode) {
        const up = button("↑", () => this.move(index, index - 1));
        up.setAttribute("aria-label", `Move ${track.title} up`); up.disabled = index === 0;
        const down = button("↓", () => this.move(index, index + 1));
        down.setAttribute("aria-label", `Move ${track.title} down`); down.disabled = index === this.tracks.length - 1;
        const remove = button("×", () => { pb.removeAt(index); this.restoreFocus(index); });
        remove.setAttribute("aria-label", `Remove ${track.title}`);
        actions.append(up, down, remove);
      } else {
        const next = button("Next", async () => pb.playNext((await localTracks([track], this.serverId))[0]!));
        next.setAttribute("aria-label", `Play ${track.title} next`);
        const add = button("+", async () => pb.enqueue(await localTracks([track], this.serverId)));
        add.setAttribute("aria-label", `Add ${track.title} to queue`);
        actions.append(next, add);
      }
      actions.addEventListener("keydown", event => event.stopPropagation());
      actions.addEventListener("click", event => event.stopPropagation());
      row.replaceChildren(h("span", { class: "track-number", "aria-hidden": "true" }, now ? "♫" : String(index + 1)), name, actions);
      row.classList.add("track-row"); row.classList.toggle("is-current", now);
    };
    this.replaceChildren(this.view);
    this.own(usePlayback().subscribe(() => this.invalidate()));
    this.listen(this.view, "list-view-activate", event => {
      if (event.detail.pointer) return;
      const index = event.detail.index;
      if (typeof index === "number") this.view.rowElement(index)?.querySelector<HTMLElement>("push-button")?.focus();
    });
    this.invalidate();
  }
  private move(from: number, to: number): void { usePlayback().move(from, to); this.restoreFocus(to); }
  private restoreFocus(index: number): void { requestAnimationFrame(() => { if (this.isActive()) { this.view.scrollToIndex(index); this.view.focus(); } }); }
  render(): void {
    const pb = usePlayback();
    if (this.queueMode) this.tracks = pb.queue;
    if (this.lastQueue !== this.tracks) { this.view.items = this.tracks; this.lastQueue = this.tracks; }
    if (this.lastCurrent !== pb.currentIndex()) { this.lastCurrent = pb.currentIndex(); this.view.refresh(); }
  }
}
customElements.define("ichoi-tracks", TrackRows);
export function tracks(items: Track[], serverId?: string) { const element = new TrackRows(); element.tracks = items; element.serverId = serverId; return element; }
