import { savedScroll, rememberScroll } from "../lib/browse-memory.ts";
import { useServers } from "../stores/servers.tsx";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  Show,
  type JSX,
} from "solid-js";
import { useNavigate } from "@solidjs/router";
import type { Album, Library } from "../lib/schema.ts";
import type { ServerApi } from "../lib/services.ts";
import { useI18n } from "../lib/i18n.tsx";
import { CoverArt } from "./CoverArt.tsx";
import { EmptyState, Spinner } from "./common.tsx";
import { AlbumTile } from "./AlbumTile.tsx";

const PAGE_SIZE = 100;
const MIN_CARD_WIDTH = 168;
const COLUMN_GAP = 20;
const ROW_GAP = 22;
const CARD_TEXT_HEIGHT = 92;
const OVERSCAN_ROWS = 3;
const LIST_ROW_HEIGHT = 76;

type AlbumSort = "title" | "artist" | "year" | "tracks";

export function VirtualAlbumGrid(props: {
  api: ServerApi;
  view?: "grid" | "list";
  query?: string;
  sort?: AlbumSort;
  library?: Library;
  detailPath?: "album" | "audiobook";
}): JSX.Element {
  const { t } = useI18n();
  const servers = useServers();
  const scrollKey = () => (servers.activeId() ?? "") + "|catalog|" +  (props.library ?? "music") + "|albums|" + (props.view ?? "grid");
  let restorePosition: number | undefined = savedScroll(scrollKey());
  const navigate = useNavigate();
  let host!: HTMLDivElement;
  let scroller: HTMLElement | undefined;
  let resizeObserver: ResizeObserver | undefined;
  let generation = 0;

  const pages = new Map<number, Album[]>();
  const inflight = new Map<number, Promise<void>>();
  const [revision, setRevision] = createSignal(0);
  const [total, setTotal] = createSignal(0);
  const [initialLoading, setInitialLoading] = createSignal(true);
  const [loadError, setLoadError] = createSignal<string>();
  const [columns, setColumns] = createSignal(1);
  const [rowHeight, setRowHeight] = createSignal(MIN_CARD_WIDTH + CARD_TEXT_HEIGHT + ROW_GAP);
  const [viewportTop, setViewportTop] = createSignal(0);
  const [viewportHeight, setViewportHeight] = createSignal(800);
  const allAlbums = createMemo(() => {
    revision();
    if (pages.size < Math.ceil(total() / PAGE_SIZE)) return undefined;
    return Array.from({ length: total() }, (_, index) => albumAt(index)).filter(
      (album): album is Album => album !== undefined,
    );
  });
  const listAlbums = createMemo(() => {
    const albums = allAlbums();
    if (!albums) return undefined;
    const query = props.query?.trim().toLocaleLowerCase() ?? "";
    const filtered = query
      ? albums.filter((album) =>
          [album.title, album.artist_name, album.year?.toString()]
            .filter(Boolean)
            .some((value) => value!.toLocaleLowerCase().includes(query)),
        )
      : [...albums];
    const sort = props.sort ?? "title";
    filtered.sort((left, right) => {
      if (sort === "artist") {
        return (left.artist_name ?? "").localeCompare(right.artist_name ?? "")
          || left.title.localeCompare(right.title);
      }
      if (sort === "year") {
        return (left.year ?? Number.MAX_SAFE_INTEGER) - (right.year ?? Number.MAX_SAFE_INTEGER)
          || left.title.localeCompare(right.title);
      }
      if (sort === "tracks") {
        return right.track_count - left.track_count || left.title.localeCompare(right.title);
      }
      return left.title.localeCompare(right.title);
    });
    return filtered;
  });
  const displayTotal = createMemo(() =>
    props.view === "list" ? (listAlbums()?.length ?? total()) : total(),
  );

  const totalRows = createMemo(() => Math.ceil(displayTotal() / columns()));
  const visibleRows = createMemo(() => {
    const height = rowHeight();
    const first = Math.max(0, Math.floor(viewportTop() / height) - OVERSCAN_ROWS);
    const last = Math.min(
      totalRows(),
      Math.ceil((viewportTop() + viewportHeight()) / height) + OVERSCAN_ROWS,
    );
    return { first, last };
  });
  const visibleIndexes = createMemo(() => {
    const rows = visibleRows();
    const start = rows.first * columns();
    const end = Math.min(displayTotal(), rows.last * columns());
    return Array.from({ length: Math.max(0, end - start) }, (_, index) => start + index);
  });

  function updateViewport(): void {
    if (!host || !scroller) return;
    if (restorePosition !== undefined) {
      scroller.scrollTop = restorePosition;
      if (Math.abs(scroller.scrollTop - restorePosition) < 2) restorePosition = undefined;
    } else rememberScroll(scrollKey(), scroller.scrollTop);
    const hostRect = host.getBoundingClientRect();
    const scrollRect = scroller.getBoundingClientRect();
    setViewportTop(Math.max(0, scrollRect.top - hostRect.top));
    setViewportHeight(scrollRect.height);
  }

  function updateLayout(): void {
    if (!host) return;
    if (props.view === "list") {
      setColumns(1);
      setRowHeight(LIST_ROW_HEIGHT);
      updateViewport();
      return;
    }
    const width = host.clientWidth;
    const nextColumns = Math.max(1, Math.floor((width + COLUMN_GAP) / (MIN_CARD_WIDTH + COLUMN_GAP)));
    const cardWidth = (width - COLUMN_GAP * (nextColumns - 1)) / nextColumns;
    setColumns(nextColumns);
    setRowHeight(cardWidth + CARD_TEXT_HEIGHT + ROW_GAP);
    updateViewport();
  }

  function initializeHost(): void {
    scroller?.removeEventListener("scroll", updateViewport);
    resizeObserver?.disconnect();
    scroller = (host.closest(".catalog-scroll") ?? host.closest(".main-route")) as
      | HTMLElement
      | undefined;
    scroller?.addEventListener("scroll", updateViewport, { passive: true });
    resizeObserver = new ResizeObserver(updateLayout);
    resizeObserver.observe(host);
    updateLayout();
  }

  function mountHost(element: HTMLDivElement): void {
    host = element;
    queueMicrotask(() => {
      if (host === element && element.isConnected) initializeHost();
    });
  }

  function albumAt(index: number): Album | undefined {
    revision();
    return pages.get(Math.floor(index / PAGE_SIZE))?.[index % PAGE_SIZE];
  }

  function displayedAlbumAt(index: number): Album | undefined {
    return props.view === "list" ? listAlbums()?.[index] : albumAt(index);
  }

  function loadPage(page: number, activeGeneration: number): Promise<void> {
    if (page < 0 || pages.has(page)) return Promise.resolve();
    const existing = inflight.get(page);
    if (existing) return existing;
    const request = (async () => {
      try {
        const response = await props.api.library.listAlbums({
          library: props.library,
          offset: page * PAGE_SIZE,
          limit: PAGE_SIZE,
        });
        if (activeGeneration !== generation) return;
        pages.set(page, response.albums);
        setTotal(response.total);
        setLoadError(undefined);
        setRevision((value) => value + 1);
      } catch (error) {
        if (activeGeneration === generation) {
          if (page === 0) setLoadError(String(error));
          else console.warn(`[library] album page ${page} failed`, error);
        }
      } finally {
        inflight.delete(page);
        if (page === 0 && activeGeneration === generation) setInitialLoading(false);
      }
    })();
    inflight.set(page, request);
    return request;
  }

  async function loadCatalog(activeGeneration: number): Promise<void> {
    await loadPage(0, activeGeneration);
    if (activeGeneration !== generation) return;
    const pageCount = Math.ceil(total() / PAGE_SIZE);
    for (let page = 1; page < pageCount; page += 1) {
      await loadPage(page, activeGeneration);
      if (activeGeneration !== generation) return;
    }
  }

  createEffect(() => {
    const api = props.api;
    const library = props.library;
    void api;
    void library;
    generation += 1;
    const activeGeneration = generation;
    pages.clear();
    inflight.clear();
    setTotal(0);
    setLoadError(undefined);
    setInitialLoading(true);
    setRevision((value) => value + 1);
    void loadCatalog(activeGeneration);
  });

  createEffect(() => {
    props.view;
    const indexes = visibleIndexes();
    if (indexes.length === 0) return;
    const firstPage = Math.floor(indexes[0]! / PAGE_SIZE);
    const lastPage = Math.floor(indexes[indexes.length - 1]! / PAGE_SIZE);
    const activeGeneration = generation;
    for (let page = firstPage; page <= lastPage; page += 1) void loadPage(page, activeGeneration);
    updateLayout();
  });

  onCleanup(() => {
    generation += 1;
    scroller?.removeEventListener("scroll", updateViewport);
    resizeObserver?.disconnect();
  });

  return (
    <Show when={!initialLoading()} fallback={<Spinner label={t("library.loading")} />}>
      <Show when={!loadError()} fallback={<EmptyState title={t("errors.generic")} hint={loadError()} />}>
        <Show when={displayTotal() > 0} fallback={<EmptyState title={t("library.noAlbums")} />}>
          <div
            ref={mountHost}
            class={`virtual-album-grid ${props.view === "list" ? "album-list" : ""}`}
            style={{
              height: `${Math.max(
                0,
                totalRows() * rowHeight() - (props.view === "list" ? 8 : ROW_GAP),
              )}px`,
            }}
          >
            <div
              class="virtual-album-window"
              style={{
                top: `${visibleRows().first * rowHeight()}px`,
                "grid-template-columns": `repeat(${columns()}, minmax(0, 1fr))`,
                "grid-auto-rows": `${rowHeight() - (props.view === "list" ? 8 : ROW_GAP)}px`,
              }}
            >
              <For each={visibleIndexes()}>
                {(index) => (
                  <Show
                    when={displayedAlbumAt(index)}
                    fallback={
                      <div
                        class={props.view === "list"
                          ? "album-list-row virtual-album-placeholder"
                          : "tile virtual-album-placeholder"}
                        aria-hidden="true"
                      >
                        <span class={props.view === "list" ? "album-list-cover" : "cover"} />
                        <span class="tile-sub">{t("library.loading")}</span>
                      </div>
                    }
                  >
                    {(album) => (
                      <Show
                        when={props.view === "list"}
                        fallback={
                          <AlbumTile
                            album={album()}
                            href={`/${props.detailPath ?? "album"}/${encodeURIComponent(album().id)}`}
                          />
                        }
                      >
                          <button
                            type="button"
                            class="album-list-row"
                            onClick={() =>
                              navigate(`/${props.detailPath ?? "album"}/${encodeURIComponent(album().id)}`)
                            }
                            aria-label={`${album().title}${album().artist_name ? `, ${album().artist_name}` : ""}`}
                          >
                            <span class="album-list-cover"><CoverArt album={album()} /></span>
                            <span class="album-list-title">{album().title}</span>
                            <span class="album-list-artist">{album().artist_name ?? "Various"}</span>
                            <span class="album-list-year">{album().year ?? "—"}</span>
                            <span class="album-list-tracks">
                              {t("library.tracksCount", { count: album().track_count })}
                            </span>
                          </button>
                      </Show>
                    )}
                  </Show>
                )}
              </For>
            </div>
          </div>
        </Show>
      </Show>
    </Show>
  );
}
