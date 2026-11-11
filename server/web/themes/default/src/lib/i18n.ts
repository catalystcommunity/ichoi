import en from "../locales/en.json";
const dictionary = new Map<string, string>();
function flatten(value: object, prefix = ""): void {
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof child === "string") dictionary.set(path, child);
    else if (child && typeof child === "object") flatten(child, path);
  }
}
flatten(en);
export type TranslateFn = (key: string, params?: Record<string, string | number>) => string;
export const t: TranslateFn = (key, params = {}) => (dictionary.get(key) ?? key)
  .replace(/\{\{?\s*(\w+)\s*\}?\}/g, (match, name: string) => String(params[name] ?? match));
export function useI18n() { return { t }; }
