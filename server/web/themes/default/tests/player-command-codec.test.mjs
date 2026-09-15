import assert from "node:assert/strict";
import test from "node:test";

import { playerCommandWireValue } from "../src/lib/player-command-codec.ts";

const commands = [
  { op: "enqueue", track_ids: ["track"] },
  { op: "enqueue-next", track_ids: ["track"] },
  { op: "remove", index: 0 },
  { op: "remove-item", queue_item_id: 1 },
  { op: "reorder", from_index: 0, to_index: 1 },
  { op: "move-item", queue_item_id: 1 },
  { op: "clear" },
  { op: "play", index: 0 },
  { op: "replace-and-play", track_ids: ["track"] },
  { op: "pause" },
  { op: "next" },
  { op: "previous" },
  { op: "seek", position_ms: 1000 },
  { op: "volume", volume: 50 },
  { op: "set-repeat", repeat_mode: "all" },
  { op: "set-shuffle", shuffle: true },
  { op: "undo" },
  { op: "playback-completed", playback_id: "playback", queue_item_id: 1 },
  {
    op: "playback-failed",
    playback_id: "playback",
    queue_item_id: 1,
    error: "failed",
  },
  {
    op: "playback-state",
    playback_id: "playback",
    queue_item_id: 1,
    status: "playing",
    position_ms: 1000,
  },
];

test("player commands use their CSIL union indexes", () => {
  for (const [index, command] of commands.entries()) {
    assert.deepEqual(playerCommandWireValue({ player_id: "player", command }), {
      command: [index, command],
      player_id: "player",
    });
  }
});
