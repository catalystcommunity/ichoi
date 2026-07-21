import { createResource, Show, type JSX } from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { CoverArt } from "../components/CoverArt.tsx";
import { TrackList } from "../components/TrackList.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";
import { IconChevronLeft, IconPlay, IconPlus } from "../components/Icons.tsx";
import { formatDuration } from "../lib/format.ts";

export function AlbumPage(): JSX.Element {
  const params = useParams();
  const navigate = useNavigate();
  const servers = useServers();
  const pb = usePlayback();
  const { t } = useI18n();

  const [detail] = createResource(
    () => {
      const api = servers.api();
      return api && params.id ? { api, id: params.id } : undefined;
    },
    (input) => input.api.library.getAlbum({ album_id: input.id }),
  );

  const totalMs = () => detail()?.tracks.reduce((sum, tr) => sum + tr.duration_ms, 0) ?? 0;

  return (
    <div class="page">
      <button type="button" class="btn btn-ghost" onClick={() => navigate("/")}>
        <IconChevronLeft size={16} /> {t("album.back")}
      </button>

      <Show when={!detail.loading} fallback={<div style={{ "margin-top": "30px" }}><Spinner /></div>}>
        <Show when={detail()} fallback={<EmptyState title={t("errors.generic")} />}>
          {(d) => (
            <>
              <header class="row" style={{ gap: "26px", "align-items": "flex-end", margin: "22px 0 26px" }}>
                <CoverArt album={d().album} class="album-hero-cover" />
                <div style={{ flex: "1", "min-width": "0" }}>
                  <div class="eyebrow">{t("library.albums")}</div>
                  <h1 class="page-title" style={{ margin: "6px 0" }}>
                    {d().album.title}
                  </h1>
                  <Show when={d().album.artist_name}>
                    {(artist) => <p class="page-sub">{artist()}</p>}
                  </Show>
                  <p class="page-sub mono">
                    {t("library.tracksCount", { count: d().album.track_count })} ·{" "}
                    {formatDuration(totalMs())}
                    <Show when={d().album.year}>{` · ${d().album.year}`}</Show>
                  </p>
                  <div class="row" style={{ "margin-top": "16px" }}>
                    <button
                      type="button"
                      class="btn btn-primary"
                      onClick={() => void pb.playNow(d().tracks, 0)}
                    >
                      <IconPlay size={16} /> {t("album.playAll")}
                    </button>
                    <button
                      type="button"
                      class="btn"
                      onClick={() => pb.enqueue(d().tracks)}
                      aria-label={t("album.queueAll")}
                    >
                      <IconPlus size={16} /> {t("album.queueAll")}
                    </button>
                  </div>
                </div>
              </header>

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
  );
}
