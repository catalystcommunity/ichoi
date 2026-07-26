const SERVER_VERSION_KEY = "ichoi.serverVersion";
const UPDATE_RELOAD_KEY = "ichoi.updateReload";

interface ServerStatus {
  version?: string;
}

function statusUrl(websocketUrl: string): string {
  const url = new URL(websocketUrl);
  url.protocol = url.protocol === "wss:" ? "https:" : "http:";
  url.pathname = "/status";
  url.search = `update_check=${Date.now()}`;
  url.hash = "";
  return url.toString();
}

function storageGet(key: string): string | null {
  try {
    return sessionStorage.getItem(key);
  } catch {
    return null;
  }
}

function storageSet(key: string, value: string): void {
  try {
    sessionStorage.setItem(key, value);
  } catch {
    /* A kiosk with blocked storage can still reconnect; it just cannot compare releases. */
  }
}

export function updateReloadInProgress(): boolean {
  return storageGet(UPDATE_RELOAD_KEY) === "true";
}

export function finishUpdateReload(): void {
  try {
    sessionStorage.removeItem(UPDATE_RELOAD_KEY);
  } catch {
    /* ignore */
  }
}

/**
 * Compare the release behind a newly re-established socket with the release previously seen
 * by this document. The queue, current track, and position live on the server, so a cache-busted
 * document reload is enough to restore a satellite kiosk without copying transient queue data.
 */
export async function reloadSatelliteForUpdate(websocketUrl: string): Promise<boolean> {
  let response: Response;
  try {
    response = await fetch(statusUrl(websocketUrl), {
      cache: "no-store",
      headers: { "cache-control": "no-cache" },
    });
  } catch {
    return false;
  }
  if (!response.ok) return false;
  const status = await response.json() as ServerStatus;
  const version = status.version?.trim();
  if (!version) return false;

  const previous = storageGet(SERVER_VERSION_KEY);
  storageSet(SERVER_VERSION_KEY, version);
  if (!previous || previous === version) return false;

  storageSet(UPDATE_RELOAD_KEY, "true");
  window.dispatchEvent(new CustomEvent("ichoi:update-reloading", { detail: { version } }));

  if ("serviceWorker" in navigator) {
    try {
      const registration = await navigator.serviceWorker.getRegistration();
      if (registration) {
        await Promise.race([
          registration.update(),
          new Promise<void>((resolve) => setTimeout(resolve, 2_000)),
        ]);
      }
    } catch {
      /* The network-first document reload remains sufficient without a service worker update. */
    }
  }

  const resume = new URL(location.href);
  resume.searchParams.set("ichoi_updated", version);
  setTimeout(() => location.replace(`${resume.pathname}${resume.search}${resume.hash}`), 250);
  return true;
}
