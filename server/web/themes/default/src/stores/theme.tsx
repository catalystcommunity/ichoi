// Appearance store. Theme is `system` (follow prefers-color-scheme), `light`, or
// `dark`. The user's choice persists locally; the *default* can come from the
// server's `settings` (DESIGN §11: theme settings live in the DB so any UI honors
// them). The resolved theme is stamped on <html data-theme> so CSS can override
// the media-query default in both directions.

import {
  createContext,
  createEffect,
  createSignal,
  onCleanup,
  useContext,
  type Accessor,
  type JSX,
  type ParentProps,
} from "solid-js";

export type ThemeMode = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "ichoi.theme";

interface ThemeContextValue {
  mode: Accessor<ThemeMode>;
  resolved: Accessor<ResolvedTheme>;
  setMode: (m: ThemeMode) => void;
  /** Apply a server-provided default (only used when the user hasn't chosen). */
  applyServerDefault: (mode: ThemeMode) => void;
}

const ThemeContext = createContext<ThemeContextValue>();

function storedMode(): ThemeMode | undefined {
  const v = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
  return v === "system" || v === "light" || v === "dark" ? v : undefined;
}

export function ThemeProvider(props: ParentProps): JSX.Element {
  const [userChose, setUserChose] = createSignal(storedMode() !== undefined);
  const [mode, setModeSignal] = createSignal<ThemeMode>(storedMode() ?? "system");

  const mql =
    typeof matchMedia !== "undefined" ? matchMedia("(prefers-color-scheme: dark)") : undefined;
  const [systemDark, setSystemDark] = createSignal(mql?.matches ?? true);

  if (mql) {
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mql.addEventListener("change", onChange);
    onCleanup(() => mql.removeEventListener("change", onChange));
  }

  const resolved = (): ResolvedTheme => {
    const m = mode();
    if (m === "system") return systemDark() ? "dark" : "light";
    return m;
  };

  createEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.dataset.theme = resolved();
    }
  });

  const setMode = (m: ThemeMode) => {
    setUserChose(true);
    if (typeof localStorage !== "undefined") localStorage.setItem(STORAGE_KEY, m);
    setModeSignal(m);
  };

  const applyServerDefault = (m: ThemeMode) => {
    if (!userChose()) setModeSignal(m);
  };

  return (
    <ThemeContext.Provider value={{ mode, resolved, setMode, applyServerDefault }}>
      {props.children}
    </ThemeContext.Provider>
  );
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used within <ThemeProvider>");
  return ctx;
}
