const CACHE = "ichoi-shell-v2";

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(
  Promise.all([
    self.clients.claim(),
    caches.keys().then((keys) => Promise.all(
      keys.filter((key) => key.startsWith("ichoi-shell-") && key !== CACHE)
        .map((key) => caches.delete(key)),
    )),
  ]),
));
self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (
    url.origin !== location.origin ||
    url.pathname.startsWith("/media/") ||
    url.pathname.startsWith("/api/") ||
    url.pathname === "/status" ||
    url.pathname === "/healthz" ||
    url.pathname === "/ws"
  ) return;
  event.respondWith(
    fetch(request, { cache: request.mode === "navigate" ? "no-store" : "default" })
      .then((response) => {
        if (response.ok) {
          const copy = response.clone();
          void caches.open(CACHE).then((cache) => cache.put(request, copy));
        }
        return response;
      })
      .catch(async () => (await caches.match(request)) ?? Response.error()),
  );
});
