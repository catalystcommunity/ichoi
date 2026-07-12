import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { AlbumTile } from "../components/AlbumTile.tsx";
import { TrackList } from "../components/TrackList.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";

export function SearchPage(): JSX.Element {
  const servers = useServers();
  const pb = usePlayback();
  const { t } = useI18n();
  const navigate = useNavigate();
  const [query, setQuery] = createSignal("");
  const [debounced, setDebounced] = createSignal("");
  let timer: ReturnType<typeof setTimeout> | undefined;

  const onInput = (value: string) => {
    setQuery(value);
    clearTimeout(timer);
    timer = setTimeout(() => setDebounced(value.trim()), 220);
  };

  const [results] = createResource(
    () => {
      const api = servers.api();
      const q = debounced();
      return api && q.length >= 1 ? { api, q } : undefined;
    },
    (input) => input.api.library.search({ query: input.q, limit: 50 }),
  );

  const hasResults = () => {
    const r = results();
    return !!r && (r.artists.length > 0 || r.albums.length > 0 || r.tracks.length > 0);
  };

  return (
    <div class="page">
      <header class="page-head">
        <div class="eyebrow">{t("nav.search")}</div>
        <h1 class="page-title">{t("search.title")}</h1>
      </header>

      <input
        class="input search-input"
        type="search"
        role="searchbox"
        aria-label={t("search.placeholder")}
        placeholder={t("search.placeholder")}
        value={query()}
        onInput={(e) => onInput(e.currentTarget.value)}
      />

      <div style={{ "margin-top": "22px" }}>
        <Show
          when={debounced().length >= 1}
          fallback={<EmptyState title={t("search.prompt")} />}
        >
          <Show when={!results.loading} fallback={<Spinner label={t("common.loading")} />}>
            <Show
              when={hasResults()}
              fallback={<EmptyState title={t("search.noResults", { query: debounced() })} />}
            >
              <Show when={results()!.artists.length}>
                <section aria-labelledby="s-artists">
                  <div class="section-head">
                    <h2 id="s-artists">{t("search.artists")}</h2>
                    <span class="count">{results()!.artists.length}</span>
                  </div>
                  <div class="grid">
                    <For each={results()!.artists}>
                      {(artist) => (
                        <button
                          type="button"
                          class="tile"
                          onClick={() => navigate(`/artist/${encodeURIComponent(artist.id)}`)}
                        >
                          <span class="cover">
                            <span class="cover-fallback">{artist.name.charAt(0).toUpperCase()}</span>
                          </span>
                          <span class="tile-title">{artist.name}</span>
                        </button>
                      )}
                    </For>
                  </div>
                </section>
              </Show>

              <Show when={results()!.albums.length}>
                <section aria-labelledby="s-albums">
                  <div class="section-head">
                    <h2 id="s-albums">{t("search.albums")}</h2>
                    <span class="count">{results()!.albums.length}</span>
                  </div>
                  <div class="grid">
                    <For each={results()!.albums}>{(album) => <AlbumTile album={album} />}</For>
                  </div>
                </section>
              </Show>

              <Show when={results()!.tracks.length}>
                <section aria-labelledby="s-tracks">
                  <div class="section-head">
                    <h2 id="s-tracks">{t("search.tracks")}</h2>
                    <span class="count">{results()!.tracks.length}</span>
                  </div>
                  <TrackList
                    tracks={results()!.tracks}
                    currentTrackId={pb.current()?.id}
                    playing={pb.snapshot().status === "playing"}
                    onPlay={(i) => void pb.playNow(results()!.tracks, i)}
                    onQueue={(i) => pb.enqueue([results()!.tracks[i]!])}
                  />
                </section>
              </Show>
            </Show>
          </Show>
        </Show>
      </div>
    </div>
  );
}
