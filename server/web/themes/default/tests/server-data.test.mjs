import assert from "node:assert/strict";
import test from "node:test";

import { PlayerCatalogStore, PlayerStateStore } from "../src/stores/server-data.ts";

function player(id, name = id) {
  return { id, name, kind: "shared" };
}

test("the catalog emits a semantic removal without changing unrelated players", async () => {
  const snapshots = [
    [player("A"), player("B"), player("C")],
    [player("A"), player("C")],
  ];
  const api = {
    player: {
      listPlayers: async () => ({ players: snapshots.shift() }),
    },
  };
  const store = new PlayerCatalogStore(api);
  const changes = [];
  store.watch((change) => changes.push(change));

  await store.refresh();
  const playerA = store.players[0];
  await store.refresh();

  assert.deepEqual(changes[1].removedIds, ["B"]);
  assert.deepEqual(changes[1].addedIds, []);
  assert.deepEqual(changes[1].updatedIds, []);
  assert.equal(store.players[0], playerA);
});

test("an identical catalog snapshot emits nothing", async () => {
  const same = [player("A"), player("B")];
  const api = { player: { listPlayers: async () => ({ players: same.map((p) => ({ ...p })) }) } };
  const store = new PlayerCatalogStore(api);
  let calls = 0;
  store.watch(() => calls += 1);
  await store.refresh();
  await store.refresh();
  assert.equal(calls, 1);
});

test("an invalidation during a refresh queues one follow-up snapshot", async () => {
  let finishFirst;
  let calls = 0;
  const api = {
    player: {
      listPlayers() {
        calls += 1;
        if (calls === 1) {
          return new Promise((resolve) => {
            finishFirst = () => resolve({ players: [player("A")] });
          });
        }
        return Promise.resolve({ players: [player("A"), player("B")] });
      },
    },
  };
  const store = new PlayerCatalogStore(api);
  const first = store.refresh();
  store.refresh();
  finishFirst();
  await first;
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(calls, 2);
  assert.deepEqual(store.players.map((entry) => entry.id), ["A", "B"]);
});

test("player state uses one wire subscription until its final consumer leaves", () => {
  const subscriptions = [];
  let receive;
  const api = {
    conn: { connectionState: "ready" },
    player: {
      onState(handler) {
        receive = handler;
        return () => undefined;
      },
      setSubscription(playerId, active) {
        subscriptions.push([playerId, active]);
      },
    },
  };
  const store = new PlayerStateStore(api);
  const first = [];
  const second = [];
  const offFirst = store.watch("A", (state) => first.push(state.status));
  const offSecond = store.watch("A", (state) => second.push(state.status));
  receive({ player_id: "A", status: "playing", volume: 100, queue: [] });

  assert.deepEqual(subscriptions, [["A", true]]);
  assert.deepEqual(first, ["playing"]);
  assert.deepEqual(second, ["playing"]);
  offFirst();
  assert.deepEqual(subscriptions, [["A", true]]);
  offSecond();
  assert.deepEqual(subscriptions, [["A", true], ["A", false]]);
});
