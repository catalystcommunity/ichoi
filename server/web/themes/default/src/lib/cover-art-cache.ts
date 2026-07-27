import type { ServerApi } from "./services.ts";

export function createCoverArtCache(maxEntries = 256) {
  const caches = new WeakMap<ServerApi, Map<string, Promise<Blob>>>();

  return function cachedCover(
    api: ServerApi,
    albumId: string,
    maxSize: number,
  ): Promise<Blob> {
    let cache = caches.get(api);
    if (!cache) {
      cache = new Map();
      caches.set(api, cache);
    }

    const key = `${albumId}:${maxSize}`;
    const cached = cache.get(key);
    if (cached) {
      cache.delete(key);
      cache.set(key, cached);
      return cached;
    }

    const request = api.library
      .getCoverArt({ album_id: albumId, max_size: maxSize })
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
  };
}

export const cachedCover = createCoverArtCache();
