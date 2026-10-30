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

/** Names of the file-type choices shown at the bottom of a file window. */
export type FileFilterNames = { files: string; all: string };

// Every file window also offers "All files": with only one filter Windows shows
// an empty-looking folder when nothing matches, and users read that as broken.
async function pickFile(
  title: string,
  filter: { name: string; extensions: string[] },
  allName: string,
  defaultPath?: string,
): Promise<string | null> {
  if (!isTauri()) return null;
  try {
    const picked = await open({
      multiple: false,
      title,
      filters: [filter, { name: allName, extensions: ["*"] }],
      ...(defaultPath ? { defaultPath } : {}),
    });
    return typeof picked === "string" ? picked : null;
  } catch (err) {
    console.error("file picker failed", err);
    return null;
  }
}

/** Lets the user pick one scenario file (`*.srflow.json`). `null` = cancelled. */
export function pickScenarioFile(title: string, names: FileFilterNames): Promise<string | null> {
  return pickFile(title, { name: names.files, extensions: ["json"] }, names.all);
}

/**
 * Lets the user pick a site folder by choosing its start page (`index.html`)
 * and returns that page's folder. `null` = cancelled.
 * Why not a folder window: Windows folder windows never show files, so a site
 * folder (only `index.html`, scripts, styles) looks empty there. Choosing the
 * page shows the files the user recognises.
 */
export async function pickSiteFolder(
  title: string,
  names: FileFilterNames,
  defaultPath?: string,
): Promise<string | null> {
  const page = await pickFile(
    title,
    { name: names.files, extensions: ["html", "htm"] },
    names.all,
    defaultPath,
  );
  return page === null ? null : parentFolder(page);
}

/**
 * Lets the user pick a project folder by choosing a file in it (`project.json`)
 * and returns that folder. `null` = cancelled. Same reason as `pickSiteFolder`:
 * a folder window would show only the project's `cache` and `renders`.
 */
export async function pickProjectFolder(
  title: string,
  names: FileFilterNames,
  defaultPath?: string,
): Promise<string | null> {
  const file = await pickFile(
    title,
    { name: names.files, extensions: ["json"] },
    names.all,
    defaultPath,
  );
  return file === null ? null : parentFolder(file);
}

/** The folder part of a file path (`C:\site\index.html` → `C:\site`), either separator. */
export function parentFolder(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (cut < 0) return path;
  // Keep the root's separator: `C:\index.html` → `C:\`, `/index.html` → `/`.
  const isRoot = cut === 0 || (cut === 2 && path[1] === ":");
  return path.slice(0, isRoot ? cut + 1 : cut);
}
