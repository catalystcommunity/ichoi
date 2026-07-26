// The persistent transport bar — the signature "hardware strip" at the bottom.
// Drives the local (private) player: prev / play-pause / next, a scrubber, and a
// live signal meter that animates only while sound is actually flowing.
import { createEffect, createSignal, For, Show, type JSX } from "solid-js";
import { useI18n } from "../lib/i18n.tsx";
import { usePlayback } from "../stores/playback.tsx";
import { formatDuration, trackTechSummary } from "../lib/format.ts";
import { Meter } from "./common.tsx";
import {
  IconNext,
  IconPause,
  IconPlay,
  IconPrev,
  IconRepeat,
  IconShuffle,
} from "./Icons.tsx";
import { satelliteToken } from "../lib/satellite-mode.ts";

export function Transport(): JSX.Element {
  const pb = usePlayback();
  const { t } = useI18n();
  const satelliteMode = Boolean(satelliteToken());

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
  const [seeking, setSeeking] = createSignal(false);
  const [seekPosition, setSeekPosition] = createSignal(0);
  const displayedPosition = () => seeking() ? seekPosition() : position();
  const progressPct = () => {
    const d = duration();
    return d > 0 ? Math.min(100, (displayedPosition() / d) * 100) : 0;
  };
  const previewSeek = (value: number) => {
    setSeeking(true);
    setSeekPosition(Math.max(0, Math.min(value, duration())));
  };
  const commitSeek = (value: number) => {
    const next = Math.max(0, Math.min(value, duration()));
    setSeekPosition(next);
    setSeeking(false);
    pb.seek(next);
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
      <input
        class="transport-progress-seek"
        type="range"
        min={0}
        max={Math.max(duration(), 1)}
        value={Math.min(displayedPosition(), duration())}
        aria-label={t("player.seek")}
        disabled={!track()}
        onInput={(event) => previewSeek(Number(event.currentTarget.value))}
        onChange={(event) => commitSeek(Number(event.currentTarget.value))}
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
          <Show
            when={!satelliteMode}
            fallback={<span class="chip">{targetLabel()}</span>}
          >
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
          </Show>
        </div>
      </div>

      <div class="transport-center">
        <div class="transport-buttons">
          <button
            type="button"
            class="btn btn-ghost btn-icon"
            classList={{ "mode-active": pb.shuffle() }}
            aria-label={t("player.shuffle")}
            aria-pressed={pb.shuffle()}
            title={t("player.shuffle")}
            onClick={pb.toggleShuffle}
            disabled={!track()}
          >
            <IconShuffle size={18} />
          </button>
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
          <button
            type="button"
            class="btn btn-ghost btn-icon playback-mode"
            classList={{ "mode-active": pb.repeatMode() !== "off" }}
            aria-label={
              pb.repeatMode() === "one"
                ? t("player.repeatOne")
                : pb.repeatMode() === "all"
                  ? t("player.repeatAll")
                  : t("player.repeatOff")
            }
            aria-pressed={pb.repeatMode() !== "off"}
            title={
              pb.repeatMode() === "one"
                ? t("player.repeatOne")
                : pb.repeatMode() === "all"
                  ? t("player.repeatAll")
                  : t("player.repeatOff")
            }
            onClick={pb.cycleRepeatMode}
            disabled={!track()}
          >
            <IconRepeat size={18} />
            <Show when={pb.repeatMode() === "one"}>
              <span class="playback-mode-one">1</span>
            </Show>
          </button>
        </div>
        <div class="scrub">
          <span class="mono">{formatDuration(displayedPosition())}</span>
          <input
            class="slider"
            type="range"
            min={0}
            max={Math.max(duration(), 1)}
            value={Math.min(displayedPosition(), duration())}
            aria-label={t("player.seek")}
            disabled={!track()}
            onInput={(event) => previewSeek(Number(event.currentTarget.value))}
            onChange={(event) => commitSeek(Number(event.currentTarget.value))}
          />
          <span class="mono">{formatDuration(duration())}</span>
        </div>
      </div>

      <div class="transport-right">
        <Show when={satelliteMode}>
          <button
            type="button"
            class="btn btn-ghost"
            onClick={() => void pb.enableOutputAudio().catch((error) => console.error(error))}
          >
            {pb.outputAudioReady() ? "Audio enabled" : "Enable audio"}
          </button>
        </Show>
        <Show when={track()}>
          <span class="badge">{isPlaying() ? t("jukebox.onAir") : t("jukebox.idle")}</span>
        </Show>
      </div>
    </footer>
  );
}
