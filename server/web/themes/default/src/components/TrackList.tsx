// A keyboard-navigable list of tracks. Each row is a button (Enter/Space plays);
// an optional trailing button adds the track to the current queue.
import { For, Show, type JSX } from "solid-js";
import type { AudiobookProgress, Track } from "../lib/schema.ts";
import { codecLabel, formatDuration, isLossless, trackTechSummary } from "../lib/format.ts";
import { useI18n } from "../lib/i18n.tsx";
import { Meter } from "./common.tsx";
import { IconPlus } from "./Icons.tsx";

interface Props {
  tracks: Track[];
  currentTrackId?: string;
  playing?: boolean;
  onPlay: (index: number) => void;
  /** When provided, each row shows an "add to queue" button. */
  onQueue?: (index: number) => void;
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
                onClick={() => props.onPlay(i())}
              >
                <span class="track-no" aria-hidden="true">
                  <Show when={isCurrent() && props.playing} fallback={track.track_no ?? i() + 1}>
                    <Meter live={true} />
                  </Show>
                </span>
                <span class="track-main">
                  <span class="track-title">{track.title}</span>
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
              <Show when={props.onQueue}>
                <button
                  type="button"
                  class="icon-btn track-queue-btn"
                  aria-label={`Add ${track.title} to queue`}
                  title="Add to queue"
                  onClick={() => props.onQueue!(i())}
                >
                  <IconPlus size={16} />
                </button>
              </Show>
            </li>
          );
        }}
      </For>
    </ul>
  );
}
