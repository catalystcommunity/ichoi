import assert from "node:assert/strict";
import test from "node:test";

import { EventRouter, EventScope } from "../src/lib/events.ts";

test("an event scope removes all listeners when its owner is destroyed", () => {
  const router = new EventRouter();
  const scope = new EventScope();
  let calls = 0;
  scope.on(router, "players", () => calls += 1);
  scope.on(router, "players", () => calls += 10);

  router.emit("players", undefined);
  assert.equal(calls, 11);
  assert.deepEqual(router.stats(), { eventTypes: 1, listeners: 2 });

  scope.dispose();
  assert.deepEqual(router.stats(), { eventTypes: 0, listeners: 0 });
  router.emit("players", undefined);
  assert.equal(calls, 11);
});

test("disposal is idempotent and removes the empty event bucket", () => {
  const router = new EventRouter();
  const off = router.on("state", () => undefined);
  off();
  off();
  assert.deepEqual(router.stats(), { eventTypes: 0, listeners: 0 });
});

test("one broken handler does not prevent other reconcilers from running", () => {
  const router = new EventRouter();
  let called = false;
  router.on("players", () => {
    throw new Error("broken component");
  });
  router.on("players", () => {
    called = true;
  });
  assert.throws(() => router.emit("players", undefined), /broken component/);
  assert.equal(called, true);
});
