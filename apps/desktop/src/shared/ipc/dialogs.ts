/**
 * Native "choose a folder / file" windows (`tauri-plugin-dialog`).
 *
 * Why a wrapper: like `commands.ts`, the rest of the UI never talks to Tauri
 * directly. Outside the desktop app (`pnpm dev:ui` in a browser) there is no
 * native dialog, so these return `null` — the same as the user pressing
 * Cancel — instead of throwing.
 * Permission: `dialog:allow-open` in `src-tauri/capabilities/default.json`.
 */
import { isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/** Lets the user pick one folder. `null` = cancelled or not available. */
export async function pickFolder(title: string, defaultPath?: string): Promise<string | null> {
  if (!isTauri()) return null;
  try {
    const picked = await open({
      directory: true,
      multiple: false,
      title,
      ...(defaultPath ? { defaultPath } : {}),
    });
    return typeof picked === "string" ? picked : null;
  } catch (err) {
    console.error("folder picker failed", err);
    return null;
  }
}

/** Lets the user pick one scenario file (`*.srflow.json`). `null` = cancelled. */
export async function pickScenarioFile(title: string, filterName: string): Promise<string | null> {
  if (!isTauri()) return null;
  try {
    const picked = await open({
      multiple: false,
      title,
      filters: [{ name: filterName, extensions: ["json"] }],
    });
    return typeof picked === "string" ? picked : null;
  } catch (err) {
    console.error("file picker failed", err);
    return null;
  }
}
