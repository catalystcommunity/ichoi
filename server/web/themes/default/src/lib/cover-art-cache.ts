import type { ServerApi } from "./services.ts";

export function createCoverArtCache(maxEntries = 256) {
  const caches = new WeakMap<ServerApi, Map<string, Promise<Blob>>>();

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

    const request = (kind === "album"
      ? api.library.getCoverArt({ album_id: albumId, max_size: maxSize })
      : api.library.getArtwork({ kind, id: albumId, max_size: maxSize }))
      .then((art) => new Blob([art.data as BlobPart], { type: art.content_type }));
    cache.set(key, request);
    void request.catch(() => {
      if (cache.get(key) === request) cache.delete(key);
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
