// Small formatting helpers for track metadata.

import type { Codec, Track } from "./schema.ts";

/** Milliseconds -> `m:ss` or `h:mm:ss`. */
export function formatDuration(ms: number | undefined): string {
  if (!ms || ms < 0) return "0:00";
  const totalSec = Math.round(ms / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  const ss = s.toString().padStart(2, "0");
  if (h > 0) return `${h}:${m.toString().padStart(2, "0")}:${ss}`;
  return `${m}:${ss}`;
}

const CODEC_LABEL: Record<Codec, string> = {
  mp3: "MP3",
  aac: "AAC",
  vorbis: "Vorbis",
  flac: "FLAC",
  alac: "ALAC",
  opus: "Opus",
  wav: "WAV",
};

export function codecLabel(codec: Codec): string {
  return CODEC_LABEL[codec] ?? codec.toUpperCase();
}

/** Whether a codec is lossless — used to badge full-quality tracks. */
export function isLossless(codec: Codec): boolean {
  return codec === "flac" || codec === "alac" || codec === "wav";
}

/** A compact technical summary, e.g. "FLAC · 44.1 kHz · 16-bit". */
export function trackTechSummary(track: Track): string {
  const parts: string[] = [codecLabel(track.codec)];
  if (track.sample_rate) parts.push(`${(track.sample_rate / 1000).toFixed(1)} kHz`);
  if (track.bit_depth) parts.push(`${track.bit_depth}-bit`);
  else if (track.bitrate_kbps) parts.push(`${track.bitrate_kbps} kbps`);
  return parts.join(" · ");
}
