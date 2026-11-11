// Transient destination cache. Nothing is written to CacheStorage or disk.
const MEDIA_BLOCK = 512 * 1024;
const MEDIA_LIMIT = 128 * 1024 * 1024;
class MediaBuffer {
  constructor(url, duration) {
    this.url = url; this.duration = duration; this.blocks = new Map(); this.pending = new Map();
    this.controller = new AbortController(); this.total = 0; this.type = 'audio/mp4'; this.bytes = 0;
    this.position = 0; this.warming = false;
  }
  close() { this.controller.abort(); this.blocks.clear(); this.bytes = 0; }
  async block(index) {
    if (this.blocks.has(index)) return this.blocks.get(index);
    if (this.pending.has(index)) return this.pending.get(index);
    const task = (async () => {
      const start = index * MEDIA_BLOCK;
      const response = await fetch(this.url, { credentials: 'include', headers: { Range: `bytes=${start}-${start + MEDIA_BLOCK - 1}` }, signal: this.controller.signal });
      const match = /^bytes (\d+)-(\d+)\/(\d+)$/.exec(response.headers.get('Content-Range') || '');
      if (response.status !== 206 || !match || Number(match[1]) !== start || Number(match[2]) >= start + MEDIA_BLOCK) throw new Error('Range buffering unavailable');
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.length !== Number(match[2]) - start + 1) throw new Error('Incomplete media range');
      this.total = Number(match[3]); this.type = response.headers.get('Content-Type') || this.type;
      while (this.bytes + bytes.length > MEDIA_LIMIT && this.blocks.size) {
        const oldest = this.blocks.keys().next().value;
        this.bytes -= this.blocks.get(oldest).length; this.blocks.delete(oldest);
      }
      this.blocks.set(index, bytes); this.bytes += bytes.length;
      return bytes;
    })().finally(() => this.pending.delete(index));
    this.pending.set(index, task); return task;
  }
  async warm(position) {
    this.position = position;
    if (this.warming) return;
    this.warming = true;
    try {
      await this.block(0);
      const seconds = Math.max(1, this.duration / 1000);
      // Container byte positions vary with codec. Playback requests always override this estimate.
      const first = Math.max(0, Math.floor(this.total * Math.max(0, position / 1000 - 10) / seconds / MEDIA_BLOCK));
      const amount = Math.min(MEDIA_LIMIT - MEDIA_BLOCK, this.total * Math.min(600, seconds) / seconds);
      const end = Math.min(this.total, first * MEDIA_BLOCK + amount);
      for (let index = first; index * MEDIA_BLOCK < end; index++) {
        if (this.position !== position || this.controller.signal.aborted) break;
        await this.block(index);
      }
    } catch { /* The native media request remains the fallback. */ }
    finally { this.warming = false; }
  }
  async respond(request) {
    const range = /^bytes=(\d+)-(\d*)$/.exec(request.headers.get('Range') || '');
    if (!range) return fetch(request);
    const start = Number(range[1]);
    const block = await this.block(Math.floor(start / MEDIA_BLOCK));
    const offset = start % MEDIA_BLOCK;
    const end = Math.min(this.total - 1, start - offset + block.length - 1, range[2] ? Number(range[2]) : Infinity);
    if (end < start) return fetch(request);
    return new Response(block.slice(offset, offset + end - start + 1), { status: 206, headers: {
      'Content-Type': this.type, 'Content-Length': String(end - start + 1),
      'Content-Range': `bytes ${start}-${end}/${this.total}`, 'Accept-Ranges': 'bytes', 'Cache-Control': 'no-store',
    } });
  }
}
const mediaBuffers = new Map();
self.addEventListener('message', event => {
  if (event.data?.type !== 'ichoi-media-buffer' || !event.source?.id) return;
  const { url, duration, position = 0 } = event.data;
  const id = event.source.id;
  const old = mediaBuffers.get(id);
  if (!url) { old?.close(); mediaBuffers.delete(id); return; }
  let parsed;
  try { parsed = new URL(url); } catch { return; }
  if (parsed.origin !== self.location.origin || !parsed.pathname.startsWith('/media/')) return;
  if (old?.url !== url) { old?.close(); mediaBuffers.set(id, new MediaBuffer(url, duration)); }
  const buffer = mediaBuffers.get(id);
  event.waitUntil(Promise.all([buffer.warm(position), self.clients.matchAll().then(clients => {
    const active = new Set(clients.map(client => client.id));
    for (const [key, value] of mediaBuffers) if (!active.has(key)) { value.close(); mediaBuffers.delete(key); }
  })]));
});
