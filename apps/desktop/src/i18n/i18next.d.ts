/**
 * Typed translation keys.
 * Why: `t("start.heding")` (typo) fails at compile time instead of showing a
 * raw key to users. Dynamic keys such as `errors.${code}` are not checked
 * here; `locales.test.ts` verifies that every Rust error code has English text.
 */
import "i18next";

import type en from "./locales/en/common.json";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "common";
    resources: { common: typeof en };
    returnNull: false;
  }
}
