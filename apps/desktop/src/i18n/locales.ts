/**
 * Locale discovery.
 * Why: adding a language must not require code changes. Copy `locales/en` to
 * `locales/<code>` (e.g. `uk`, `sk`), translate the values, done — the folder
 * is picked up here automatically and appears in the language picker.
 * Kept separate from `index.ts` (no side effects) so tests can import it.
 */
import type en from "./locales/en/common.json";

/** Shape of one locale file. English defines it; others must match. */
export type LocaleMessages = typeof en;

/** The source language and fallback for missing keys. */
export const DEFAULT_LANGUAGE = "en";

const modules = import.meta.glob<LocaleMessages>("./locales/*/common.json", {
  eager: true,
  import: "default",
});

/** All bundled locales, keyed by language code (`en`, …). */
export const LOCALES: Record<string, LocaleMessages> = Object.fromEntries(
  Object.entries(modules).flatMap(([path, messages]) => {
    const code = /\.\/locales\/([^/]+)\/common\.json$/.exec(path)?.[1];
    return code ? [[code, messages] as const] : [];
  }),
);

/** True if a locale folder exists for this language code. */
export function isSupportedLanguage(code: string): boolean {
  return Object.hasOwn(LOCALES, code);
}

/** Languages for the picker, each named in its own language ("English"). */
export function availableLanguages(): { code: string; name: string }[] {
  return Object.entries(LOCALES)
    .map(([code, messages]) => ({ code, name: messages.meta.languageName }))
    .sort((a, b) => a.name.localeCompare(b.name));
}
