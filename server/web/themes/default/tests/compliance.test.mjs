import assert from "node:assert/strict";
import test from "node:test";

import {
  acceptTerms,
  buildReportRequest,
  canManageReports,
  changeReportStatus,
  deleteCurrentAccount,
  deleteReportedTarget,
  hasAcceptedTerms,
  isSignedIn,
  loadAllAccounts,
  loadAllContentReports,
  reportPage,
  requireTermsForPlaylist,
  submitContentReport,
} from "../src/lib/compliance.ts";

function memoryStorage() {
  const values = new Map();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
}

function report(id, status = "open", targetType = "playlist") {
  return {
    id,
    reporter_account_id: "reporter@example.com",
    target_type: targetType,
    target_id: `${targetType}-${id}`,
    reason: "spam",
    status,
    created_at: `2026-09-${String(id).padStart(2, "0")}T00:00:00Z`,
  };
}

test("terms acceptance applies to signed-in users and guests and gates playlists", () => {
  const storage = memoryStorage();
  const guest = { account_id: "__guest__", handle: "Guest", role: "guest" };
  const member = { account_id: "member@example.com", handle: "member", role: "member" };
  assert.equal(isSignedIn(guest), false);
  assert.equal(isSignedIn(member), true);
  assert.equal(hasAcceptedTerms(storage), false);
  assert.throws(() => requireTermsForPlaylist(storage), /Accept the terms/);
  acceptTerms(storage);
  assert.equal(hasAcceptedTerms(storage), true);
  assert.doesNotThrow(() => requireTermsForPlaylist(storage));
});

test("account deletion validates the exact handle and clears only after success", async () => {
  let requests = 0;
  let clears = 0;
  await assert.rejects(
    deleteCurrentAccount(async () => { requests += 1; }, async () => { clears += 1; }, "alice", "Alice"),
    /alice exactly/,
  );
  assert.equal(requests, 0);
  await deleteCurrentAccount(async (handle) => {
    assert.equal(handle, "alice");
    requests += 1;
  }, async () => { clears += 1; }, "alice", "alice");
  assert.equal(clears, 1);
  await assert.rejects(
    deleteCurrentAccount(async () => { throw new Error("server unavailable"); }, async () => { clears += 1; }, "alice", "alice"),
    /server unavailable/,
  );
  assert.equal(clears, 1);
});

test("report requests cover every reason, trim details, and reject invalid input", async () => {
  for (const reason of ["objectionable-content", "harassment", "spam", "other"]) {
    const request = buildReportRequest("playlist", "playlist-1", reason, "  detail  ");
    assert.deepEqual(request, {
      target_type: "playlist",
      target_id: "playlist-1",
      reason,
      details: "detail",
    });
  }
  assert.throws(() => buildReportRequest("account", "", "spam", ""), /Select content/);
  assert.throws(() => buildReportRequest("account", "account-1", "invalid", ""), /reason/);
  assert.throws(() => buildReportRequest("account", "account-1", "other", "😀".repeat(2_001)), /2,000/);
  const sent = [];
  await submitContentReport(async (request) => {
    sent.push(request);
    return report("1");
  }, buildReportRequest("account", "account-1", "harassment", ""));
  assert.equal(sent[0].target_type, "account");
  await assert.rejects(
    submitContentReport(async () => { throw new Error("service failed"); }, sent[0]),
    /service failed/,
  );
});

test("the report queue is admin-only, open-first, paginated, and supports moderation", async () => {
  assert.equal(canManageReports({ account_id: "a", handle: "a", role: "admin", can_admin: true }), true);
  assert.equal(canManageReports({ account_id: "m", handle: "m", role: "member" }), false);
  assert.deepEqual(reportPage([], 0, 20), []);
  const reports = [report("1", "resolved"), report("2", "open"), report("3", "dismissed")];
  assert.deepEqual(reportPage(reports, 0, 2).map((item) => item.id), ["2", "3"]);
  assert.deepEqual(reportPage(reports, 2, 2).map((item) => item.id), ["1"]);
  const loadCalls = [];
  const loaded = await loadAllContentReports(async ({ offset, limit }) => {
    loadCalls.push({ offset, limit });
    return offset === 0
      ? { reports: [report("1", "resolved"), report("2", "open")], total: 3 }
      : { reports: [report("3", "dismissed")], total: 3 };
  });
  assert.deepEqual(loadCalls, [{ offset: 0, limit: 1_000 }, { offset: 2, limit: 1_000 }]);
  assert.deepEqual(loaded.map((item) => item.id), ["2", "3", "1"]);
  const accountCalls = [];
  const accounts = await loadAllAccounts(async ({ offset, limit }) => {
    accountCalls.push({ offset, limit });
    return offset === 0
      ? { accounts: Array.from({ length: 1_000 }, (_, id) => ({ id: String(id), handle: String(id), role: "member", created_at: new Date(0) })) }
      : { accounts: [{ id: "last", handle: "last", role: "member", created_at: new Date(0) }] };
  });
  assert.deepEqual(accountCalls, [{ offset: 0, limit: 1_000 }, { offset: 1_000, limit: 1_000 }]);
  assert.equal(accounts.length, 1_001);

  const changes = [];
  await changeReportStatus(async (request) => {
    changes.push(request);
    return report("2", request.status);
  }, "2", "resolved");
  await changeReportStatus(async (request) => {
    changes.push(request);
    return report("3", request.status);
  }, "3", "dismissed");
  assert.deepEqual(changes, [
    { report_id: "2", status: "resolved" },
    { report_id: "3", status: "dismissed" },
  ]);

  const deleted = [];
  const actions = {
    deletePlaylist: async (id) => deleted.push(["playlist", id]),
    deleteAccount: async (id, handle) => deleted.push(["account", id, handle]),
  };
  await deleteReportedTarget(report("4"), actions);
  await deleteReportedTarget(report("5", "open", "account"), actions, "alice");
  assert.deepEqual(deleted, [
    ["playlist", "playlist-4"],
    ["account", "account-5", "alice"],
  ]);
  await assert.rejects(deleteReportedTarget(report("6", "open", "account"), actions), /no longer available/);
});
