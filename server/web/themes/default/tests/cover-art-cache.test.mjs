import assert from "node:assert/strict";
import test from "node:test";
import { createCoverArtCache } from "../src/lib/cover-art-cache.ts";

function fakeApi() {
  let calls = 0;
  const api = {
    library: {
      async getCoverArt({ album_id }) {
        calls += 1;
        return {
          content_type: "image/jpeg",
          data: new TextEncoder().encode(album_id),
        };
      },
    },
  };
  return { api, calls: () => calls };
}

test("reuses cover data after its row leaves and returns to the DOM", async () => {
  const cache = createCoverArtCache();
  const fake = fakeApi();

  const first = cache(fake.api, "album-1", 48);
  await first;
  const returned = cache(fake.api, "album-1", 48);

  assert.strictEqual(returned, first);
  assert.equal(fake.calls(), 1);
});

test("evicts the least recently used cover when the cache is full", async () => {
  const cache = createCoverArtCache(2);
  const fake = fakeApi();

  await cache(fake.api, "album-1", 48);
  await cache(fake.api, "album-2", 48);
  await cache(fake.api, "album-1", 48);
  await cache(fake.api, "album-3", 48);
  await cache(fake.api, "album-2", 48);

  assert.equal(fake.calls(), 4);
});

test("retries a cover request after a failure", async () => {
  const cache = createCoverArtCache();
  let calls = 0;
  const api = {
    library: {
      async getCoverArt() {
        calls += 1;
        if (calls === 1) throw new Error("temporary failure");
        return { content_type: "image/jpeg", data: new Uint8Array([1]) };
      },
    },
  };

  await assert.rejects(cache(api, "album-1", 48), /temporary failure/);
  await cache(api, "album-1", 48);

  assert.equal(calls, 2);
});

test("a saved cover invalidates only its server and notifies mounted views", async () => {
  const cache = createCoverArtCache();
  const one = fakeApi(); const two = fakeApi();
  let changes = 0;
  const stop = cache.watch(one.api, () => changes++);
  const first = await cache(one.api, "same-id", 48);
  const other = await cache(two.api, "same-id", 48);
  cache.invalidate(one.api);
  assert.notStrictEqual(await cache(one.api, "same-id", 48), first);
  assert.strictEqual(await cache(two.api, "same-id", 48), other);
  assert.equal(changes, 1);
  stop(); cache.invalidate(one.api);
  assert.equal(changes, 1);
});

test("artist and album IDs do not collide", async () => {
  const cache = createCoverArtCache();
  const fake = fakeApi();
  fake.api.library.getArtwork = async ({ kind, id }) => ({ content_type: "image/png", data: new TextEncoder().encode(`${kind}:${id}`) });
  const album = await cache(fake.api, "same-id", 48);
  const artist = await cache(fake.api, "same-id", 48, "artist");
  assert.notStrictEqual(album, artist);
  assert.equal(await artist.text(), "artist:same-id");
});
