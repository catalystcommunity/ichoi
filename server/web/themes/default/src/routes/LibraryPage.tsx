import { createSignal, Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { VirtualAlbumGrid } from "../components/VirtualAlbumGrid.tsx";
import { VirtualArtistGrid } from "../components/VirtualArtistGrid.tsx";
import { EmptyState } from "../components/common.tsx";

type Tab = "albums" | "artists";
type AlbumView = "grid" | "list";
type AlbumSort = "title" | "artist" | "year" | "tracks";

export function LibraryPage(): JSX.Element {
  const servers = useServers();
  const { t } = useI18n();
  const [tab, setTab] = createSignal<Tab>("albums");
  const [albumView, setAlbumView] = createSignal<AlbumView>("grid");
  const [albumQuery, setAlbumQuery] = createSignal("");
  const [albumSort, setAlbumSort] = createSignal<AlbumSort>("title");

  return (
    <div class="page catalog-page">
      <header class="page-head">
        <div class="eyebrow">{t("nav.library")}</div>
        <div class="row spread">
          <h1 class="page-title">{t("library.title")}</h1>
          <div class="segmented" role="group" aria-label={t("library.title")}>
            <button aria-pressed={tab() === "albums"} onClick={() => setTab("albums")}>
              {t("library.albums")}
            </button>
            <button aria-pressed={tab() === "artists"} onClick={() => setTab("artists")}>
              {t("library.artists")}
            </button>
          </div>
        </div>
      </header>

      <div class="catalog-body">
        <Show when={servers.api()} fallback={<EmptyState title={t("errors.connectFirst")} />}>
          {(api) => (
            <>
              <div class="catalog-pane" hidden={tab() !== "albums"}>
                <div class="library-view-controls">
                  <div class="segmented" role="group" aria-label={t("library.view")}>
                    <button
                      aria-pressed={albumView() === "grid"}
                      onClick={() => setAlbumView("grid")}
                    >
                      {t("library.grid")}
                    </button>
                    <button
                      aria-pressed={albumView() === "list"}
                      onClick={() => setAlbumView("list")}
                    >
                      {t("library.list")}
                    </button>
                  </div>
                  <Show when={albumView() === "list"}>
                    <input
                      class="input library-filter"
                      type="search"
                      value={albumQuery()}
                      placeholder={t("library.filterAlbums")}
                      aria-label={t("library.filterAlbums")}
                      onInput={(event) => setAlbumQuery(event.currentTarget.value)}
                    />
                    <select
                      class="select library-sort"
                      value={albumSort()}
                      aria-label={t("library.sortAlbums")}
                      onChange={(event) => setAlbumSort(event.currentTarget.value as AlbumSort)}
                    >
                      <option value="title">{t("library.sortTitle")}</option>
                      <option value="artist">{t("library.sortArtist")}</option>
                      <option value="year">{t("library.sortYear")}</option>
                      <option value="tracks">{t("library.sortTracks")}</option>
                    </select>
                  </Show>
                </div>
                <div class="catalog-scroll">
                  <VirtualAlbumGrid
                    api={api()}
                    view={albumView()}
                    query={albumQuery()}
                    sort={albumSort()}
                  />
                </div>
              </div>
              <div class="catalog-pane" hidden={tab() !== "artists"}>
                <div class="catalog-scroll">
                  <VirtualArtistGrid api={api()} />
                </div>
              </div>
            </>
          )}
        </Show>
      </div>
    </div>
  );
}
