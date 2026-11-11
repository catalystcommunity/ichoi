import type { ServerApi } from "./services.ts";

export function createCoverArtCache(maxEntries = 512) {
  const caches = new WeakMap<ServerApi, Map<string, Promise<Blob>>>();

  const queues = new WeakMap<ServerApi, { active: number; waiting: (() => void)[] }>();
  function schedule<T>(api: ServerApi, run: () => Promise<T>): Promise<T> {
    let queue = queues.get(api);
    if (!queue) { queue = { active: 0, waiting: [] }; queues.set(api, queue); }
    const lane = queue;
    return new Promise<T>((resolve, reject) => {
      const start = () => {
        lane.active++;
        void run().then(resolve, reject).finally(() => { lane.active--; lane.waiting.shift()?.(); });
      };
      if (lane.active < 2) start(); else lane.waiting.push(start);
    });
  }
  const listeners = new WeakMap<ServerApi, Set<() => void>>();
  function cachedCover(
    api: ServerApi,
    albumId: string,
    maxSize: number,
    kind: "album" | "artist" = "album",
  ): Promise<Blob> {
    let cache = caches.get(api);
    if (!cache) {
      cache = new Map();
      caches.set(api, cache);
    }

    const key = `${kind}:${albumId}:${maxSize}`;
    const cached = cache.get(key);
    if (cached) {
      cache.delete(key);
      cache.set(key, cached);
      return cached;
    }

    const request = schedule(api, () => (kind === "album"
      ? api.library.getCoverArt({ album_id: albumId, max_size: maxSize })
      : api.library.getArtwork({ kind, id: albumId, max_size: maxSize })))
      .then((art) => new Blob([art.data as BlobPart], { type: art.content_type }));
    cache.set(key, request);
    void request.catch(error => {
      // Missing art is stable until an artwork change. Do not repeat a 404 on every scroll.
      const missing = typeof error === "object" && error !== null && "code" in error && error.code === 404;
      if (!missing && cache.get(key) === request) cache.delete(key);
    });

    while (cache.size > maxEntries) {
      const oldest = cache.keys().next().value;
      if (oldest === undefined) break;
      cache.delete(oldest);
    }
    return request;
  }
  return Object.assign(cachedCover, {
    invalidate(api: ServerApi) {
      caches.delete(api);
      listeners.get(api)?.forEach((listener) => listener());
    },
    watch(api: ServerApi, listener: () => void) {
      let group = listeners.get(api);
      if (!group) { group = new Set(); listeners.set(api, group); }
      group.add(listener);
      return () => { group.delete(listener); };
    },
  });
}

export const cachedCover = createCoverArtCache();
