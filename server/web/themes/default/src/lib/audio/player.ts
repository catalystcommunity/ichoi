// PlayerController — the private-player playback engine (DESIGN §6.3). It ties the
// MediaService bidi stream (open/seek/pause/resume/stop) to the Symphonia→WASM
// decoder and the `ichoi-output` worklet, and enforces sample-accurate gapless
// trimming (§5.4).
//
// Data path (§5.5):
//   MediaService stream  → codec packets (MediaChunk.data)
//   → IchoiDecoder (WASM) → planar Float32 PCM
//   → trim head/tail      → postMessage → AudioWorklet ring → AudioContext
//
// Decoding runs on the main thread here for clarity; it is cheap (~100× realtime
// for FLAC) and can be moved into a Web Worker later without changing this API.
// The worklet is fed only via postMessage — never SharedArrayBuffer.

import type { MediaControl, MediaEvent, MediaHeader, StreamPref } from "../schema.ts";
import type { MediaStream } from "../services.ts";
import { loadDecoder, type IchoiDecoder } from "./decoder.ts";
import { PlanarFifo } from "./planar.ts";
import { WORKLET_MODULE, WORKLET_PROCESSOR, type FromWorklet, type ToWorklet } from "./worklet-protocol.ts";

export type PlaybackStatus = "idle" | "loading" | "playing" | "paused" | "ended" | "error";

export interface PlaybackSnapshot {
  status: PlaybackStatus;
  /** Milliseconds of real audio delivered to the output (position estimate). */
  positionMs: number;
  durationMs?: number;
  /** True when the decoder WASM is missing, so playback is unavailable. */
  decoderMissing: boolean;
  error?: string;
}

type Listener = (snap: PlaybackSnapshot) => void;

export class PlayerController {
  private ctx?: AudioContext;
  private node?: AudioWorkletNode;
  private decoder?: IchoiDecoder;
  private fifo?: PlanarFifo;
  private pendingTrimStart = 0;
  private trimEnd = 0;
  private streamSampleRate = 48000;
  private baseUrl: string;

  private snap: PlaybackSnapshot = {
    status: "idle",
    positionMs: 0,
    decoderMissing: false,
  };
  private listeners = new Set<Listener>();

  constructor(
    private readonly media: MediaStream,
    baseUrl = import.meta.env.BASE_URL,
  ) {
    this.baseUrl = baseUrl;
    this.media.listen((e) => this.onMediaEvent(e));
  }

  subscribe(fn: Listener): () => void {
    this.listeners.add(fn);
    fn(this.snap);
    return () => this.listeners.delete(fn);
  }

  get snapshot(): PlaybackSnapshot {
    return this.snap;
  }

  /** Open a track for local playback. Lazily boots the AudioContext + worklet on
   * first use (must be called from a user gesture so autoplay policy is satisfied). */
  async open(trackId: string, pref: StreamPref = {}): Promise<void> {
    await this.ensureAudio();
    this.update({ status: "loading", positionMs: 0, error: undefined });
    this.resetStreamState();
    const control: MediaControl = { kind: "open", track_id: trackId, pref };
    this.media.send(control);
  }

  pause(): void {
    this.post({ type: "pause" });
    this.media.send({ kind: "pause" });
    if (this.snap.status === "playing") this.update({ status: "paused" });
  }

  resume(): void {
    void this.ctx?.resume();
    this.post({ type: "play" });
    this.media.send({ kind: "resume" });
    if (this.snap.status === "paused") this.update({ status: "playing" });
  }

  seek(positionMs: number): void {
    // Flush buffered audio and decoder state, then reposition the server demuxer.
    this.post({ type: "flush" });
    this.decoder?.reset();
    this.fifo?.clear();
    this.pendingTrimStart = 0; // trim_start only applies at true stream start
    this.media.send({ kind: "seek", position_ms: positionMs });
  }

  stop(): void {
    this.media.send({ kind: "stop" });
    this.post({ type: "flush" });
    this.resetStreamState();
    this.update({ status: "idle", positionMs: 0 });
  }

  dispose(): void {
    this.stop();
    this.decoder?.close();
    this.node?.disconnect();
    void this.ctx?.close();
  }

  // --- Audio graph bootstrap ---

  private async ensureAudio(): Promise<void> {
    if (this.node) return;
    // Create the context; the worklet ring adapts to whatever rate it runs at.
    const ctx = new AudioContext({ latencyHint: "playback" });
    this.ctx = ctx;
    const moduleUrl = new URL(WORKLET_MODULE, new URL(this.baseUrl, location.href)).href;
    await ctx.audioWorklet.addModule(moduleUrl);
    const node = new AudioWorkletNode(ctx, WORKLET_PROCESSOR, {
      numberOfInputs: 0,
      numberOfOutputs: 1,
      outputChannelCount: [2],
      processorOptions: { channels: 2 },
    });
    node.port.onmessage = (e: MessageEvent<FromWorklet>) => this.onWorkletMessage(e.data);
    node.connect(ctx.destination);
    this.node = node;
  }

  // --- MediaService stream handling ---

  private async onMediaEvent(e: MediaEvent): Promise<void> {
    switch (e.kind) {
      case "header":
        await this.onHeader(e);
        break;
      case "chunk":
        this.onChunk(e.data);
        break;
      case "end":
        this.onEnd();
        break;
      case "error":
        this.update({ status: "error", error: e.error.message });
        break;
    }
  }

  private async onHeader(h: MediaHeader): Promise<void> {
    this.streamSampleRate = h.sample_rate;
    this.pendingTrimStart = h.trim_start_samples ?? 0;
    this.trimEnd = h.trim_end_samples ?? 0;
    this.fifo = new PlanarFifo(h.channels);

    const mod = await loadDecoder(this.baseUrl);
    if (!mod) {
      this.update({
        status: "error",
        decoderMissing: true,
        error: "Audio decoder (ichoi-decoder.wasm) is not installed on this server.",
        durationMs: h.duration_ms,
      });
      return;
    }
    this.decoder?.close();
    this.decoder = mod.createDecoder({
      codec: h.codec,
      sampleRate: h.sample_rate,
      channels: h.channels,
      codecConfig: h.codec_config,
    });
    this.post({ type: "flush" });
    this.post({ type: "play" });
    void this.ctx?.resume();
    this.update({ status: "playing", positionMs: 0, durationMs: h.duration_ms, decoderMissing: false });
  }

  private onChunk(data: Uint8Array): void {
    if (!this.decoder || !this.fifo) return;
    let planar: Float32Array[];
    try {
      planar = this.decoder.decode(data);
    } catch (err) {
      this.update({ status: "error", error: `decode failed: ${String(err)}` });
      return;
    }
    if (planar.length) this.fifo.push(planar);
    this.drainToWorklet(false);
  }

  private onEnd(): void {
    if (this.decoder && this.fifo) {
      try {
        const tail = this.decoder.flush();
        if (tail.length) this.fifo.push(tail);
      } catch {
        /* ignore flush errors */
      }
      this.drainToWorklet(true);
    }
    this.post({ type: "end" });
  }

  /**
   * Move decodable audio from the FIFO to the worklet, applying gapless trims:
   *  - skip `trim_start_samples` at the very start of the stream,
   *  - hold back `trim_end_samples` until end-of-stream so encoder padding is
   *    never played.
   */
  private drainToWorklet(atEnd: boolean): void {
    const fifo = this.fifo;
    if (!fifo) return;

    // Head trim.
    if (this.pendingTrimStart > 0) {
      this.pendingTrimStart -= fifo.drop(this.pendingTrimStart);
    }

    if (atEnd) {
      // Emit everything except the final `trimEnd` frames.
      const emit = Math.max(0, fifo.length - this.trimEnd);
      if (emit > 0) this.emit(fifo.shift(emit));
      // Whatever remains (<= trimEnd) is padding; drop it.
      fifo.clear();
      return;
    }

    // Steady state: always keep at least `trimEnd` frames buffered so the tail is
    // available to drop when `end` arrives.
    const emit = fifo.length - this.trimEnd;
    if (emit > 0) this.emit(fifo.shift(emit));
  }

  private emit(planar: Float32Array[] | undefined): void {
    if (!planar || !this.node) return;
    const transfer = planar.map((a) => a.buffer);
    const msg: ToWorklet = { type: "pcm", channels: planar };
    this.node.port.postMessage(msg, transfer as Transferable[]);
  }

  private onWorkletMessage(msg: FromWorklet): void {
    switch (msg.type) {
      case "progress": {
        const ms = Math.round((msg.playedFrames / this.streamSampleRate) * 1000);
        this.update({ positionMs: ms });
        break;
      }
      case "drained":
        this.update({ status: "ended" });
        break;
      case "underrun":
        // Carrier backpressure / decode lag. The server keeps sending; the ring
        // refills. Persistent underruns past a threshold would warrant a reset.
        break;
      case "overrun":
        console.warn(`[ichoi] worklet dropped ${msg.dropped} frames (ring full)`);
        break;
    }
  }

  private post(msg: ToWorklet): void {
    this.node?.port.postMessage(msg);
  }

  private resetStreamState(): void {
    this.pendingTrimStart = 0;
    this.trimEnd = 0;
    this.fifo?.clear();
    this.decoder?.reset();
  }

  private update(patch: Partial<PlaybackSnapshot>): void {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l(this.snap);
  }
}
