import assert from "node:assert/strict";
import test from "node:test";

import { appendTracks, insertTrackNext } from "../src/lib/queue-plan.ts";

const track = (id) => ({ id, title: id });

test("append selects the first track without starting playback", () => {
  assert.deepEqual(appendTracks([], -1, [track("one")]), {
    tracks: [track("one")],
    currentIndex: 0,
  });
});

test("append preserves the current track", () => {
  assert.deepEqual(appendTracks([track("one")], 0, [track("two")]), {
    tracks: [track("one"), track("two")],
    currentIndex: 0,
  });
});

test("play next inserts after the current track", () => {
  assert.deepEqual(
    insertTrackNext([track("one"), track("three")], 0, track("two")),
    {
      tracks: [track("one"), track("two"), track("three")],
      currentIndex: 0,
    },
  );
});

test("play next selects an inserted track when no track is current", () => {
  assert.deepEqual(insertTrackNext([track("later")], -1, track("next")), {
    tracks: [track("next"), track("later")],
    currentIndex: 0,
  });
});
