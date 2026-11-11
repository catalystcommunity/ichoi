// Open /tests/browser/reliability.html on the Vite development server.
// Use a separate browser profile. This fixture replaces the local session store.
import { defineControls } from "@catalystcommunity/controls";
import { defineContainers } from "@catalystcommunity/containers";
import { defineMenus } from "@catalystcommunity/menus";
import { defineDataViews } from "@catalystcommunity/data-views";
defineControls(); defineContainers(); defineMenus(); defineDataViews();
const { IchoiApp } = await import("../../src/app.ts");
const { usePlayback } = await import("../../src/stores/playback.ts");
const { useServers } = await import("../../src/stores/servers.ts");
import { CsilConnection } from "../../src/lib/csil.ts";
import { decode } from "../../src/lib/cbor.ts";

const root = document.getElementById("root");
const failures = new Map();
const responses = new Map();
const calls = [];
const results = [];
const originalStorage = new Map(Object.entries(localStorage));
const originalFetch = window.fetch;
window.reliabilityResults = results;
let dispose;

function check(condition, message) {
  if (!condition) throw new Error(message);
  results.push(message);
}
async function until(condition) {
  const end = Date.now() + 3000;
  while (!condition()) {
    if (Date.now() > end) throw new Error("UI condition timed out");
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}
const text = () => root.innerText;
const panel = (title) => {
  const section = [...root.querySelectorAll("section")].find(element => element.querySelector("h2")?.textContent === title);
  return section?.querySelector(".request-error") ?? (title === "catalog" || title === "page" ? root.querySelector("main .request-error") : null);
};
function go(path) {
  const link = document.createElement("a");
  link.href = path;
  root.querySelector("ichoi-app").append(link);
  link.click();
  link.remove();
}
const timeout = (operation) => new Error(`csil: call LibraryService.${operation} timed out`);
const defaults = {
  whoami: { account_id: "admin", handle: "admin@test", can_admin: true, role: "admin" },
  "list-libraries": { libraries: [{ id: "books", kind: "audiobook", name: "Audiobooks" }] },
  "list-albums": { albums: [], total: 0 },
  "list-artists": { artists: [], total: 0 },
  "list-players": { players: [] },
  "get-settings": { entries: {} },
  "get-resync-status": { running: false },
  "list-content-reports": { reports: [], total: 0 },
  "list-accounts": { accounts: [] },
  "list-trusted-identities": { identities: [] },
  "list-blocked-accounts": { accounts: [], total: 0 },
  "list-groups": { groups: [] },
  "list-satellite-tokens": { satellites: [] },
  "list-nodes": { nodes: [] },
};
CsilConnection.prototype.connect = async function () {
  this.state = "ready";
  this.opts.onState?.("ready");
};
CsilConnection.prototype.sendChannel = function () {};
CsilConnection.prototype.call = async function (_service, operation, bytes) {
  const request = decode(bytes);
  calls.push({ operation, request });
  if (failures.has(operation)) throw failures.get(operation);
  if (responses.has(operation)) return responses.get(operation)(request);
  return defaults[operation] ?? {};
};
window.fetch = (url, ...args) => String(url) === "/api/auth"
  ? Promise.resolve(new Response(JSON.stringify({ guest_allowed: true }), { headers: { "Content-Type": "application/json" } }))
  : originalFetch(url, ...args);

try {
  localStorage.clear();
  localStorage.setItem("ichoi.terms.accepted.v1", "accepted");
  history.replaceState(null, "", "/settings");
  failures.set("list-accounts", timeout("list-accounts"));
  const app = new IchoiApp(); root.replaceChildren(app); dispose = () => app.remove();
  await until(() => text().includes("No content reports") && panel("Accounts"));
  check(text().includes("Appearance"), "Settings remains available after an account request fails");
  check(text().includes("No content reports"), "Empty reports do not depend on the account list");

  failures.set("list-albums", timeout("list-albums"));
  go("/audiobooks");
  await until(() => panel("catalog"));
  check(!!root.querySelector("nav"), "Navigation remains available after an album timeout");
  failures.delete("list-albums");
  panel("catalog").querySelector("button").click();
  await until(() => text().includes("No albums in this library"));
  check(!panel("catalog"), "Album retry recovers without a page reload");

  failures.set("get-settings", timeout("get-settings"));
  failures.set("list-content-reports", timeout("list-content-reports"));
  go("/settings");
  await until(() => panel("Content reports") && panel("Server settings"));
  check(text().includes("Appearance"), "A server settings failure does not hide local settings");
  check(!text().includes("No content reports"), "A failed reports request does not appear empty");
  failures.delete("list-content-reports");
  panel("Content reports").querySelector("button").click();
  await until(() => text().includes("No content reports"));
  check(!panel("Content reports"), "Reports retry recovers");
  failures.clear();

  failures.set("get-album", timeout("get-album"));
  go("/audiobook/test");
  await until(() => panel("page"));
  go("/settings");
  await until(() => text().includes("Appearance"));
  check(!panel("page"), "Navigation resets a failed page boundary");
  failures.clear();

  const albumPage = (offset) => ({
    albums: Array.from({ length: 60 }, (_, index) => ({
      id: `book-${offset + index}`, title: `Book ${offset + index}`, track_count: 1,
    })), total: 400,
  });
  responses.set("list-albums", ({ offset = 0 }) => {
    if (offset > 0) throw timeout("list-albums");
    return albumPage(offset);
  });
  calls.length = 0;
  go("/audiobooks");
  await until(() => text().includes("Book 0"));
  check(calls.filter((call) => call.operation === "list-albums").every((call) => call.request.offset === 0),
    "Grid view does not fetch the full catalog on entry");
  const scroller = root.querySelector(".catalog-viewport");
  for (const position of [0.4, 0.7]) {
    scroller.scrollTop = scroller.scrollHeight * position;
    scroller.dispatchEvent(new Event("scroll"));
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  await until(() => panel("catalog"));
  const failedOffsets = new Set(calls.filter((call) => call.operation === "list-albums" && call.request.offset > 0).map((call) => call.request.offset));
  check(failedOffsets.size >= 2, "Fixture reached multiple failed catalog pages");
  scroller.scrollTop = 0;
  scroller.dispatchEvent(new Event("scroll"));
  await until(() => text().includes("Book 0"));
  check(!!panel("catalog"), "Loaded albums remain available after a later page fails");
  responses.set("list-albums", ({ offset = 0 }) => albumPage(offset));
  panel("catalog").querySelector("button").click();
  await until(() => !panel("catalog"));
  check([...failedOffsets].every((offset) => calls.filter((call) => call.operation === "list-albums" && call.request.offset === offset).length === 2),
    "One retry recovers all failed pages without repeated automatic requests");
  check(calls.filter((call) => call.operation === "list-albums" && call.request.offset === 0).length === 1,
    "Retry retains pages that already loaded");

  calls.length = 0;
  go("/");
  await until(() => text().includes("Book 0"));
  const view = root.querySelector('select[aria-label="Library view"]'); view.value = "list"; view.dispatchEvent(new Event("change"));
  await until(() => calls.some((call) => call.operation === "list-albums" && call.request.offset === 360));
  await until(() => root.querySelector(".list-cell"));
  check(calls.filter((call) => call.operation === "list-albums").length === 7,
    "List view loads all pages once for sorting and filtering");

  failures.set("list-artists", timeout("list-artists"));
  go("/settings");
  await until(() => text().includes("Appearance"));
  go("/");
  await until(() => root.querySelector('select[aria-label="Browse"]'));
  const browse = root.querySelector('select[aria-label="Browse"]'); browse.value = "artists"; browse.dispatchEvent(new Event("change"));
  await until(() => panel("catalog"));
  failures.delete("list-artists");
  panel("catalog").querySelector("button").click();
  await until(() => !panel("catalog"));
  check(!!root.querySelector("nav"), "Artist retry recovers within the catalog");

  go("/search");
  const search = root.querySelector('input[type="search"]');
  search.value = "kept query";
  search.dispatchEvent(new Event("input"));
  await new Promise(resolve => setTimeout(resolve, 300));
  const pb = usePlayback();
  const track = { id: "one", title: "One", duration_ms: 1000, library: "music", codec: "mp3", sample_rate: 44100, channels: 2, root_relative_path: "one.mp3" };
  pb.enqueue([track, { ...track, id: "two", title: "Two" }]);
  await until(() => text().includes("Queue · 2"));
  [...root.querySelectorAll("button")].find(button => button.textContent === "Queue · 2").click();
  await until(() => root.querySelector('.queue-host:not([hidden])'));
  check(root.querySelector('input[type="search"]') === search && search.value === "kept query", "Queue opening preserves the search field and query");
  await until(() => root.querySelector('button[aria-label="Move Two up"]'));
  root.querySelector('button[aria-label="Move Two up"]').click();
  await until(() => pb.queue[0]?.id === "two");
  check(pb.queue[0].id === "two", "Queue controls reorder tracks");
  await until(() => root.querySelector('button[aria-label="Remove Two"]'));
  root.querySelector('button[aria-label="Remove Two"]').click();
  await until(() => pb.queue.length === 1);
  await pb.undoQueue();
  check(pb.queue.length === 2, "Queue removal can be undone");
  check(!!root.querySelector('select[aria-label="Playback device"]'), "Device selection remains available with the queue open");
  pb.clearQueue();
  pb.enqueue([track, { ...track, id: "two", title: "Two" }]);
  pb.removeAt(0);
  check(pb.snapshot().status === "idle" && pb.current()?.id === "two", "Removing an idle current track does not start playback");
  window.reliabilityStatus = "passed";
} catch (error) {
  window.reliabilityStatus = "failed";
  results.push(String(error));
  console.error(error);
} finally {
  dispose?.();
  usePlayback().dispose(); useServers().dispose();
  window.fetch = originalFetch;
  localStorage.clear();
  for (const [key, value] of originalStorage) localStorage.setItem(key, value);
  const output = document.createElement("pre");
  output.textContent = `${window.reliabilityStatus.toUpperCase()}\n${results.join("\n")}`;
  root.replaceChildren(output);
}
