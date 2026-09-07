import { createResource, createSignal, Show, type JSX } from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import { useI18n } from "../lib/i18n.tsx";
import { useServers } from "../stores/servers.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { CoverArt } from "../components/CoverArt.tsx";
import { TrackList } from "../components/TrackList.tsx";
import { EmptyState, Spinner } from "../components/common.tsx";
import { IconChevronLeft, IconPlay, IconPlus } from "../components/Icons.tsx";
import { formatDuration } from "../lib/format.ts";
import { copyTracksToInstance, type FederationServer } from "../lib/federation.ts";
import type { Track } from "../lib/schema.ts";
import { useToast } from "../stores/toasts.tsx";

export function AlbumPage(): JSX.Element {
  const params = useParams();
  const navigate = useNavigate();
  const servers = useServers();
  const pb = usePlayback();
  const toast = useToast();
  const { t } = useI18n();
  const [busy, setBusy] = createSignal(false);

  const source = (): FederationServer | undefined => {
    const record = params.serverId
      ? servers.servers.find((server) => server.id === params.serverId)
      : servers.active();
    if (record?.state !== "ready") return undefined;
    const api = record && servers.apiFor(record.id);
    return record && api ? { id: record.id, name: record.name, api } : undefined;
  };

  const destination = (): FederationServer | undefined => {
    const record = servers.active();
    if (record?.state !== "ready") return undefined;
    const api = record && servers.apiFor(record.id);
    return record && api ? { id: record.id, name: record.name, api } : undefined;
  };

  const [detail] = createResource(
    () => {
      const api = source()?.api;
      return api && params.id ? { api, id: params.id } : undefined;
    },
    (input) => input.api.library.getAlbum({ album_id: input.id }),
  );

  const totalMs = () => detail()?.tracks.reduce((sum, tr) => sum + tr.duration_ms, 0) ?? 0;

  async function localTracks(tracks: Track[]): Promise<Track[]> {
    const from = source();
    const to = destination();
    if (!from || !to) throw new Error(t("errors.connectFirst"));
    if (from.id !== to.id && servers.active()?.session?.can_admin !== true) {
      throw new Error(t("search.importAdminRequired", { name: to.name }));
    }
    return copyTracksToInstance(from, to, tracks);
  }

  async function run(action: (tracks: Track[]) => void | Promise<void>, tracks: Track[]): Promise<void> {
    if (busy()) return;
    setBusy(true);
    try {
      await action(await localTracks(tracks));
    } catch (error) {
      toast.show(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="page">
      <button
        type="button"
        class="btn btn-ghost"
        onClick={() => params.serverId ? navigate(-1) : navigate("/")}
      >
        <IconChevronLeft size={16} /> {t("album.back")}
      </button>

      <Show when={!detail.loading} fallback={<div style={{ "margin-top": "30px" }}><Spinner /></div>}>
        <Show when={detail()} fallback={<EmptyState title={t("errors.generic")} />}>
          {(d) => (
            <>
              <header class="row" style={{ gap: "26px", "align-items": "flex-end", margin: "22px 0 26px" }}>
                <CoverArt album={d().album} class="album-hero-cover" serverId={params.serverId} />
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
                      disabled={busy()}
                      onClick={() => void run((tracks) => pb.playNow(tracks, 0), d().tracks)}
                    >
                      <IconPlay size={16} /> {t("album.playAll")}
                    </button>
                    <button
                      type="button"
                      class="btn"
                      disabled={busy()}
                      onClick={() => void run((tracks) => pb.enqueue(tracks), d().tracks)}
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
                onQueue={(i) => void run((tracks) => pb.enqueue(tracks), [d().tracks[i]!])}
                onPlayNext={(i) => void run((tracks) => pb.playNext(tracks[0]!), [d().tracks[i]!])}
                onPlayNow={(i) => void run((tracks) => pb.playNow(tracks, 0), [d().tracks[i]!])}
              />
            </>
          )}
        </Show>
      </Show>
    </div>
  );
}
