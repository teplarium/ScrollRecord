/**
 * Safe access to per-device UI preferences (localStorage).
 * Why: storage can be unavailable or throw (private mode, locked profile).
 * A failed read must never break the app — it just falls back to defaults.
 * Only small UI conveniences belong here (e.g. language). Project data and
 * real settings are stored by the Rust core in files.
 */
const PREFIX = "scrollrecord.";

/** Reads a saved preference, or `null` if absent or storage is unavailable. */
export function readSetting(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(PREFIX + key) ?? null;
  } catch {
    return null;
  }
}

/** Saves a preference; silently does nothing if storage is unavailable. */
export function writeSetting(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(PREFIX + key, value);
  } catch {
    // Not fatal: the preference simply won't be remembered.
  }
}
