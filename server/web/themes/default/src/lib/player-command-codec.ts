import type { CborValue } from "./cbor.ts";
import type { CommandRequest, PlayerCommand } from "./schema.ts";

// CSIL encodes a type-choice union as `[variant_index, value]`. The variant order
// must match the PlayerCommand declaration in the schema.
const PLAYER_CMD_INDEX: Record<PlayerCommand["op"], number> = {
  enqueue: 0,
  "enqueue-next": 1,
  remove: 2,
  "remove-item": 3,
  reorder: 4,
  "move-item": 5,
  clear: 6,
  play: 7,
  "replace-and-play": 8,
  pause: 9,
  next: 10,
  previous: 11,
  seek: 12,
  volume: 13,
  "set-repeat": 14,
  "set-shuffle": 15,
  undo: 16,
  "playback-completed": 17,
  "playback-failed": 18,
  "playback-state": 19,
};

export function playerCommandWireValue(req: CommandRequest): CborValue {
  const index = PLAYER_CMD_INDEX[req.command.op];
  if (index === undefined) throw new Error(`unknown player command: ${req.command.op}`);
  return {
    player_id: req.player_id,
    command: [index, req.command as unknown as CborValue],
  };
}
