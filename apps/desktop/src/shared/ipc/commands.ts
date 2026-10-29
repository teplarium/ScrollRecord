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
import type { Flow } from "./generated/Flow";
import type { ImportedFlow } from "./generated/ImportedFlow";
import type { NewProject } from "./generated/NewProject";
import type { OpenedProject } from "./generated/OpenedProject";
import type { ProjectHome } from "./generated/ProjectHome";
import type { JsonValue } from "./generated/serde_json/JsonValue";

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

  // ── Projects (Rust: `commands::project`, logic in `crates/sr-project`) ──
  // A project is identified by its folder `dir`: several can be open as tabs.

  /** Recent projects, tabs to restore, suggested location for a new project. */
  projectHome: () => call<ProjectHome>("project_home"),
  /** Create a project folder and open it. Errors: SR-1006, SR-1002, SR-1001, SR-2001, SR-2003. */
  createProject: (request: NewProject) => call<OpenedProject>("create_project", { request }),
  /** Open a project; `force` = "open anyway" after SR-1005. Errors: SR-1003, SR-1004, SR-1005, SR-2001. */
  openProject: (dir: string, force = false) => call<OpenedProject>("open_project", { dir, force }),
  /** Save scenario and layout (autosave). Errors: SR-1003, SR-1002. */
  saveProject: (dir: string, flow: Flow, layout: JsonValue) =>
    call<null>("save_project", { dir, flow, layout }),
  /** Close a project tab (releases its lock). */
  closeProject: (dir: string) => call<null>("close_project", { dir }),
  /** Remove a project from the recent list; the folder is not touched. */
  forgetRecent: (dir: string) => call<null>("forget_recent", { dir }),
  /** How a picked site folder/address is written into `meta.site` of project `dir`. */
  portableSite: (dir: string, site: string) => call<string | null>("portable_site", { dir, site }),
  /** Read a scenario file to import into the open project `dir` (nothing is written). */
  importFlow: (dir: string, file: string) => call<ImportedFlow>("import_flow", { dir, file }),
  /** Write the project's scenario as one file with the layout embedded. */
  exportFlow: (dir: string, dest: string) => call<null>("export_flow", { dir, dest }),
} as const;
