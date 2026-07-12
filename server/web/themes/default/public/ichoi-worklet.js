// Ichoi audio output worklet — the AudioWorkletProcessor that turns decoded PCM
// into gapless sound. It is fed EXCLUSIVELY by `postMessage` (DESIGN §5.5: NOT
// SharedArrayBuffer — the SAB pattern needs crossOriginIsolated, which would
// break cross-origin theme assets; the efficiency delta is immaterial at audio
// data rates). The main thread decodes packets with the Symphonia→WASM module and
// posts Float32 PCM here; this processor buffers it in a ring and drains 128
// frames per `process()` call.
//
// This file is plain JS in `public/` so it is served verbatim and runs as-is; the
// message protocol it implements is mirrored (typed) in
// `src/lib/audio/worklet-protocol.ts`.

// A per-channel ring buffer of Float32 samples. Fixed capacity; writes past the
// free space are dropped with an "overrun" report (backpressure is the main
// thread's job, via WebSocket.bufferedAmount + decode queue depth).
class Ring {
  constructor(channels, capacity) {
    this.channels = channels;
    this.capacity = capacity;
    this.buffers = [];
    for (let c = 0; c < channels; c++) this.buffers.push(new Float32Array(capacity));
    this.read = 0;
    this.write = 0;
    this.size = 0;
  }

  available() {
    return this.size;
  }

  free() {
    return this.capacity - this.size;
  }

  // Push planar PCM (array of Float32Array, one per channel). Returns frames written.
  push(planar) {
    const frames = planar[0] ? planar[0].length : 0;
    const toWrite = Math.min(frames, this.free());
    for (let c = 0; c < this.channels; c++) {
      const src = planar[c] || planar[0] || new Float32Array(toWrite);
      const dst = this.buffers[c];
      let w = this.write;
      for (let i = 0; i < toWrite; i++) {
        dst[w] = src[i];
        w = w + 1 === this.capacity ? 0 : w + 1;
      }
    }
    this.write = (this.write + toWrite) % this.capacity;
    this.size += toWrite;
    return toWrite;
  }

  // Pull `frames` into the given planar output arrays. Zero-fills a shortfall
  // (an underrun) and returns the number of real frames delivered.
  pull(outputs, frames) {
    const toRead = Math.min(frames, this.size);
    for (let c = 0; c < outputs.length; c++) {
      const dst = outputs[c];
      const src = this.buffers[Math.min(c, this.channels - 1)];
      let r = this.read;
      for (let i = 0; i < toRead; i++) {
        dst[i] = src[r];
        r = r + 1 === this.capacity ? 0 : r + 1;
      }
      for (let i = toRead; i < frames; i++) dst[i] = 0;
    }
    this.read = (this.read + toRead) % this.capacity;
    this.size -= toRead;
    return toRead;
  }

  clear() {
    this.read = 0;
    this.write = 0;
    this.size = 0;
  }
}

class IchoiOutputProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const opt = (options && options.processorOptions) || {};
    this.channels = opt.channels || 2;
    // ~4 seconds of headroom at the context sample rate.
    this.ring = new Ring(this.channels, Math.max(1, Math.round(sampleRate * 4)));
    this.playing = true;
    this.ended = false;
    this.playedFrames = 0;
    this.lastReport = 0;
    this.underrunning = false;

    this.port.onmessage = (e) => this.onMessage(e.data);
  }

  onMessage(msg) {
    switch (msg.type) {
      case "pcm": {
        // msg.channels: Array<Float32Array> (transferred). Enqueue.
        const written = this.ring.push(msg.channels);
        if (written < (msg.channels[0] ? msg.channels[0].length : 0)) {
          this.port.postMessage({ type: "overrun", dropped: (msg.channels[0].length - written) });
        }
        break;
      }
      case "play":
        this.playing = true;
        break;
      case "pause":
        this.playing = false;
        break;
      case "flush":
        // Seek/reset: drop buffered audio so we resync to the new position.
        this.ring.clear();
        this.ended = false;
        this.playedFrames = 0;
        break;
      case "end":
        // No more PCM will arrive; drain what remains, then report drained.
        this.ended = true;
        break;
      default:
        break;
    }
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    if (!out || out.length === 0) return true;
    const frames = out[0].length;

    if (!this.playing) {
      for (let c = 0; c < out.length; c++) out[c].fill(0);
      return true;
    }

    const delivered = this.ring.pull(out, frames);
    this.playedFrames += delivered;

    if (delivered < frames) {
      if (this.ended && this.ring.available() === 0) {
        this.port.postMessage({ type: "drained" });
        this.ended = false; // report once
      } else if (!this.underrunning) {
        this.underrunning = true;
        this.port.postMessage({ type: "underrun" });
      }
    } else if (this.underrunning) {
      this.underrunning = false;
    }

    // Throttled progress report (~ every 100ms) for position + gapless bookkeeping.
    const now = currentFrame;
    if (now - this.lastReport >= sampleRate / 10) {
      this.lastReport = now;
      this.port.postMessage({ type: "progress", playedFrames: this.playedFrames });
    }
    return true;
  }
}

registerProcessor("ichoi-output", IchoiOutputProcessor);
