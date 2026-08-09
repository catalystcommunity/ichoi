import type { PlayerCommand } from "./schema.ts";

export interface VolumeChange {
  volume: number;
  mediaVolume?: number;
  command?: PlayerCommand;
}

export function clampVolume(value: number): number {
  if (!Number.isFinite(value)) return 100;
  return Math.max(0, Math.min(100, Math.round(value)));
}

export function planVolumeChange(target: string, value: number): VolumeChange {
  const volume = clampVolume(value);
  if (target === "local") return { volume, mediaVolume: volume / 100 };
  if (target === "satellite-pending") return { volume };
  return { volume, command: { op: "volume", volume } };
}
