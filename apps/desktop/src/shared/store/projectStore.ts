/**
 * Project store — everything one project tab holds while it is open
 * (ADR 0006 §3): the document `{flow, layout}` with its undo history, the
 * view (shown device, selection — never saved, never undone) and autosave.
 *
 * Why a registry by folder and not React state: `App.tsx` mounts only the
 * shown tab's editor. Switching tabs must keep each tab's history and its
 * unsaved changes, so the stores live here, are created when a tab is first
 * shown and dropped when the tab closes (after a last save).
 *
 * Saving (owner's answer 2, FR-PRJ-02): ~1 s after the last edit, at once on
 * Ctrl+S, before a tab closes and before the window closes. Writes go one at
 * a time (a queue); each writes the latest state, so a write that started
 * before an edit is followed by another. Rust writes atomically (`.tmp` →
 * rename, `.bak` kept), so a crash loses at most about a second.
 * The canvas viewport is remembered without an undo step and written with
 * the next save.
 *
 * Related: `edits.ts` (what an edit does), `history.ts` (undo rules),
 * `features/editor/Editor.tsx` (uses it), `app/App.tsx` (closes it).
 */
import { useStore } from "zustand";
import { createStore, type StoreApi } from "zustand/vanilla";

import { commands } from "../ipc/commands";
import type { AppError } from "../ipc/generated/AppError";
import type { OpenedProject } from "../ipc/generated/OpenedProject";
import type { NodeSource, ProjectDoc } from "./edits";
import { commit, redo, startHistory, undo, type History } from "./history";
import { readLayout, type CanvasLayout, type CanvasViewport } from "./layout";

/** Wait after the last edit before writing to disk. */
export const AUTOSAVE_DELAY_MS = 1000;

export type SaveStatus = "saved" | "pending" | "saving" | "failed";

/** A request for the canvas to select a node (and with `show`, pan to it). */
export type Focus = { id: string; seq: number; show: boolean };

/** A request for the canvas to add a site map row in the middle of the view. */
export type AddRequest = { source: NodeSource; seq: number };

export type ProjectState = {
  dir: string;
  history: History<ProjectDoc>;
  /** Pan and zoom of the canvas; saved, not undone. */
  viewport: CanvasViewport | null;
  /** The scenario names a site folder that doesn't exist (until located). */
  siteMissing: boolean;
  // ── View: never saved, never undone ──
  targetId: string | null;
  selectedId: string | null;
  focus: Focus | null;
  addRequest: AddRequest | null;
  // ── Saving ──
  save: { status: SaveStatus; error: AppError | null };
};

export type ProjectActions = {
  /** Applies one edit as one undo step (same `coalesceKey` in a row = one step). */
  edit: (change: (doc: ProjectDoc) => ProjectDoc, coalesceKey?: string) => void;
  undo: () => void;
  redo: () => void;
  setViewport: (viewport: CanvasViewport) => void;
  setView: (view: Partial<Pick<ProjectState, "targetId" | "selectedId">>) => void;
  /**
   * Asks the canvas to add `source` (a clicked site map row) in a free spot
   * near the middle of the view; only the canvas knows the view and shapes.
   */
  requestAdd: (source: NodeSource) => void;
  /** Asks the canvas to select node `id` and, with `show`, bring it into view. */
  focusNode: (id: string, show?: boolean) => void;
  siteLocated: () => void;
  /** Writes now (after any write in progress). `true` when all is on disk. */
  saveNow: () => Promise<boolean>;
  dismissSaveError: () => void;
};

export type ProjectStore = StoreApi<ProjectState & ProjectActions>;

/** `layout.json` content to write: positions plus the current viewport. */
export function layoutToSave(layout: CanvasLayout, viewport: CanvasViewport | null): CanvasLayout {
  const out: CanvasLayout = { version: 1, nodes: layout.nodes };
  if (viewport) out.viewport = viewport;
  return out;
}

function createProjectStore(project: OpenedProject): ProjectStore {
  const layout = readLayout(project.layout);
  const doc: ProjectDoc = { flow: project.flow, layout };

  // What is on disk: compared by identity (edits never mutate, undo restores
  // the same objects), so undoing back to the saved state is "saved" again.
  let savedDoc = doc;
  let savedViewport = layout.viewport ?? null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let queue: Promise<boolean> = Promise.resolve(true);

  const store = createStore<ProjectState & ProjectActions>()((set, get) => {
    const isSaved = () => get().history.present === savedDoc && get().viewport === savedViewport;

    // ── One write of the latest state ──
    async function write(): Promise<boolean> {
      if (isSaved()) {
        set({ save: { status: "saved", error: null } });
        return true;
      }
      const { dir, history, viewport } = get();
      const writing = history.present;
      set({ save: { status: "saving", error: null } });
      const result = await commands.saveProject(
        dir,
        writing.flow,
        layoutToSave(writing.layout, viewport),
      );
      if (!result.ok) {
        set({ save: { status: "failed", error: result.error } });
        return false;
      }
      savedDoc = writing;
      savedViewport = viewport;
      set({ save: { status: isSaved() ? "saved" : "pending", error: null } });
      return true;
    }

    function saveNow() {
      if (timer) clearTimeout(timer);
      timer = null;
      queue = queue.then(write);
      return queue;
    }

    function changed() {
      if (isSaved()) {
        if (timer) clearTimeout(timer);
        timer = null;
        set({ save: { status: "saved", error: null } });
        return;
      }
      set((s) => ({
        save: { ...s.save, status: s.save.status === "failed" ? "failed" : "pending" },
      }));
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => void saveNow(), AUTOSAVE_DELAY_MS);
    }

    return {
      dir: project.dir,
      history: startHistory(doc),
      viewport: savedViewport,
      siteMissing: project.siteMissing,
      targetId: project.flow.targets[0]?.id ?? null,
      selectedId: null,
      focus: null,
      addRequest: null,
      save: { status: "saved", error: null },

      edit(change, coalesceKey) {
        const before = get().history;
        const after = commit(before, change(before.present), coalesceKey);
        if (after === before) return;
        set({ history: after });
        changed();
      },
      undo() {
        set((s) => ({ history: undo(s.history) }));
        changed();
      },
      redo() {
        set((s) => ({ history: redo(s.history) }));
        changed();
      },
      setViewport(viewport) {
        // Written with the next save or on close: panning alone doesn't write.
        set({ viewport });
      },
      setView(view) {
        set(view);
      },
      focusNode(id, show = true) {
        set((s) => ({ selectedId: id, focus: { id, seq: (s.focus?.seq ?? 0) + 1, show } }));
      },
      requestAdd(source) {
        set((s) => ({ addRequest: { source, seq: (s.addRequest?.seq ?? 0) + 1 } }));
      },
      siteLocated() {
        set({ siteMissing: false });
      },
      saveNow,
      dismissSaveError() {
        set((s) => ({ save: { ...s.save, error: null } }));
      },
    };
  });
  return store;
}

// ── Registry: one store per open project folder ──

// Development only: when Vite reloads this file after a code change, keep the
// open tabs' stores. A fresh registry would rebuild them from the project as
// it was when the tab opened, and the next save would write that old state
// over the user's edits.
const stores: Map<string, ProjectStore> =
  (import.meta.hot?.data as { stores?: Map<string, ProjectStore> } | undefined)?.stores ??
  new Map();
if (import.meta.hot) (import.meta.hot.data as { stores?: unknown }).stores = stores;

/** The store of an opened project (created on first use). */
export function projectStore(project: OpenedProject): ProjectStore {
  let store = stores.get(project.dir);
  if (!store) {
    store = createProjectStore(project);
    stores.set(project.dir, store);
  }
  return store;
}

/**
 * Before a tab closes: saves, then forgets the store. `false` (store kept,
 * error shown in its tab) when the save failed.
 */
export async function closeProjectStore(dir: string): Promise<boolean> {
  const store = stores.get(dir);
  if (!store) return true;
  const saved = await store.getState().saveNow();
  if (saved) stores.delete(dir);
  return saved;
}

/** Before the window closes: saves every open tab. `true` if all saved. */
export async function saveAllProjects(): Promise<boolean> {
  const results = await Promise.all([...stores.values()].map((s) => s.getState().saveNow()));
  return results.every(Boolean);
}

/** React hook: a slice of a project store. */
export function useProject<T>(
  store: ProjectStore,
  pick: (state: ProjectState & ProjectActions) => T,
): T {
  return useStore(store, pick);
}
