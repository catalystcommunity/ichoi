import { test } from 'node:test';
import assert from 'node:assert/strict';
import { savedQuery, rememberQuery, rememberScroll, savedScroll, resetBrowsing, rememberQueueReturn, queueReturn } from '../src/lib/browse-memory.ts';
test('queue return retains query and scroll, with server isolation and explicit reset', () => {
  rememberQuery('one','Reader'); rememberScroll('one|/search?q=Reader',850);
  rememberQueueReturn('one','/search?q=Reader'); rememberQueueReturn('one','/now-playing');
  assert.equal(queueReturn('one'),'/search?q=Reader'); assert.equal(savedQuery('one'),'Reader');
  assert.equal(savedScroll('one|/search?q=Reader'),850); assert.equal(savedQuery('two'),'');
  resetBrowsing('one'); assert.equal(savedQuery('one'),''); assert.equal(savedScroll('one|/search?q=Reader'),0);
});
