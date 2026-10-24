/**
 * Typed wrappers for every Rust IPC command (the TypeScript half of the
 * contract defined in `crates/sr-ipc` and `apps/desktop/src-tauri/src/commands`).
 *
 * Why a wrapper layer:
 * - the rest of the UI never calls `invoke("some_string")` directly, so a
 *   renamed command is fixed in exactly one place;
 * - every call returns a `Result` instead of throwing, which forces the
 *   caller to handle the error case (and keeps components free of try/catch);
 * - outside the desktop app (plain browser during `pnpm dev:ui`) calls fail
 *   with a clear `SR-0003` instead of a cryptic exception.
 *
 * Adding a command: register it in src-tauri `lib.rs`, add one line below,
 * add it to the "IPC map" in PROJECT_MAP.md.
 */
import { invoke, isTauri, type InvokeArgs } from "@tauri-apps/api/core";

import { appError, toAppError } from "./errors";
import type { AppError } from "./generated/AppError";
import type { AppInfo } from "./generated/AppInfo";

/** Outcome of an IPC call: either a value or a typed error. */
export type IpcResult<T> = { ok: true; value: T } | { ok: false; error: AppError };

async function call<T>(command: string, args?: InvokeArgs): Promise<IpcResult<T>> {
  if (!isTauri()) {
    return {
      ok: false,
      error: appError("SR-0003", `'${command}' called outside the desktop app`, { command }),
    };
  }
  try {
    return { ok: true, value: await invoke<T>(command, args) };
  } catch (raw) {
    return { ok: false, error: toAppError(raw) };
  }
}

export const commands = {
  /** Versions and platform. Rust: `commands::app::app_info`. */
  appInfo: () => call<AppInfo>("app_info"),
  /** Create a project (Phase 3; returns SR-0002 for now). Rust: `commands::project::create_project`. */
  createProject: () => call<null>("create_project"),
  /** Open a project (Phase 3; returns SR-0002 for now). Rust: `commands::project::open_project`. */
  openProject: () => call<null>("open_project"),
} as const;
