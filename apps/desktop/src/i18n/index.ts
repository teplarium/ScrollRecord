/**
 * i18n setup (i18next + react-i18next).
 * Why: every visible string comes from `locales/<lang>/common.json`
 * (HANDOFF R-12, D-12). English is the source and the fallback: if a
 * translation misses a key, the English text is shown instead of a raw key.
 * The chosen language is remembered per device (see shared/storage.ts).
 */
import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import { readSetting, writeSetting } from "../shared/storage";
import { DEFAULT_LANGUAGE, LOCALES, isSupportedLanguage } from "./locales";

const LANGUAGE_SETTING = "language";

// ── Initial language ────────────────────────────────────────────────────────
// Saved choice wins; otherwise English. (OS-language detection can be added
// once more than one locale exists.)
const saved = readSetting(LANGUAGE_SETTING);
const initialLanguage = saved && isSupportedLanguage(saved) ? saved : DEFAULT_LANGUAGE;

void i18n.use(initReactI18next).init({
  resources: Object.fromEntries(
    Object.entries(LOCALES).map(([code, common]) => [code, { common }]),
  ),
  lng: initialLanguage,
  fallbackLng: DEFAULT_LANGUAGE,
  defaultNS: "common",
  ns: ["common"],
  interpolation: { escapeValue: false }, // React already escapes output
  returnNull: false,
});

/** Switches the UI language and remembers the choice on this device. */
export function changeLanguage(code: string): void {
  if (!isSupportedLanguage(code)) return;
  writeSetting(LANGUAGE_SETTING, code);
  void i18n.changeLanguage(code);
}

export default i18n;
