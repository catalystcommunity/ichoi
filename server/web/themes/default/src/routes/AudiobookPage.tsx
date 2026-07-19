import { createMemo, createResource, Show, type JSX } from "solid-js";
import { useNavigate, useParams } from "@solidjs/router";
import { CoverArt } from "../components/CoverArt.tsx";
import { TrackList } from "../components/TrackList.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";
import { IconChevronLeft, IconPlay, IconPlus } from "../components/Icons.tsx";
import { formatDuration } from "../lib/format.ts";
import { useI18n } from "../lib/i18n.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { useServers } from "../stores/servers.tsx";

export function AudiobookPage(): JSX.Element {
  const params = useParams();
  const navigate = useNavigate();
  const servers = useServers();
  const playback = usePlayback();
  const { t } = useI18n();

  const [detail] = createResource(
    () => {
      const api = servers.api();
      return api && params.id ? { api, id: params.id } : undefined;
    },
    (input) => input.api.library.getAlbum({ album_id: input.id }),
  );
  const [progress] = createResource(
    () => {
      const api = servers.api();
      const tracks = detail()?.tracks;
      const session = servers.active()?.session;
      return api && tracks && session ? { api, tracks } : undefined;
    },
    (input) => input.api.library.getAudiobookProgress({ track_ids: input.tracks.map((t) => t.id) }),
  );
  const progressMap = createMemo(
    () => new Map((progress()?.progress ?? []).map((item) => [item.track_id, item])),
  );
  const resume = createMemo(() => {
    const tracks = detail()?.tracks ?? [];
    const recent = progress()?.progress.find((item) => !item.completed && item.position_ms > 0);
    const index = recent ? tracks.findIndex((track) => track.id === recent.track_id) : 0;
    return { index: Math.max(0, index), position: recent?.position_ms ?? 0 };
  });
  const totalMs = () => detail()?.tracks.reduce((sum, track) => sum + track.duration_ms, 0) ?? 0;

  return (
    <div class="page">
      <button type="button" class="btn btn-ghost" onClick={() => navigate("/audiobooks")}>
        <IconChevronLeft size={16} /> {t("audiobooks.back")}
      </button>

      <Show when={!detail.loading} fallback={<div style={{ "margin-top": "30px" }}><Spinner /></div>}>
        <Show when={detail()} fallback={<EmptyState title={t("errors.generic")} />}>
          {(book) => (
            <>
              <header class="row audiobook-hero">
                <CoverArt album={book().album} class="album-hero-cover" />
                <div class="audiobook-hero-copy">
                  <div class="eyebrow">{t("nav.audiobooks")}</div>
                  <h1 class="page-title">{book().album.title}</h1>
                  <p class="page-sub mono">
                    {t("audiobooks.chapters", { count: book().album.track_count })} · {formatDuration(totalMs())}
                  </p>
                  <Show when={!servers.active()?.session}>
                    <p class="page-sub">{t("audiobooks.signInProgress")}</p>
                  </Show>
                  <Show when={servers.active()?.session?.role === "guest"}>
                    <p class="page-sub">{t("audiobooks.guestProgress")}</p>
                  </Show>
                  <div class="row audiobook-actions">
                    <button
                      type="button"
                      class="btn btn-primary"
                      onClick={() => void playback.playNow(book().tracks, resume().index, resume().position)}
                    >
                      <IconPlay size={16} />
                      {resume().position > 0 ? t("audiobooks.resume") : t("audiobooks.play")}
                    </button>
                    <button type="button" class="btn" onClick={() => playback.enqueue(book().tracks)}>
                      <IconPlus size={16} /> {t("audiobooks.queue")}
                    </button>
                  </div>
                </div>
              </header>

              <TrackList
                tracks={book().tracks}
                currentTrackId={playback.current()?.id}
                playing={playback.snapshot().status === "playing"}
                audiobookProgress={progressMap()}
                onPlay={(index) => {
                  const saved = progressMap().get(book().tracks[index]!.id);
                  void playback.playNow(book().tracks, index, saved?.completed ? 0 : saved?.position_ms ?? 0);
                }}
                onQueue={(index) => playback.enqueue([book().tracks[index]!])}
              />
            </>
          )}
        </Show>
      </Show>
    </div>
  );
}
