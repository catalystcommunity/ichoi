import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { useToast } from "../stores/toasts.tsx";
import { TrackList } from "../components/TrackList.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";
import {
  copyTracksToInstance,
  searchAllInstances,
  type FederatedSearchResult,
  type FederationServer,
} from "../lib/federation.ts";
import type { Track } from "../lib/schema.ts";

export function SearchPage(): JSX.Element {
  const servers = useServers();
  const playback = usePlayback();
  const toast = useToast();
  const { t } = useI18n();
  const [query, setQuery] = createSignal("");
  const [debounced, setDebounced] = createSignal("");
  const [busy, setBusy] = createSignal<string>();
  let timer: ReturnType<typeof setTimeout> | undefined;

  const onInput = (value: string) => {
    setQuery(value);
    clearTimeout(timer);
    timer = setTimeout(() => setDebounced(value.trim()), 220);
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

  async function addTracks(source: FederationServer, tracks: Track[], play = false): Promise<void> {
    const target = destination();
    if (!target) throw new Error(t("errors.connectFirst"));
    if (source.id !== target.id && servers.active()?.session?.can_admin !== true) {
      throw new Error(t("search.importAdminRequired", { name: target.name }));
    }
    const localTracks = await copyTracksToInstance(source, target, tracks);
    if (play && localTracks[0]) await playback.enqueueAndPlay(localTracks[0]);
    else playback.enqueue(localTracks);
    toast.show(t("search.added", { count: localTracks.length, name: target.name }));
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

  async function addAlbum(result: FederatedSearchResult, albumId: string): Promise<void> {
    const detail = await result.server.api.library.getAlbum({ album_id: albumId });
    await addTracks(result.server, detail.tracks);
  }

  async function addArtist(result: FederatedSearchResult, artistId: string): Promise<void> {
    const artist = await result.server.api.library.getArtist({
      artist_id: artistId,
      library: result.library,
    });
    const tracks: Track[] = [];
    for (const album of artist.albums) {
      const detail = await result.server.api.library.getAlbum({ album_id: album.id });
      tracks.push(...detail.tracks);
    }
    await addTracks(result.server, tracks);
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
                              disabled={Boolean(busy())}
                              onClick={() => void runAction(
                                `${result.server.id}:artist:${artist.id}`,
                                () => addArtist(result, artist.id),
                              )}
                            >
                              <span class="cover"><span class="cover-fallback">{artist.name[0]?.toUpperCase()}</span></span>
                              <span class="tile-title">{artist.name}</span>
                              <span class="tile-sub">{t("search.queueArtist")}</span>
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
                              disabled={Boolean(busy())}
                              onClick={() => void runAction(
                                `${result.server.id}:album:${album.id}`,
                                () => addAlbum(result, album.id),
                              )}
                            >
                              <span class="cover"><span class="cover-fallback">{album.title[0]?.toUpperCase()}</span></span>
                              <span class="tile-title">{album.title}</span>
                              <Show when={album.artist_name}><span class="tile-sub">{album.artist_name}</span></Show>
                              <span class="tile-sub">{t("search.queueAlbum")}</span>
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
                        onPlay={(index) => void runAction(
                          `${result.server.id}:play:${result.response!.tracks[index]!.id}`,
                          () => addTracks(result.server, [result.response!.tracks[index]!], true),
                        )}
                        onQueue={(index) => void runAction(
                          `${result.server.id}:track:${result.response!.tracks[index]!.id}`,
                          () => addTracks(result.server, [result.response!.tracks[index]!]),
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
