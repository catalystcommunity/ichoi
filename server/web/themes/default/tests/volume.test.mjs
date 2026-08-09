import assert from "node:assert/strict";
import test from "node:test";

import { clampVolume, planVolumeChange } from "../src/lib/volume.ts";

test("volume values are rounded and limited to the supported range", () => {
  assert.equal(clampVolume(-1), 0);
  assert.equal(clampVolume(42.6), 43);
  assert.equal(clampVolume(101), 100);
  assert.equal(clampVolume(Number.NaN), 100);
});

test("local volume changes use the browser media element", () => {
  assert.deepEqual(planVolumeChange("local", 35), {
    volume: 35,
    mediaVolume: 0.35,
  });
});

test("shared target volume changes use the player command", () => {
  assert.deepEqual(planVolumeChange("satellite:kitchen", 72), {
    volume: 72,
    command: { op: "volume", volume: 72 },
  });
});

test("a satellite without its registered player does not send a command", () => {
  assert.deepEqual(planVolumeChange("satellite-pending", 20), { volume: 20 });
});
