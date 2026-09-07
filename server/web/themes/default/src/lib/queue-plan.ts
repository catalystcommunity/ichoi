import type { Track } from "./schema.ts";

export interface QueuePlan {
  tracks: Track[];
  currentIndex: number;
}

export function appendTracks(
  queue: readonly Track[],
  currentIndex: number,
  tracks: readonly Track[],
): QueuePlan {
  const next = [...queue, ...tracks];
  return {
    tracks: next,
    currentIndex: currentIndex >= 0 ? currentIndex : next.length ? 0 : -1,
  };
}

export function insertTrackNext(
  queue: readonly Track[],
  currentIndex: number,
  track: Track,
): QueuePlan {
  const insertionIndex = currentIndex >= 0 ? Math.min(currentIndex + 1, queue.length) : 0;
  const next = [...queue];
  next.splice(insertionIndex, 0, track);
  return {
    tracks: next,
    currentIndex: currentIndex >= 0 ? currentIndex : 0,
  };
}
