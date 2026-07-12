// A small planar-PCM FIFO used for gapless trimming. Holds a queue of planar
// chunks (one Float32Array per channel) and can shift a precise number of frames
// off the front — needed to drop `trim_end_samples` at end-of-stream and to skip
// `trim_start_samples` at the head (DESIGN §5.4).

export class PlanarFifo {
  private chunks: Float32Array[][] = [];
  private frames = 0;

  constructor(readonly channels: number) {}

  get length(): number {
    return this.frames;
  }

  push(planar: Float32Array[]): void {
    const n = planar[0]?.length ?? 0;
    if (n === 0) return;
    this.chunks.push(planar);
    this.frames += n;
  }

  /** Remove and return the first `count` frames as one planar block, or all
   * available if fewer. Returns `undefined` if nothing is available. */
  shift(count: number): Float32Array[] | undefined {
    const take = Math.min(count, this.frames);
    if (take <= 0) return undefined;
    const out: Float32Array[] = [];
    for (let c = 0; c < this.channels; c++) out.push(new Float32Array(take));
    let filled = 0;
    while (filled < take && this.chunks.length) {
      const head = this.chunks[0]!;
      const headLen = head[0]?.length ?? 0;
      const need = take - filled;
      if (headLen <= need) {
        for (let c = 0; c < this.channels; c++) out[c]!.set(head[c] ?? new Float32Array(headLen), filled);
        filled += headLen;
        this.chunks.shift();
      } else {
        // Split the head chunk: copy `need` frames, keep the remainder.
        const remainder: Float32Array[] = [];
        for (let c = 0; c < this.channels; c++) {
          const src = head[c] ?? new Float32Array(headLen);
          out[c]!.set(src.subarray(0, need), filled);
          remainder.push(src.subarray(need));
        }
        this.chunks[0] = remainder;
        filled += need;
      }
    }
    this.frames -= take;
    return out;
  }

  /** Discard up to `count` frames from the front. Returns frames actually dropped. */
  drop(count: number): number {
    const dropped = this.shift(count);
    return dropped?.[0]?.length ?? 0;
  }

  clear(): void {
    this.chunks = [];
    this.frames = 0;
  }
}
