import { h, heading, button, request, empty, link } from "./dom.ts";
import { cover, editArtwork } from "./artwork.ts";
import { tracks, localTracks } from "./tracks.ts";
import { albumTile } from "./catalog.ts";
import { reportContent } from "./settings.ts";
import { useServers } from "../stores/servers.ts";
import { usePlayback } from "../stores/playback.ts";
import { formatDuration } from "../lib/format.ts";
import { isSignedIn } from "../lib/compliance.ts";
import type { Library } from "../lib/schema.ts";
import { federatedDetailRoute } from "../lib/federation.ts";

export function detailPage(kind: "album" | "artist", id: string, library: Library, serverId?: string) {
  const servers = useServers(), pb = usePlayback();
  const record = serverId ? servers.servers.find(server => server.id === serverId) : servers.active();
  const api = record && servers.apiFor(record.id);
  if (!api) return empty("Connect to this server first");
  const page = h("div", { class: "page" }, link("← Back to library", library === "audiobook" ? "/audiobooks" : "/"));
  const canEdit = record.session?.can_edit_library || record.session?.can_admin;
  if (kind === "artist") {
    page.append(request(() => api.library.getArtist({ artist_id: id, library }), result => h("div", {},
      h("header", { class: "album-hero" }, cover(api, result.artist.id, result.artist.name, "artist"), h("div", {}, heading(result.artist.name, `${result.artist.album_count} albums`), canEdit ? button("Edit artwork", () => editArtwork(api, id, result.artist.name, "artist")) : null)),
      h("div", { class: "album-grid" }, ...result.albums.map(album => albumTile(album, api, serverId ? federatedDetailRoute(serverId, library, "album", album.id) : `${library === "audiobook" ? "/audiobook/" : "/album/"}${encodeURIComponent(album.id)}`))))));
  } else {
    page.append(request(() => api.library.getAlbum({ album_id: id }), result => {
      const content = h("div");
      const actions = h("div", { class: "actions" }, button("Play all", async () => pb.playNow(await localTracks(result.tracks, serverId)), "primary"),
        button("Add to queue", async () => pb.enqueue(await localTracks(result.tracks, serverId))), canEdit ? button("Edit artwork", () => editArtwork(api, id, result.album.title), "flat") : null);
      content.append(h("header", { class: "album-hero" }, cover(api, id, result.album.title), h("div", {}, h("p", { class: "eyebrow" }, library === "audiobook" ? "Audiobook" : "Album"), heading(result.album.title, result.album.artist_name),
        h("p", { class: "muted" }, `${result.tracks.length} ${library === "audiobook" ? "chapters" : "tracks"} · ${formatDuration(result.tracks.reduce((sum, track) => sum + track.duration_ms, 0))}${result.album.year ? ` · ${result.album.year}` : ""}`), actions)));
      if (library === "audiobook") content.append(request(() => api.library.getAudiobookProgress({ track_ids: result.tracks.map(track => track.id) }), progress => {
        const recent = progress.progress.find(item => !item.completed && item.position_ms > 0);
        const index = recent ? result.tracks.findIndex(track => track.id === recent.track_id) : -1;
        if (index < 0 || !recent) return h("p", { class: "muted" }, "No saved listening position.");
        return h("div", { class: "resume-card" }, h("span", {}, `Continue ${result.tracks[index]!.title} at ${formatDuration(recent.position_ms)}`),
          button("Resume book", async () => pb.playNow(await localTracks(result.tracks, serverId), index, recent.position_ms), "primary"));
      }));
      content.append(tracks(result.tracks, serverId)); return content;
    }));
  }
  return page;
}
export function playlistsPage() {
  const servers = useServers(), api = servers.api(), pb = usePlayback();
  if (!api) return empty("Connect to a server first");
  const selected = h("div");
  const list = request(() => api.library.listAllPlaylists(), result => {
    if (!result.playlists.length) return empty("No playlists yet", "Save your queue as a playlist to keep a mix.");
    return h("div", { class: "playlist-grid" }, ...result.playlists.map(playlist => button(`${playlist.name} · ${playlist.entry_count} tracks`, () => {
      selected.replaceChildren(request(() => api.library.getPlaylist({ playlist_id: playlist.id }), detail => {
        const actions = h("div", { class: "actions" }, button("Play all", () => pb.playNow(detail.tracks), "primary"), button("Add to queue", () => pb.enqueue(detail.tracks)));
        if (isSignedIn(servers.active()?.session)) {
          actions.append(button("Report playlist", () => reportContent(api, "playlist", playlist.id), "flat"));
          if (playlist.owner) {
            actions.append(button("Report user", () => reportContent(api, "account", playlist.owner!), "flat"));
            if (playlist.owner !== servers.active()?.session?.account_id) actions.append(button("Block user", async () => { await api.library.blockAccount({ account_id: playlist.owner! }); selected.replaceChildren(); await list.refresh(); }, "flat"));
          }
        }
        return h("section", {}, h("h2", {}, detail.playlist.name), actions, tracks(detail.tracks));
      }));
    })));
  });
  return h("div", { class: "page" }, heading("Playlists", "Your saved mixes and shared playlists"), list, selected);
}
