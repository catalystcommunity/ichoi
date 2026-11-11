// Keep browsing state separate from playback. Queue visits must not clear a search.
const views = new Map<string, unknown>();
export function savedView<T>(key: string, fallback: T): T { return (views.get(key) as T | undefined) ?? fallback; }
export function rememberView(key: string, value: unknown): void { views.set(key, value); }
const queries = new Map<string, string>();
const scrolls = new Map<string, number>();
const returns = new Map<string, string>();
export function savedQuery(server: string): string { return queries.get(server) ?? ""; }
export function rememberQuery(server: string, query: string): void { queries.set(server, query); }
export function resetBrowsing(server: string): void {
  queries.delete(server);
  for (const key of views.keys()) if (key.startsWith(server + "|")) views.delete(key);
  if (typeof window !== "undefined") window.dispatchEvent(new Event("ichoi:reset-browse"));
  for (const key of scrolls.keys()) if (key.startsWith(server + "|")) scrolls.delete(key);
}
export function rememberScroll(key: string, position: number): void { scrolls.set(key, position); }
export function savedScroll(key: string): number { return scrolls.get(key) ?? 0; }
export function rememberQueueReturn(server: string, path: string): void {
  if (path !== "/now-playing") returns.set(server, path);
}
export function queueReturn(server: string): string { return returns.get(server) ?? "/"; }
