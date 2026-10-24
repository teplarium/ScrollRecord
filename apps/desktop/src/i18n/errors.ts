/**
 * Turns an `AppError` from the Rust core into a translated sentence.
 * Why: Rust sends only a stable code + parameters (HANDOFF FR-I18N-04); the
 * wording lives in the locale files, so it can be improved or translated
 * without touching Rust. The English technical `message` is the last-resort
 * fallback if a key is ever missing.
 */
import type { TFunction } from "i18next";

import type { AppError } from "../shared/ipc/generated/AppError";

export function formatError(t: TFunction, error: AppError): string {
  return t(`errors.${error.code}`, {
    ...error.params,
    code: error.code,
    defaultValue: error.message,
  });
}
