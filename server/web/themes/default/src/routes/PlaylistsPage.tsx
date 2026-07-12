import { createResource, createSignal, For, Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { TrackList } from "../components/TrackList.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";
import { IconPlay, IconPlaylist } from "../components/Icons.tsx";

export function PlaylistsPage(): JSX.Element {
  const servers = useServers();
  const pb = usePlayback();
  const { t } = useI18n();
  const [selected, setSelected] = createSignal<string>();

  const [list] = createResource(
    () => servers.api(),
    (api) => api.library.listPlaylists({ limit: 200 }),
  );

  const [detail] = createResource(
    () => {
      const api = servers.api();
      const id = selected();
      return api && id ? { api, id } : undefined;
    },
    (input) => input.api.library.getPlaylist({ playlist_id: input.id }),
  );

  return (
    <div class="page">
      <header class="page-head">
        <div class="eyebrow">{t("nav.playlists")}</div>
        <h1 class="page-title">{t("playlists.title")}</h1>
        <p class="page-sub">{t("playlists.note")}</p>
      </header>

      <Show when={servers.api()} fallback={<EmptyState title={t("errors.connectFirst")} />}>
        <Show when={!list.loading} fallback={<Spinner label={t("common.loading")} />}>
          <Show
            when={(list()?.playlists.length ?? 0) > 0}
            fallback={<EmptyState title={t("playlists.empty")} />}
          >
            <div class="strip-grid">
              <For each={list()!.playlists}>
                {(pl) => (
                  <button
                    type="button"
                    class="strip"
                    style={{ "text-align": "left", cursor: "pointer" }}
                    aria-pressed={selected() === pl.id}
                    onClick={() => setSelected(pl.id)}
                  >
                    <div class="strip-top">
                      <div>
                        <div class="strip-name">{pl.name}</div>
                        <div class="strip-where">
                          {t("playlists.entries", { count: pl.entry_count })}
                          <Show when={pl.owner}>{` · ${t("playlists.ownedBy", { owner: pl.owner! })}`}</Show>
                        </div>
                      </div>
                      <IconPlaylist size={20} />
                    </div>
                    <div class="chip mono">{pl.root_relative_path}</div>
                  </button>
                )}
              </For>
            </div>
          </Show>
        </Show>

        <Show when={selected()}>
          <div style={{ "margin-top": "26px" }}>
            <Show when={!detail.loading} fallback={<Spinner />}>
              <Show when={detail()}>
                {(d) => (
                  <>
                    <div class="section-head">
                      <h2>{d().playlist.name}</h2>
                      <button
                        type="button"
                        class="btn btn-primary"
                        onClick={() => void pb.playNow(d().tracks, 0)}
                        disabled={d().tracks.length === 0}
                      >
                        <IconPlay size={16} /> {t("album.playAll")}
                      </button>
                    </div>
                    <TrackList
                      tracks={d().tracks}
                      currentTrackId={pb.current()?.id}
                      playing={pb.snapshot().status === "playing"}
                      onPlay={(i) => void pb.playNow(d().tracks, i)}
                      onQueue={(i) => pb.enqueue([d().tracks[i]!])}
                    />
                  </>
                )}
              </Show>
            </Show>
          </div>
        </Show>
      </Show>
    </div>
  );
}
