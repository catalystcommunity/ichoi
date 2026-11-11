import { test } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import fs from 'node:fs';

function cacheHarness(bytes) {
  let requests = 0;
  const context = vm.createContext({ Map, Uint8Array, AbortController, URL, Response, Error, Promise,
    self: { addEventListener() {} },
    fetch: async (_url, options) => {
      requests++;
      const [start, requestedEnd] = options.headers.Range.slice(6).split('-').map(Number);
      const end = Math.min(requestedEnd, bytes.length - 1);
      return new Response(bytes.slice(start, end + 1), { status: 206, headers: { 'Content-Range': `bytes ${start}-${end}/${bytes.length}`, 'Content-Type': 'audio/mp4' } });
    },
  });
  vm.runInContext(fs.readFileSync(new URL('../public/media-buffer.js', import.meta.url), 'utf8') + '\nglobalThis.BufferClass = MediaBuffer;', context);
  return { cache: new context.BufferClass('https://example.test/media/book', 600_000), requests: () => requests };
}
test('cached seek returns exact bytes without another network request', async () => {
  const data = Uint8Array.from({ length: 1_300_000 }, (_, i) => i % 251);
  const { cache, requests } = cacheHarness(data);
  await cache.warm(0);
  const before = requests();
  const response = await cache.respond(new Request(cache.url, { headers: { Range: 'bytes=600000-600127' } }));
  assert.equal(response.status, 206);
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()), data.slice(600000,600128));
  assert.equal(requests(), before);
  assert.equal(response.headers.get('Content-Range'), 'bytes 600000-600127/1300000');
  cache.close(); assert.equal(cache.bytes, 0);
});
test('concurrent requests for the same range share the download', async () => {
  const { cache, requests } = cacheHarness(new Uint8Array(100));
  await Promise.all([cache.block(0),cache.block(0)]);
  assert.equal(requests(), 1);
});
