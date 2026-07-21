import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  Show,
  type JSX,
} from "solid-js";
import type { Album } from "../lib/schema.ts";
import type { ServerApi } from "../lib/services.ts";
import { useI18n } from "../lib/i18n.tsx";
import { EmptyState, Spinner } from "./common.tsx";
import { AlbumTile } from "./AlbumTile.tsx";

const PAGE_SIZE = 100;
const MIN_CARD_WIDTH = 168;
const COLUMN_GAP = 20;
const ROW_GAP = 22;
const CARD_TEXT_HEIGHT = 92;
const OVERSCAN_ROWS = 3;
const RETAIN_PAGE_RADIUS = 2;

export function VirtualAlbumGrid(props: { api: ServerApi }): JSX.Element {
  const { t } = useI18n();
  let host!: HTMLDivElement;
  let scroller: HTMLElement | undefined;
  let resizeObserver: ResizeObserver | undefined;
  let generation = 0;

  const pages = new Map<number, Album[]>();
  const inflight = new Set<number>();
  const [revision, setRevision] = createSignal(0);
  const [total, setTotal] = createSignal(0);
  const [initialLoading, setInitialLoading] = createSignal(true);
  const [loadError, setLoadError] = createSignal<string>();
  const [columns, setColumns] = createSignal(1);
  const [rowHeight, setRowHeight] = createSignal(MIN_CARD_WIDTH + CARD_TEXT_HEIGHT + ROW_GAP);
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
    setRowHeight(cardWidth + CARD_TEXT_HEIGHT + ROW_GAP);
    updateViewport();
  }

  function initializeHost(element: HTMLDivElement): void {
    host = element;
    scroller?.removeEventListener("scroll", updateViewport);
    resizeObserver?.disconnect();
    scroller = host.closest(".main") as HTMLElement | undefined;
    scroller?.addEventListener("scroll", updateViewport, { passive: true });
    resizeObserver = new ResizeObserver(updateLayout);
    resizeObserver.observe(host);
    updateLayout();
  }

  function albumAt(index: number): Album | undefined {
    revision();
    return pages.get(Math.floor(index / PAGE_SIZE))?.[index % PAGE_SIZE];
  }

  function evictDistantPages(firstNeeded: number, lastNeeded: number): void {
    for (const page of pages.keys()) {
      if (page < firstNeeded - RETAIN_PAGE_RADIUS || page > lastNeeded + RETAIN_PAGE_RADIUS) {
        pages.delete(page);
      }
    }
  }

  async function loadPage(page: number, activeGeneration: number): Promise<void> {
    if (page < 0 || pages.has(page) || inflight.has(page)) return;
    inflight.add(page);
    try {
      const response = await props.api.library.listAlbums({
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
    void loadPage(0, activeGeneration);
  });

  createEffect(() => {
    const indexes = visibleIndexes();
    if (indexes.length === 0) return;
    const firstPage = Math.floor(indexes[0]! / PAGE_SIZE);
    const lastPage = Math.floor(indexes[indexes.length - 1]! / PAGE_SIZE);
    const activeGeneration = generation;
    for (let page = firstPage; page <= lastPage; page += 1) void loadPage(page, activeGeneration);
    evictDistantPages(firstPage, lastPage);
    setRevision((value) => value + 1);
  });

  onCleanup(() => {
    generation += 1;
    scroller?.removeEventListener("scroll", updateViewport);
    resizeObserver?.disconnect();
  });

  return (
    <Show when={!initialLoading()} fallback={<Spinner label={t("library.loading")} />}>
      <Show when={!loadError()} fallback={<EmptyState title={t("errors.generic")} hint={loadError()} />}>
        <Show when={total() > 0} fallback={<EmptyState title={t("library.noAlbums")} />}>
          <div
            ref={initializeHost}
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
                    when={albumAt(index)}
                    fallback={
                      <div class="tile virtual-album-placeholder" aria-hidden="true">
                        <span class="cover" />
                        <span class="tile-sub">{t("library.loading")}</span>
                      </div>
                    }
                  >
                    {(album) => <AlbumTile album={album()} />}
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
