import { createResource, createSignal, For, Match, Show, Switch, type JSX } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { AlbumTile } from "../components/AlbumTile.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";

type Tab = "albums" | "artists";

export function LibraryPage(): JSX.Element {
  const servers = useServers();
  const { t } = useI18n();
  const navigate = useNavigate();
  const [tab, setTab] = createSignal<Tab>("albums");

  const [albums] = createResource(
    () => (tab() === "albums" ? servers.api() : undefined),
    (api) => api!.library.listAlbums({ limit: 200 }),
  );
  const [artists] = createResource(
    () => (tab() === "artists" ? servers.api() : undefined),
    (api) => api!.library.listArtists({ limit: 200 }),
  );

  return (
    <div class="page">
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

      <Show when={servers.api()} fallback={<EmptyState title={t("errors.connectFirst")} />}>
        <Switch>
          <Match when={tab() === "albums"}>
            <Show when={!albums.loading} fallback={<Spinner label={t("library.loading")} />}>
              <Show
                when={(albums()?.albums.length ?? 0) > 0}
                fallback={<EmptyState title={t("library.noAlbums")} />}
              >
                <div class="grid">
                  <For each={albums()!.albums}>{(album) => <AlbumTile album={album} />}</For>
                </div>
              </Show>
            </Show>
          </Match>

          <Match when={tab() === "artists"}>
            <Show when={!artists.loading} fallback={<Spinner label={t("library.loading")} />}>
              <Show
                when={(artists()?.artists.length ?? 0) > 0}
                fallback={<EmptyState title={t("library.noArtists")} />}
              >
                <div class="grid">
                  <For each={artists()!.artists}>
                    {(artist) => (
                      <button
                        type="button"
                        class="tile"
                        onClick={() => navigate(`/artist/${encodeURIComponent(artist.id)}`)}
                        aria-label={artist.name}
                      >
                        <span class="cover">
                          <span class="cover-fallback">
                            {artist.name.charAt(0).toUpperCase()}
                          </span>
                        </span>
                        <span>
                          <span class="tile-title">{artist.name}</span>
                          <span class="tile-sub">
                            {t("library.albumsCount", { count: artist.album_count })}
                          </span>
                        </span>
                      </button>
                    )}
                  </For>
                </div>
              </Show>
            </Show>
          </Match>
        </Switch>
      </Show>
    </div>
  );
}
