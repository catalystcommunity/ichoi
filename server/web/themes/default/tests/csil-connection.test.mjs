import assert from "node:assert/strict";
import test from "node:test";
import { CsilConnection, CsilServiceError, encodeEnvelope, decodeEnvelope } from "../src/lib/csil.ts";
import { encode, decode } from "../src/lib/cbor.ts";
import { requestErrorMessage } from "../src/lib/request-error.ts";

class Socket {
  static OPEN = 1;
  static instances = [];
  readyState = 0;
  sent = [];
  constructor() { Socket.instances.push(this); }
  send(bytes) {
    if (this.failSend) throw new Error("send failed");
    this.sent.push(decodeEnvelope(bytes));
  }
  open() { this.readyState = 1; this.onopen?.(); }
  receive(event, payload = {}, id) {
    this.onmessage?.({ data: encodeEnvelope({ event, id, payload: encode(payload) }).buffer });
  }
  close() { this.readyState = 3; }
}

function setup(t, options = {}) {
  Socket.instances = [];
  const original = globalThis.WebSocket;
  globalThis.WebSocket = Socket;
  const conn = new CsilConnection({ url: "ws://test", callTimeoutMs: 100, ...options });
  t.after(() => { conn.close(); globalThis.WebSocket = original; });
  const connected = conn.connect();
  return { conn, connected, socket: Socket.instances.at(-1) };
}

async function ready(t, options) {
  const fixture = setup(t, options);
  fixture.socket.open();
  fixture.socket.receive("$hello-ack");
  await fixture.connected;
  return fixture;
}

test("a stalled handshake rejects both connection and waiting calls", async (t) => {
  const { conn, connected, socket } = setup(t, { connectTimeoutMs: 15 });
  socket.open();
  await Promise.all([
    assert.rejects(connected, /connection timed out/),
    assert.rejects(conn.call("AdminService", "list-content-reports", encode({}), decode), /connection timed out/),
  ]);
  assert.equal(socket.readyState, 3);
  assert.equal(conn.pending.size, 0);
  assert.equal(conn.readyWaiters.length, 0);
});

test("concurrent connects share one socket", async (t) => {
  const { conn, connected, socket } = setup(t);
  const another = conn.connect();
  assert.equal(Socket.instances.length, 1);
  socket.open();
  socket.receive("$hello-ack");
  await Promise.all([connected, another]);
});

test("a call that expires before hello-ack is never sent later", async (t) => {
  const { conn, connected, socket } = setup(t);
  const call = conn.call("PlayerService", "command", encode({}), decode, 5);
  await assert.rejects(call, /timed out/);
  socket.open();
  socket.receive("$hello-ack");
  await connected;
  assert.deepEqual(socket.sent.map((frame) => frame.event), ["$hello"]);
  assert.equal(conn.pending.size, 0);
});

test("send failures clean up correlation entries and timers", async (t) => {
  const { conn, socket } = await ready(t);
  socket.failSend = true;
  await assert.rejects(conn.call("LibraryService", "list-albums", encode({}), decode), /send failed/);
  assert.equal(conn.pending.size, 0);
});

test("a timed-out request is not replayed and a later retry can succeed", async (t) => {
  const { conn, socket } = await ready(t);
  await assert.rejects(conn.call("LibraryService", "list-albums", encode({}), decode, 5), /timed out/);
  const retry = conn.call("LibraryService", "list-albums", encode({}), decode);
  await Promise.resolve();
  const calls = socket.sent.filter((frame) => frame.id !== undefined);
  assert.equal(calls.length, 2);
  socket.receive("list-albums", { albums: [], total: 0 }, calls[1].id);
  assert.deepEqual(await retry, { albums: [], total: 0 });
  assert.equal(conn.pending.size, 0);
});

test("correlated transport errors reject immediately without closing other calls", async (t) => {
  const { conn, socket } = await ready(t);
  const first = conn.call("AdminService", "list-content-reports", encode({}), decode);
  const second = conn.call("LibraryService", "list-albums", encode({}), decode);
  const rejected = assert.rejects(first, /unsupported request/);
  await Promise.resolve();
  const calls = socket.sent.filter((frame) => frame.id !== undefined);
  socket.receive("$error", { status: 400, reason: "unsupported request" }, calls[0].id);
  socket.receive("list-albums", { albums: [], total: 0 }, calls[1].id);
  await rejected;
  assert.deepEqual(await second, { albums: [], total: 0 });
  assert.equal(conn.connectionState, "ready");
});

test("server close rejects pending calls and retires the old socket", async (t) => {
  const { conn, socket } = await ready(t);
  const pending = conn.call("LibraryService", "list-albums", encode({}), decode);
  const rejected = assert.rejects(pending, /maintenance/);
  socket.receive("$close", { status: 0, reason: "maintenance" });
  await rejected;
  assert.equal(socket.onmessage, null);
  assert.equal(socket.readyState, 3);
  assert.equal(conn.pending.size, 0);
});

test("a late correlated error cannot break a newer request", async (t) => {
  const { conn, socket } = await ready(t);
  await assert.rejects(conn.call("LibraryService", "list-albums", encode({}), decode, 5), /timed out/);
  const expired = socket.sent.at(-1).id;
  const retry = conn.call("LibraryService", "list-albums", encode({}), decode);
  await Promise.resolve();
  socket.receive("$error", { status: 500, reason: "late error" }, expired);
  socket.receive("list-albums", { albums: [], total: 0 }, socket.sent.at(-1).id);
  assert.deepEqual(await retry, { albums: [], total: 0 });
  assert.equal(conn.connectionState, "ready");
});

test("service errors preserve codes and have useful UI messages", async (t) => {
  const { conn, socket } = await ready(t);
  const call = conn.call("AdminService", "list-content-reports", encode({}), decode);
  await Promise.resolve();
  socket.receive("list-content-reports", { code: 403, message: "forbidden" }, socket.sent.at(-1).id);
  await assert.rejects(call, (error) => error instanceof CsilServiceError && error.code === 403);
  assert.match(requestErrorMessage(new CsilServiceError(403, "forbidden")), /permission/);
  assert.match(requestErrorMessage(new Error("csil: call LibraryService.list-albums timed out")), /too long/);
  assert.match(requestErrorMessage(new Error("unknown operation admin.list-content-reports")), /Update the server/);
});
