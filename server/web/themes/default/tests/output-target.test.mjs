import assert from "node:assert/strict";
import test from "node:test";

import {
  firstOwnedTarget,
  isBrowserShareTarget,
  outputTargetName,
  parseOwnedTargetStore,
  resolveOutputTarget,
} from "../src/lib/output-target.ts";

const targets = [
  { id: "share:friend:kitchen", name: "Kitchen" },
  { id: "share:me:phone", name: "Phone" },
];

test("local resolves to the available target shared by this browser", () => {
  assert.equal(resolveOutputTarget("local", targets, ["share:me:phone"]), "share:me:phone");
  assert.deepEqual(firstOwnedTarget(targets, ["share:me:phone"]), targets[1]);
});

test("local remains private when this browser has no available shared target", () => {
  assert.equal(resolveOutputTarget("local", targets, ["share:me:missing"]), "local");
});

test("an explicit shared target is never redirected", () => {
  assert.equal(
    resolveOutputTarget("share:friend:kitchen", targets, ["share:me:phone"]),
    "share:friend:kitchen",
  );
});

test("only the owner presentation adds the Mine marker", () => {
  assert.equal(outputTargetName("Phone", true, "Mine"), "Phone (Mine)");
  assert.equal(outputTargetName("Phone", false, "Mine"), "Phone");
});

test("ownership is stored separately for each server", () => {
  const parsed = parseOwnedTargetStore(JSON.stringify({
    version: 2,
    servers: {
      home: ["share:me:phone"],
      cloud: ["share:me:browser"],
    },
  }));
  assert.deepEqual(parsed.servers.home, ["share:me:phone"]);
  assert.deepEqual(parsed.servers.cloud, ["share:me:browser"]);
  assert.deepEqual(parsed.legacy, []);
});

test("native satellite targets cannot become browser-owned shares", () => {
  const parsed = parseOwnedTargetStore(JSON.stringify({
    version: 2,
    servers: {
      home: ["player:sat:kitchen:default", "share:me:phone"],
    },
  }));

  assert.deepEqual(parsed.servers.home, ["share:me:phone"]);
  assert.equal(isBrowserShareTarget("player:sat:kitchen:default"), false);
  assert.equal(isBrowserShareTarget("share:me:phone"), true);
});

test("the old global ownership list is migrated only once", () => {
  const parsed = parseOwnedTargetStore('["share:me:phone"]');
  assert.deepEqual(parsed.servers, {});
  assert.deepEqual(parsed.legacy, ["share:me:phone"]);
});
