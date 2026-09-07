// A keyboard-navigable list of tracks. Activating a row appends it to the queue.
import { For, Show, type JSX } from "solid-js";
import type { AudiobookProgress, Track } from "../lib/schema.ts";
import { codecLabel, formatDuration, isLossless, trackTechSummary } from "../lib/format.ts";
import { useI18n } from "../lib/i18n.tsx";
import { Meter } from "./common.tsx";
import { IconMoreVertical } from "./Icons.tsx";

interface Props {
  tracks: Track[];
  currentTrackId?: string;
  playing?: boolean;
  onQueue: (index: number) => void;
  onPlayNext: (index: number) => void;
  onPlayNow: (index: number) => void;
  audiobookProgress?: Map<string, AudiobookProgress>;
}

export function TrackList(props: Props): JSX.Element {
  const { t } = useI18n();
  return (
    <ul class="tracklist" role="list">
      <For each={props.tracks}>
        {(track, i) => {
          const isCurrent = () => track.id === props.currentTrackId;
          const progress = () => props.audiobookProgress?.get(track.id);
          const percent = () => {
            const p = progress();
            if (!p || track.duration_ms <= 0) return 0;
            return p.completed ? 100 : Math.min(100, (p.position_ms / track.duration_ms) * 100);
          };
          return (
            <li role="listitem" class="track-item">
              <button
                type="button"
                class="track-row"
                aria-current={isCurrent() ? "true" : undefined}
                aria-label={t("queue.addTrack", { title: track.title })}
                title={t("queue.addTrack", { title: track.title })}
                onClick={() => props.onQueue(i())}
              >
                <span class="track-no" aria-hidden="true">
                  <Show when={isCurrent() && props.playing} fallback={track.track_no ?? i() + 1}>
                    <Meter live={true} />
                  </Show>
                </span>
                <span class="track-main">
                  <span class="track-title">{track.title}</span>
                  <Show when={track.artist_name || track.album_title}>
                    <span class="track-context">
                      {[track.artist_name, track.album_title].filter(Boolean).join(" · ")}
                    </span>
                  </Show>
                  <span class="track-tech">{trackTechSummary(track)}</span>
                  <Show when={progress()}>
                    {(p) => (
                      <span class="audiobook-track-progress">
                        <span class="audiobook-progress-bar">
                          <span style={{ width: `${percent()}%` }} />
                        </span>
                        {p().completed ? t("audiobooks.completed") : `${Math.round(percent())}%`}
                      </span>
                    )}
                  </Show>
                </span>
                <Show when={isLossless(track.codec)}>
                  <span class="badge lossless">{codecLabel(track.codec)}</span>
                </Show>
                <span class="track-dur">{formatDuration(track.duration_ms)}</span>
              </button>
              <details class="track-menu">
                <summary
                  class="icon-btn"
                  aria-label={t("queue.moreActions", { title: track.title })}
                  title={t("queue.moreActions", { title: track.title })}
                >
                  <IconMoreVertical size={16} />
                </summary>
                <div class="track-menu-popover" role="menu">
                  <button
                    type="button"
                    role="menuitem"
                    onClick={(event) => {
                      props.onPlayNext(i());
                      event.currentTarget.closest("details")?.removeAttribute("open");
                    }}
                  >
                    {t("queue.playNext")}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={(event) => {
                      props.onPlayNow(i());
                      event.currentTarget.closest("details")?.removeAttribute("open");
                    }}
                  >
                    {t("queue.playNow")}
                  </button>
                </div>
              </details>
            </li>
          );
        }}
      </For>
    </ul>
  );
}
