import { State } from "../lib/state.ts";
export type ThemeMode = "system" | "light" | "dark";
const state = new State();
let saved: string | null = null;
try { saved = localStorage.getItem("ichoi.theme"); } catch { /* Private browsing. */ }
let userChose = saved !== null;
const [mode, update] = state.cell<ThemeMode>("mode", saved === "light" || saved === "dark" ? saved : "system");
const media = matchMedia("(prefers-color-scheme: dark)");
const resolved = () => mode() === "system" ? (media.matches ? "dark" : "light") : mode();
const apply = () => { document.documentElement.dataset.theme = resolved(); };
media.addEventListener("change", apply);
state.subscribe(apply);
apply();
export function useTheme() {
  return { mode, resolved, subscribe: (listener: () => void) => state.subscribe(listener),
    setMode(value: ThemeMode) { userChose = true; update(value); try { localStorage.setItem("ichoi.theme", value); } catch { /* Private browsing. */ } },
    applyServerDefault(value: ThemeMode) { if (!userChose) update(value); } };
}
