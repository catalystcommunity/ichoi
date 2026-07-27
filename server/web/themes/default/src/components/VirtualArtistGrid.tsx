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
import type { Artist } from "../lib/schema.ts";
import type { ServerApi } from "../lib/services.ts";
import { useI18n } from "../lib/i18n.tsx";
import { EmptyState, Spinner } from "./common.tsx";

const PAGE_SIZE = 100;
const MIN_CARD_WIDTH = 168;
const COLUMN_GAP = 20;
const ROW_GAP = 22;
const CARD_HEIGHT = 260;
const OVERSCAN_ROWS = 3;

export function VirtualArtistGrid(props: { api: ServerApi }): JSX.Element {
  const { t } = useI18n();
  const navigate = useNavigate();
  let host!: HTMLDivElement;
  let scroller: HTMLElement | undefined;
  let resizeObserver: ResizeObserver | undefined;
  let generation = 0;

  const pages = new Map<number, Artist[]>();
  const inflight = new Map<number, Promise<void>>();
  const [revision, setRevision] = createSignal(0);
  const [total, setTotal] = createSignal(0);
  const [initialLoading, setInitialLoading] = createSignal(true);
  const [loadError, setLoadError] = createSignal<string>();
  const [columns, setColumns] = createSignal(1);
  const [rowHeight, setRowHeight] = createSignal(CARD_HEIGHT + ROW_GAP);
  const [viewportTop, setViewportTop] = createSignal(0);
  const [viewportHeight, setViewportHeight] = createSignal(800);

  const totalRows = createMemo(() => Math.ceil(total() / columns()));
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
    const end = Math.min(total(), rows.last * columns());
    return Array.from({ length: Math.max(0, end - start) }, (_, index) => start + index);
  });

  function updateViewport(): void {
    if (!host || !scroller) return;
    const hostRect = host.getBoundingClientRect();
    const scrollRect = scroller.getBoundingClientRect();
    setViewportTop(Math.max(0, scrollRect.top - hostRect.top));
    setViewportHeight(scrollRect.height);
  }

  function updateLayout(): void {
    if (!host) return;
    const width = host.clientWidth;
    const nextColumns = Math.max(1, Math.floor((width + COLUMN_GAP) / (MIN_CARD_WIDTH + COLUMN_GAP)));
    const cardWidth = (width - COLUMN_GAP * (nextColumns - 1)) / nextColumns;
    setColumns(nextColumns);
    setRowHeight(cardWidth + 92 + ROW_GAP);
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

  function artistAt(index: number): Artist | undefined {
    revision();
    return pages.get(Math.floor(index / PAGE_SIZE))?.[index % PAGE_SIZE];
  }

  function loadPage(page: number, activeGeneration: number): Promise<void> {
    if (page < 0 || pages.has(page)) return Promise.resolve();
    const existing = inflight.get(page);
    if (existing) return existing;
    const request = (async () => {
      try {
        const response = await props.api.library.listArtists({
          offset: page * PAGE_SIZE,
          limit: PAGE_SIZE,
        });
        if (activeGeneration !== generation) return;
        pages.set(page, response.artists);
        setTotal(response.total);
        setLoadError(undefined);
        setRevision((value) => value + 1);
      } catch (error) {
        if (activeGeneration === generation) {
          if (page === 0) setLoadError(String(error));
          else console.warn(`[library] artist page ${page} failed`, error);
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
    void api;
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
    const indexes = visibleIndexes();
    if (indexes.length === 0) return;
    const firstPage = Math.floor(indexes[0]! / PAGE_SIZE);
    const lastPage = Math.floor(indexes[indexes.length - 1]! / PAGE_SIZE);
    const activeGeneration = generation;
    for (let page = firstPage; page <= lastPage; page += 1) void loadPage(page, activeGeneration);
  });

  onCleanup(() => {
    generation += 1;
    scroller?.removeEventListener("scroll", updateViewport);
    resizeObserver?.disconnect();
  });

  return (
    <Show when={!initialLoading()} fallback={<Spinner label={t("library.loading")} />}>
      <Show when={!loadError()} fallback={<EmptyState title={t("errors.generic")} hint={loadError()} />}>
        <Show when={total() > 0} fallback={<EmptyState title={t("library.noArtists")} />}>
          <div
            ref={mountHost}
            class="virtual-album-grid"
            style={{ height: `${Math.max(0, totalRows() * rowHeight() - ROW_GAP)}px` }}
          >
            <div
              class="virtual-album-window"
              style={{
                top: `${visibleRows().first * rowHeight()}px`,
                "grid-template-columns": `repeat(${columns()}, minmax(0, 1fr))`,
                "grid-auto-rows": `${rowHeight() - ROW_GAP}px`,
              }}
            >
              <For each={visibleIndexes()}>
                {(index) => (
                  <Show
                    when={artistAt(index)}
                    fallback={
                      <div class="tile virtual-album-placeholder" aria-hidden="true">
                        <span class="cover" />
                        <span class="tile-sub">{t("library.loading")}</span>
                      </div>
                    }
                  >
                    {(artist) => (
                      <button
                        type="button"
                        class="tile"
                        onClick={() => navigate(`/artist/${encodeURIComponent(artist().id)}`)}
                        aria-label={artist().name}
                      >
                        <span class="cover">
                          <span class="cover-fallback">
                            {artist().name.charAt(0).toUpperCase()}
                          </span>
                        </span>
                        <span>
                          <span class="tile-title">{artist().name}</span>
                          <span class="tile-sub">
                            {t("library.albumsCount", { count: artist().album_count })}
                          </span>
                        </span>
                      </button>
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
