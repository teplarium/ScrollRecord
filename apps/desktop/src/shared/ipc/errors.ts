/**
 * Normalises anything thrown across the IPC bridge into a typed `AppError`.
 * Why: the UI must handle exactly one error shape. Rust commands already
 * return `AppError`; anything else (a JS exception, a Tauri transport error)
 * becomes `SR-0001` so it is still visible, logged and reportable.
 */
import type { AppError } from "./generated/AppError";
import type { ErrorCode } from "./generated/ErrorCode";
import errorCodes from "./generated/error-codes.json";

const KNOWN_CODES: ReadonlySet<string> = new Set(errorCodes);

/** Builds an `AppError` on the TypeScript side (same shape as Rust's). */
export function appError(
  code: ErrorCode,
  message: string,
  params: Record<string, string> = {},
): AppError {
  return { code, message, params };
}

/** True if the value has the exact shape of a Rust `AppError`. */
export function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.code === "string" &&
    KNOWN_CODES.has(v.code) &&
    typeof v.message === "string" &&
    typeof v.params === "object" &&
    v.params !== null
  );
}

/** Converts any thrown value into an `AppError`. */
export function toAppError(raw: unknown): AppError {
  if (isAppError(raw)) return raw;
  const message =
    raw instanceof Error ? raw.message : typeof raw === "string" ? raw : JSON.stringify(raw);
  return appError("SR-0001", `unexpected IPC failure: ${message}`);
}
