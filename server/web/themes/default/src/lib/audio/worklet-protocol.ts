// The message protocol between the main thread and the `ichoi-output` worklet
// (public/ichoi-worklet.js). Typed here so both sides agree on the shapes even
// though the worklet itself is plain JS. All audio data crosses as postMessage
// payloads — never SharedArrayBuffer (DESIGN §5.5).

export const WORKLET_MODULE = "ichoi-worklet.js";
export const WORKLET_PROCESSOR = "ichoi-output";

/** Main thread -> worklet. */
export type ToWorklet =
  | { type: "pcm"; channels: Float32Array[] }
  | { type: "play" }
  | { type: "pause" }
  | { type: "flush" }
  | { type: "end" };

/** Worklet -> main thread. */
export type FromWorklet =
  | { type: "underrun" }
  | { type: "overrun"; dropped: number }
  | { type: "drained" }
  | { type: "progress"; playedFrames: number };
