import { virtualWindow } from "@catalystcommunity/data-views";
import { BaseComponent, h, empty, loading, requestError, heading, select, request } from "./dom.ts";
import { cover } from "./artwork.ts";
import { tracks } from "./tracks.ts";
import { useServers } from "../stores/servers.ts";
import type { ServerApi } from "../lib/services.ts";
import type { Album, Artist, Library } from "../lib/schema.ts";
import { savedScroll, rememberScroll, savedView, rememberView, savedQuery, rememberQuery } from "../lib/browse-memory.ts";
import { searchAllInstances, federatedDetailRoute } from "../lib/federation.ts";

export function albumTile(album: Album, api: ServerApi, href: string) {
  return h("a", { class: "album-tile", href }, cover(api, album.id, album.title),
    h("strong", { title: album.title }, album.title), h("span", { class: "muted" }, album.artist_name || `${album.track_count} tracks`));
}
const PAGE = 60;
type CatalogItem = Album | Artist;
export class Catalog extends BaseComponent {
  api!: ServerApi;
  library: Library = "music";
  kind: "albums" | "artists" = "albums";
  view: "grid" | "list" = "grid";
  sort = "title";
  memoryKey = "";
  private pages = new Map<number, CatalogItem[]>();
  private pending = new Set<number>();
  private errors = new Map<number, unknown>();
  private total = 0;
  private generation = 0;
  private viewport = h("div", { class: "catalog-viewport", tabIndex: 0, "aria-label": "Library catalog" });
  private spacer = h("div", { class: "catalog-spacer" });
  private status = h("div", { class: "catalog-status", "aria-live": "polite" });
  private rows = new Map<number, HTMLElement>();
  private signature = "";
  private restore = true;
  connectedCallback(): void {
    super.connectedCallback(); this.classList.add("catalog");
    this.viewport.append(this.spacer); this.replaceChildren(this.status, this.viewport);
    this.listen(this.viewport, "scroll", () => { rememberScroll(this.memoryKey, this.viewport.scrollTop); this.invalidate(); }, { passive: true });
    const resize = new ResizeObserver(() => this.invalidate()); resize.observe(this.viewport); this.own(() => resize.disconnect());
    void this.load(0);
  }
  disconnectedCallback(): void { ++this.generation; this.pending.clear(); super.disconnectedCallback(); }
  async retry(): Promise<void> { const failed = [...this.errors.keys()]; this.errors.clear(); await Promise.all(failed.map(page => this.load(page))); }
  private async load(page: number): Promise<void> {
    if (this.pages.has(page) || this.pending.has(page) || this.errors.has(page)) return;
    const generation = this.generation; this.pending.add(page); this.invalidate();
    try {
      const query = { library: this.library, offset: page * PAGE, limit: PAGE };
      const result = this.kind === "artists" ? await this.api.library.listArtists(query) : await this.api.library.listAlbums(query);
      if (!this.isActive() || generation !== this.generation) return;
      this.pages.set(page, "artists" in result ? result.artists : result.albums); this.total = result.total;
    } catch (error) { if (this.isActive() && generation === this.generation) this.errors.set(page, error); }
    finally { if (generation === this.generation) { this.pending.delete(page); this.invalidate(); } }
  }
  render(): void {
    this.status.replaceChildren(this.errors.size ? requestError(this.errors.values().next().value, () => this.retry()) :
      !this.pages.has(0) ? loading() : h("span", { class: "muted" }, `${this.total.toLocaleString()} ${this.kind}`));
    if (!this.pages.has(0)) return;
    if (!this.total) { this.spacer.replaceChildren(empty(`No ${this.kind} in this library`)); return; }
    if (this.view === "list" && !this.pending.size && !this.errors.size) {
      for (let page = 0; page * PAGE < this.total; page++) if (!this.pages.has(page)) { void this.load(page); break; }
    }
    const width = this.viewport.clientWidth || 700;
    const columns = this.view === "list" ? 1 : Math.max(2, Math.floor((width + 24) / 190));
    const rowHeight = this.view === "list" ? 80 : Math.floor((width - (columns - 1) * 24) / columns) + 92;
    const window = virtualWindow({ count: Math.ceil(this.total / columns), rowHeight,
      viewport: this.viewport.clientHeight || 600, scrollTop: this.restore ? savedScroll(this.memoryKey) : this.viewport.scrollTop, buffer: 2 });
    this.spacer.style.height = `${window.height}px`;
    if (this.restore) { this.viewport.scrollTop = savedScroll(this.memoryKey); this.restore = false; }
    const signature = `${columns}|${rowHeight}|${this.view}|${this.sort}`;
    if (this.signature !== signature) { this.rows.clear(); this.spacer.replaceChildren(); this.signature = signature; }
    const all = this.view === "list" ? [...this.pages.entries()].sort((a, b) => a[0] - b[0]).flatMap(([, items]) => items).sort((a, b) => {
      if (this.sort === "year") return ((b as Album).year ?? 0) - ((a as Album).year ?? 0);
      if (this.sort === "tracks") return ((b as Album).track_count ?? 0) - ((a as Album).track_count ?? 0);
      const name = (item: CatalogItem) => "name" in item ? item.name : this.sort === "artist" ? item.artist_name ?? "" : item.title;
      return name(a).localeCompare(name(b));
    }) : undefined;
    const visible = new Set<number>();
    for (let index = window.first * columns; index < Math.min(this.total, window.last * columns); index++) {
      visible.add(index);
      const page = Math.floor(index / PAGE);
      if (this.view === "grid") void this.load(page);
      const item = all ? all[index] : this.pages.get(page)?.[index % PAGE];
      const key = item?.id ?? "pending";
      let tile = this.rows.get(index);
      if (!tile || tile.dataset.key !== key) {
        const title = item ? "name" in item ? item.name : item.title : "Loading…";
        const path = this.kind === "artists" ? "/artist/" : this.library === "audiobook" ? "/audiobook/" : "/album/";
        const content = item ? this.kind === "artists"
          ? h("a", { class: "album-tile", href: `${path}${encodeURIComponent(item.id)}` }, cover(this.api, item.id, title, "artist"), h("strong", {}, title), h("span", { class: "muted" }, `${(item as Artist).album_count} albums`))
          : albumTile(item as Album, this.api, `${path}${encodeURIComponent(item.id)}`)
          : h("div", { class: "album-tile skeleton", "aria-hidden": "true" });
        const next = h("div", { class: this.view === "list" ? "catalog-cell list-cell" : "catalog-cell" }, content);
        next.dataset.key = key;
        if (tile) tile.replaceWith(next); else this.spacer.append(next);
        tile = next; this.rows.set(index, tile);
      }
      tile.style.width = `${(width - (columns - 1) * 24) / columns}px`;
      tile.style.transform = `translate(${(index % columns) * ((width + 24) / columns)}px, ${window.top(Math.floor(index / columns))}px)`;
    }
    for (const [index, tile] of this.rows) if (!visible.has(index) && !tile.contains(document.activeElement)) { tile.remove(); this.rows.delete(index); }
  }
}
customElements.define("ichoi-catalog", Catalog);

export function searchResults(query: string) {
  const servers = useServers();
  const instances = servers.servers.flatMap(server => {
    const api = server.state === "ready" ? servers.apiFor(server.id) : undefined;
    return api ? [{ ...server, api }] : [];
  });
  const panel = request(() => searchAllInstances(instances, query), results => {
    const sections = h("div", { class: "search-results" });
    for (const result of results) {
      const { server, library, response } = result;
      if (result.error) { sections.append(h("section", {}, h("h2", {}, `${server.name} · ${library}`), requestError(result.error, () => panel.refresh()))); continue; }
      if (!response || !(response.albums.length + response.artists.length + response.tracks.length)) continue;
      const grid = h("div", { class: "album-grid" }, ...response.albums.map(album => albumTile(album, server.api, federatedDetailRoute(server.id, library, "album", album.id))));
      for (const artist of response.artists) grid.append(h("a", { class: "album-tile", href: federatedDetailRoute(server.id, library, "artist", artist.id) }, cover(server.api, artist.id, artist.name, "artist"), h("strong", {}, artist.name)));
      sections.append(h("section", {}, h("h2", {}, `${server.name} · ${library === "music" ? "Music" : "Audiobooks"}`), grid,
        response.tracks.length ? tracks(response.tracks, server.id) : null));
    }
    if (!sections.childElementCount) sections.append(empty(`No results for “${query}”`, "Try an artist, album, book, or track title."));
    return sections;
  });
  return panel;
}
export class LibraryPage extends BaseComponent {
  library: Library = "music";
  searchOnly = false;
  initialQuery = "";
  connectedCallback(): void {
    super.connectedCallback();
    const servers = useServers(), api = servers.api();
    if (!api) { this.replaceChildren(empty("Connect to a server", "Open Settings to add your server.")); return; }
    this.className = "library-page";
    const key = `${servers.activeId()}|${this.library}`;
    const query = this.initialQuery || (this.searchOnly ? savedQuery(servers.activeId() ?? "") : savedView(`${key}|query`, ""));
    const input = h("input", { type: "search", class: "library-search", "aria-label": "Search library", placeholder: "Search artists, albums, books, and tracks", value: query });
    const result = h("div", { class: "library-results" });
    const catalog = new Catalog(); Object.assign(catalog, { api, library: this.library, memoryKey: `${key}|catalog`, kind: savedView(`${key}|kind`, "albums"), view: savedView(`${key}|view`, "grid"), sort: savedView(`${key}|sort`, "title") });
    const viewSelect = select("Library view", [["grid", "Covers"], ["list", "List"]], catalog.view, value => { catalog.view = value as "grid" | "list"; rememberView(`${key}|view`, value); catalog.invalidate(); });
    const controls = h("div", { class: "browse-controls" },
      select("Browse", [["albums", this.library === "music" ? "Albums" : "Books"], ["artists", this.library === "music" ? "Artists" : "Authors"]], catalog.kind, value => {
        rememberView(`${key}|kind`, value); this.connectedRefresh();
      }),
      viewSelect,
      select("Sort albums", [["title", "Title"], ["artist", "Artist"], ["year", "Year"], ["tracks", "Track count"]], catalog.sort, value => {
        catalog.sort = value; catalog.view = "list"; viewSelect.value = "list"; rememberView(`${key}|sort`, value); rememberView(`${key}|view`, "list"); catalog.invalidate();
      }));
    const show = () => {
      const text = input.value.trim(); rememberQuery(servers.activeId() ?? "", text); rememberView(`${key}|query`, text);
      catalog.hidden = Boolean(text) || this.searchOnly; controls.hidden = Boolean(text) || this.searchOnly;
      result.replaceChildren(text ? searchResults(text) : this.searchOnly ? empty("Find your next listen", "Search by artist, album, book, or track title.") : h("span"));
      result.hidden = !text && !this.searchOnly;
    };
    let timer: ReturnType<typeof setTimeout>;
    this.listen(input, "input", () => { clearTimeout(timer); timer = setTimeout(show, 220); }); this.own(() => clearTimeout(timer));
    this.replaceChildren(heading(this.searchOnly ? "Search" : this.library === "music" ? "Your library" : "Audiobooks"), input, controls, result);
    if (!this.searchOnly) this.append(catalog);
    show();
  }
  private connectedRefresh(): void { const next = new LibraryPage(); next.library = this.library; this.replaceWith(next); }
}
customElements.define("ichoi-library", LibraryPage);
