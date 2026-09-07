import { createResource, createSignal, For, onCleanup, Show, type JSX } from "solid-js";
import { useNavigate, useSearchParams } from "@solidjs/router";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { useToast } from "../stores/toasts.tsx";
import { TrackList } from "../components/TrackList.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";
import {
  copyTracksToInstance,
  federatedDetailRoute,
  searchAllInstances,
  type FederationServer,
} from "../lib/federation.ts";
import type { Track } from "../lib/schema.ts";

export function SearchPage(): JSX.Element {
  const servers = useServers();
  const playback = usePlayback();
  const toast = useToast();
  const { t } = useI18n();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const initialQuery = typeof searchParams.q === "string" ? searchParams.q : "";
  const [query, setQuery] = createSignal(initialQuery);
  const [debounced, setDebounced] = createSignal(initialQuery.trim());
  const [busy, setBusy] = createSignal<string>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => clearTimeout(timer));

  const onInput = (value: string) => {
    setQuery(value);
    clearTimeout(timer);
    timer = setTimeout(() => {
      const next = value.trim();
      setDebounced(next);
      setSearchParams({ q: next || undefined }, { replace: true });
    }, 220);
  };

  const connectedInstances = (): FederationServer[] =>
    servers.servers.flatMap((server) => {
      const api = server.state === "ready" ? servers.apiFor(server.id) : undefined;
      return api ? [{ id: server.id, name: server.name, api }] : [];
    });

  const destination = (): FederationServer | undefined => {
    const record = servers.active();
    const api = record && servers.apiFor(record.id);
    return record && api ? { id: record.id, name: record.name, api } : undefined;
  };

  const [results] = createResource(
    () => {
      const q = debounced();
      const instances = connectedInstances();
      return q && instances.length ? { q, instances } : undefined;
    },
    (input) => searchAllInstances(input.instances, input.q),
  );

  const visibleResults = () =>
    (results() ?? []).filter((result) => {
      const response = result.response;
      return response && (response.artists.length || response.albums.length || response.tracks.length);
    });

  async function addTracks(
    source: FederationServer,
    tracks: Track[],
    action: "queue" | "next" | "now",
  ): Promise<void> {
    const target = destination();
    if (!target) throw new Error(t("errors.connectFirst"));
    if (source.id !== target.id && servers.active()?.session?.can_admin !== true) {
      throw new Error(t("search.importAdminRequired", { name: target.name }));
    }
    const localTracks = await copyTracksToInstance(source, target, tracks);
    const first = localTracks[0];
    if (action === "now" && first) await playback.playNow([first], 0);
    else if (action === "next" && first) playback.playNext(first);
    else playback.enqueue(localTracks);
  }

  async function runAction(key: string, action: () => Promise<void>): Promise<void> {
    if (busy()) return;
    setBusy(key);
    try {
      await action();
    } catch (error) {
      toast.show(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(undefined);
    }
  }

  return (
    <div class="page">
      <header class="page-head">
        <div class="eyebrow">{t("nav.search")}</div>
        <h1 class="page-title">{t("search.title")}</h1>
        <Show when={servers.active()}>
          {(server) => <p class="page-sub">{t("search.destination", { name: server().name })}</p>}
        </Show>
      </header>

      <input
        class="input search-input"
        type="search"
        role="searchbox"
        aria-label={t("search.placeholder")}
        placeholder={t("search.placeholder")}
        value={query()}
        onInput={(event) => onInput(event.currentTarget.value)}
      />

      <div style={{ "margin-top": "22px" }}>
        <Show when={debounced()} fallback={<EmptyState title={t("search.prompt")} />}>
          <Show when={!results.loading} fallback={<Spinner label={t("common.loading")} />}>
            <Show
              when={visibleResults().length}
              fallback={<EmptyState title={t("search.noResults", { query: debounced() })} />}
            >
              <For each={visibleResults()}>
                {(result) => (
                  <section class="federated-results" aria-label={`${result.server.name} ${result.library}`}>
                    <div class="section-head">
                      <h2>{result.server.name}</h2>
                      <span class="badge">{t(`search.${result.library}`)}</span>
                    </div>

                    <Show when={result.response!.artists.length}>
                      <h3>{t("search.artists")}</h3>
                      <div class="grid">
                        <For each={result.response!.artists}>
                          {(artist) => (
                            <button
                              type="button"
                              class="tile"
                              onClick={() => navigate(federatedDetailRoute(
                                result.server.id,
                                result.library,
                                "artist",
                                artist.id,
                              ))}
                            >
                              <span class="cover"><span class="cover-fallback">{artist.name[0]?.toUpperCase()}</span></span>
                              <span class="tile-title">{artist.name}</span>
                              <span class="tile-sub">{t("search.viewArtist")}</span>
                            </button>
                          )}
                        </For>
                      </div>
                    </Show>

                    <Show when={result.response!.albums.length}>
                      <h3>{result.library === "audiobook" ? t("search.audiobooks") : t("search.albums")}</h3>
                      <div class="grid">
                        <For each={result.response!.albums}>
                          {(album) => (
                            <button
                              type="button"
                              class="tile"
                              onClick={() => navigate(federatedDetailRoute(
                                result.server.id,
                                result.library,
                                "album",
                                album.id,
                              ))}
                            >
                              <span class="cover"><span class="cover-fallback">{album.title[0]?.toUpperCase()}</span></span>
                              <span class="tile-title">{album.title}</span>
                              <Show when={album.artist_name}><span class="tile-sub">{album.artist_name}</span></Show>
                              <span class="tile-sub">{t("search.viewAlbum")}</span>
                            </button>
                          )}
                        </For>
                      </div>
                    </Show>

                    <Show when={result.response!.tracks.length}>
                      <h3>{t("search.tracks")}</h3>
                      <TrackList
                        tracks={result.response!.tracks}
                        currentTrackId={playback.current()?.id}
                        playing={playback.snapshot().status === "playing"}
                        onQueue={(index) => void runAction(
                          `${result.server.id}:track:${result.response!.tracks[index]!.id}`,
                          () => addTracks(result.server, [result.response!.tracks[index]!], "queue"),
                        )}
                        onPlayNext={(index) => void runAction(
                          `${result.server.id}:next:${result.response!.tracks[index]!.id}`,
                          () => addTracks(result.server, [result.response!.tracks[index]!], "next"),
                        )}
                        onPlayNow={(index) => void runAction(
                          `${result.server.id}:now:${result.response!.tracks[index]!.id}`,
                          () => addTracks(result.server, [result.response!.tracks[index]!], "now"),
                        )}
                      />
                    </Show>
                  </section>
                )}
              </For>
            </Show>
          </Show>
        </Show>
      </div>
    </div>
  );
}
