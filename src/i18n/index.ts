import { useCallback, useSyncExternalStore } from "react";
import { en, type MessageKey, type Messages } from "./en";
import { fr } from "./fr";
import { es } from "./es";

export type { MessageKey } from "./en";

const MESSAGES = { en, fr, es } satisfies Record<string, Messages>;
export type Language = keyof typeof MESSAGES;
/** "system" follows the OS language, read through the WebView. */
export type LanguagePreference = Language | "system";

export const LANGUAGES = Object.keys(MESSAGES) as Language[];
/** Each language named in itself, so a reader can find their own in any UI language. */
export const LANGUAGE_NAMES: Record<Language, string> = { en: "English", fr: "Français", es: "Español" };

const STORAGE_KEY = "language";

function isLanguage(value: string): value is Language {
  return (LANGUAGES as string[]).includes(value);
}

export function systemLanguage(): Language {
  const tags = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const tag of tags) {
    const base = tag.toLowerCase().split("-")[0];
    if (isLanguage(base)) return base;
  }
  return "en";
}

function readPreference(): LanguagePreference {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && isLanguage(saved)) return saved;
  } catch {
    /* storage unavailable: follow the system */
  }
  return "system";
}

interface State {
  preference: LanguagePreference;
  language: Language;
}

function resolve(preference: LanguagePreference): State {
  return { preference, language: preference === "system" ? systemLanguage() : preference };
}

let state = resolve(readPreference());
const listeners = new Set<() => void>();

function apply(next: State) {
  state = next;
  document.documentElement.lang = next.language;
  listeners.forEach((l) => l());
}

document.documentElement.lang = state.language;

// Pop-out windows share localStorage, so a change in one window reaches the others.
window.addEventListener("storage", (e) => {
  if (e.key === STORAGE_KEY || e.key === null) apply(resolve(readPreference()));
});

export function setLanguagePreference(preference: LanguagePreference) {
  try {
    if (preference === "system") localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, preference);
  } catch {
    /* the choice still applies to this window */
  }
  apply(resolve(preference));
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function interpolate(text: string, vars?: Record<string, string | number>): string {
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) => (name in vars ? String(vars[name]) : match));
}

export type Translate = (key: MessageKey, vars?: Record<string, string | number>) => string;

export function useI18n() {
  const { preference, language } = useSyncExternalStore(subscribe, () => state);
  const t: Translate = useCallback(
    (key, vars) => interpolate(MESSAGES[language][key], vars),
    [language],
  );
  // "4.8K viewers" in English, "4,8 k spectateurs" in French.
  const viewers = useCallback(
    (count: number) => {
      const form = pluralRules(language).select(count) === "one" ? "viewers_one" : "viewers_other";
      return interpolate(MESSAGES[language][form], { count: formatViewers(count, language) });
    },
    [language],
  );
  return { language, preference, t, viewers };
}

const compactFormats = new Map<Language, Intl.NumberFormat>();
const pluralRulesCache = new Map<Language, Intl.PluralRules>();

function pluralRules(language: Language): Intl.PluralRules {
  let rules = pluralRulesCache.get(language);
  if (!rules) {
    rules = new Intl.PluralRules(language);
    pluralRulesCache.set(language, rules);
  }
  return rules;
}

/** Short viewer count in the language's own style: 4.8K, 4,8 k, 4,8 mil. */
export function formatViewers(count: number, language: Language): string {
  let format = compactFormats.get(language);
  if (!format) {
    format = new Intl.NumberFormat(language, { notation: "compact", maximumFractionDigits: 1 });
    compactFormats.set(language, format);
  }
  return format.format(count);
}
