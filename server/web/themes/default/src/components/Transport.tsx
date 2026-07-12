// The persistent transport bar — the signature "hardware strip" at the bottom.
// Drives the local (private) player: prev / play-pause / next, a scrubber, and a
// live signal meter that animates only while sound is actually flowing.
import { createEffect, For, Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { formatDuration, trackTechSummary } from "../lib/format.ts";
import { Meter } from "./common.tsx";
import { IconNext, IconPause, IconPlay, IconPrev } from "./Icons.tsx";

export function Transport(): JSX.Element {
  const pb = usePlayback();
  const { t } = useI18n();

  const track = () => pb.current();
  const targetLabel = () => {
    const id = pb.target();
    if (id === "local") return t("nowPlaying.localPlayer");
    return pb.sharedTargets().find((p) => p.id === id)?.name ?? id;
  };
  // Remote (shared-target) queue items carry only title/duration, not codec details.
  const techSummary = () => {
    const tr = track();
    return tr && tr.sample_rate > 0 ? trackTechSummary(tr) : undefined;
  };
  const status = () => pb.snapshot().status;
  const isPlaying = () => status() === "playing";
  const duration = () => pb.snapshot().durationMs ?? track()?.duration_ms ?? 0;
  const position = () => pb.snapshot().positionMs;
  const progressPct = () => {
    const d = duration();
    return d > 0 ? Math.min(100, (position() / d) * 100) : 0;
  };

  // Keep the native <select> showing the current target even after its options re-render
  // (a controlled select can drop its value when options change).
  let targetRef: HTMLSelectElement | undefined;
  createEffect(() => {
    const t = pb.target();
    pb.sharedTargets();
    if (targetRef && targetRef.value !== t) targetRef.value = t;
  });

  return (
    <footer class="transport" aria-label={t("nowPlaying.title")}>
      {/* Always-visible progress fill — the only progress indicator on mobile, where the
          full scrubber is hidden. */}
      <div
        class="transport-progress"
        style={{ width: `${progressPct()}%` }}
        aria-hidden="true"
      />
      <div class="transport-track">
        <div class="transport-cover" aria-hidden="true">
          <Meter live={isPlaying()} />
        </div>
        <div class="transport-meta">
          <Show
            when={track()}
            fallback={<div class="transport-title">{t("player.nothingPlaying")}</div>}
          >
            <div class="transport-title">{track()!.title}</div>
            <div class="transport-sub">
              <Show when={techSummary()}>{(s) => <>{s()} · </>}</Show>
              {targetLabel()}
            </div>
          </Show>
          <Show when={pb.snapshot().decoderMissing}>
            <div class="transport-sub" style={{ color: "var(--danger)" }}>
              {t("nowPlaying.decoderMissing")}
            </div>
          </Show>
          <select
            ref={targetRef}
            class="transport-target"
            aria-label={t("player.target")}
            value={pb.target()}
            onChange={(e) => pb.setTarget(e.currentTarget.value)}
          >
            <option value="local">{t("player.thisDevice")}</option>
            <For each={pb.sharedTargets()}>
              {(p) => <option value={p.id}>{p.name}</option>}
            </For>
          </select>
        </div>
      </div>

      <div class="transport-center">
        <div class="transport-buttons">
          <button
            type="button"
            class="btn btn-ghost btn-icon"
            aria-label={t("jukebox.previous")}
            onClick={() => void pb.previous()}
            disabled={!track()}
          >
            <IconPrev size={20} />
          </button>
          <button
            type="button"
            class="play-btn"
            aria-label={isPlaying() ? t("player.pause") : t("player.play")}
            aria-pressed={isPlaying()}
            onClick={() => pb.togglePlay()}
            disabled={!track() || pb.snapshot().decoderMissing}
          >
            <Show when={isPlaying()} fallback={<IconPlay size={22} />}>
              <IconPause size={22} />
            </Show>
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-icon"
            aria-label={t("jukebox.next")}
            onClick={() => void pb.next()}
            disabled={!track()}
          >
            <IconNext size={20} />
          </button>
        </div>
        <div class="scrub">
          <span class="mono">{formatDuration(position())}</span>
          <input
            class="slider"
            type="range"
            min={0}
            max={Math.max(duration(), 1)}
            value={Math.min(position(), duration())}
            aria-label={t("player.seek")}
            disabled={!track()}
            onInput={(e) => pb.seek(Number(e.currentTarget.value))}
          />
          <span class="mono">{formatDuration(duration())}</span>
        </div>
      </div>

      <div class="transport-right">
        <Show when={track()}>
          <span class="badge">{isPlaying() ? t("jukebox.onAir") : t("jukebox.idle")}</span>
        </Show>
      </div>
    </footer>
  );
}
