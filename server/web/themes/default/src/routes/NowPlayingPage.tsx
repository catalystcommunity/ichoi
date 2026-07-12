import { For, Show, createSignal, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { EmptyState, Meter } from "../components/common.tsx";
import { formatDuration } from "../lib/format.ts";
import { IconStop } from "../components/Icons.tsx";

export function NowPlayingPage(): JSX.Element {
  const pb = usePlayback();
  const { t } = useI18n();
  const playing = () => pb.snapshot().status === "playing";

  const [dragIndex, setDragIndex] = createSignal<number | null>(null);
  const [overIndex, setOverIndex] = createSignal<number | null>(null);

  const drop = (to: number) => {
    const from = dragIndex();
    if (from !== null && from !== to) pb.move(from, to);
    setDragIndex(null);
    setOverIndex(null);
  };

  const rowState = (i: number): "played" | "current" | "upnext" => {
    if (i < pb.currentIndex()) return "played";
    if (i === pb.currentIndex()) return "current";
    return "upnext";
  };

  const progressPct = () => {
    const s = pb.snapshot();
    const d = s.durationMs ?? pb.current()?.duration_ms ?? 0;
    return d > 0 ? Math.min(100, (s.positionMs / d) * 100) : 0;
  };

  return (
    <div class="page">
      <header class="page-head row spread">
        <div>
          <div class="eyebrow">{t("nav.nowPlaying")}</div>
          <h1 class="page-title">{t("nowPlaying.title")}</h1>
        </div>
        <Show when={pb.queue.length > 0}>
          <button type="button" class="btn btn-ghost" onClick={() => pb.stop()}>
            <IconStop size={16} /> {t("nowPlaying.clear")}
          </button>
        </Show>
      </header>

      <Show
        when={pb.current()}
        fallback={<EmptyState title={t("nowPlaying.empty")} hint={t("nowPlaying.emptyHint")} />}
      >
        <div class="panel">
          <div class="row spread">
            <div>
              <div class="eyebrow">{t("nowPlaying.nowPlaying")}</div>
              <h2 style={{ "margin-top": "6px" }}>{pb.current()!.title}</h2>
              <p class="page-sub mono">
                {formatDuration(pb.snapshot().positionMs)} /{" "}
                {formatDuration(pb.snapshot().durationMs ?? pb.current()!.duration_ms)}
              </p>
            </div>
            <Meter live={playing()} />
          </div>
        </div>

        <div class="section-head">
          <h2>{t("nowPlaying.queue")}</h2>
          <span class="count">{pb.queue.length}</span>
        </div>

        <ul class="tracklist queue-list" role="list">
          <For each={pb.queue}>
            {(track, i) => (
              <li
                role="listitem"
                class="track-item queue-item"
                classList={{
                  "is-over": overIndex() === i(),
                  "is-current": rowState(i()) === "current",
                }}
                draggable={true}
                style={{
                  opacity: rowState(i()) === "played" ? "0.55" : "1",
                  "--progress": rowState(i()) === "current" ? `${progressPct()}%` : "0%",
                }}
                onDragStart={() => setDragIndex(i())}
                onDragOver={(e) => {
                  e.preventDefault();
                  setOverIndex(i());
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  drop(i());
                }}
                onDragEnd={() => {
                  setDragIndex(null);
                  setOverIndex(null);
                }}
              >
                <span class="drag-handle" aria-hidden="true" title="Drag to reorder">
                  ⠿
                </span>
                <button
                  type="button"
                  class="track-row"
                  aria-current={rowState(i()) === "current" ? "true" : undefined}
                  onClick={() => void pb.playIndex(i())}
                >
                  <span class="track-no" aria-hidden="true">
                    <Show when={rowState(i()) === "current" && playing()} fallback={i() + 1}>
                      <Meter live={true} />
                    </Show>
                  </span>
                  <span class="track-main">
                    <span class="track-title">{track.title}</span>
                    <span class="track-tech">
                      {rowState(i()) === "current"
                        ? t("nowPlaying.nowPlaying")
                        : rowState(i()) === "played"
                          ? t("nowPlaying.played")
                          : t("nowPlaying.upNext")}
                    </span>
                  </span>
                  <span class="track-dur">{formatDuration(track.duration_ms)}</span>
                </button>
                <button
                  type="button"
                  class="icon-btn"
                  aria-label={`Remove ${track.title} from queue`}
                  title={t("nowPlaying.clear")}
                  onClick={() => pb.removeAt(i())}
                >
                  ×
                </button>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
