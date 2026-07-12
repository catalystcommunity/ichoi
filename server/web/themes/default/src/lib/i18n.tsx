// i18n from the first commit (DESIGN §11). Built on @solid-primitives/i18n: a
// flattened dictionary + a reactive translator, wrapped in a context so any
// component can call `t("nav.library")`. English is bundled; more locales load
// lazily. The same primitives are available to third-party themes.

import { flatten, resolveTemplate, translator, type Flatten } from "@solid-primitives/i18n";
import {
  createContext,
  createResource,
  createSignal,
  useContext,
  type JSX,
  type ParentProps,
} from "solid-js";
import en from "../locales/en.json";

export type RawDict = typeof en;
export type Dict = Flatten<RawDict>;

/** Locales we can load. English is bundled; others are dynamically imported. */
export const LOCALES = ["en"] as const;
export type Locale = (typeof LOCALES)[number] | string;

const BUNDLED: Record<string, RawDict> = { en };

async function fetchDict(locale: Locale): Promise<Dict> {
  if (BUNDLED[locale]) return flatten(BUNDLED[locale]!);
  try {
    // Vite code-splits each locale JSON into its own chunk.
    const mod = (await import(`../locales/${locale}.json`)) as { default: RawDict };
    return flatten(mod.default);
  } catch {
    console.warn(`[i18n] locale '${locale}' not found, falling back to en`);
    return flatten(en);
  }
}

export type TranslateFn = (
  key: keyof Dict,
  params?: Record<string, string | number>,
) => string;

interface I18nContextValue {
  t: TranslateFn;
  locale: () => Locale;
  setLocale: (l: Locale) => void;
}

const I18nContext = createContext<I18nContextValue>();

const STORAGE_KEY = "ichoi.locale";

function initialLocale(): Locale {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
  if (stored) return stored;
  const nav = typeof navigator !== "undefined" ? navigator.language.split("-")[0] : "en";
  return nav || "en";
}

export function I18nProvider(props: ParentProps): JSX.Element {
  const [locale, setLocaleSignal] = createSignal<Locale>(initialLocale());
  const [dict] = createResource(locale, fetchDict, { initialValue: flatten(en) });

  const translate = translator(dict, resolveTemplate) as unknown as (
    key: string,
    params?: Record<string, string | number>,
  ) => unknown;

  // The translator's flattened dictionary yields string leaves; coerce defensively
  // and fall back to the key itself if a translation is missing.
  const t: TranslateFn = (key, params) => {
    const out = translate(key as string, params);
    return typeof out === "string" ? out : (key as string);
  };

  const setLocale = (l: Locale) => {
    if (typeof localStorage !== "undefined") localStorage.setItem(STORAGE_KEY, l);
    if (typeof document !== "undefined") document.documentElement.lang = l;
    setLocaleSignal(l);
  };

  return (
    <I18nContext.Provider value={{ t, locale, setLocale }}>
      {props.children}
    </I18nContext.Provider>
  );
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within <I18nProvider>");
  return ctx;
}
