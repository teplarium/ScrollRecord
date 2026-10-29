/**
 * Project tabs — which projects are open in this window and which one is
 * shown (HANDOFF D-38: tabs like file tabs in Figma).
 *
 * Why a pure reducer: the rules (no duplicate tab for one project, which
 * tab becomes active after closing) are easy to get subtly wrong, so they
 * live here without React and are unit-tested in `tabs.test.ts`.
 * `active === null` means the Home tab (start screen), which always exists.
 */
import type { OpenedProject } from "../shared/ipc/generated/OpenedProject";

export type TabsState = {
  /** Open projects in tab order. */
  projects: OpenedProject[];
  /** Folder of the shown project, or `null` for the Home tab. */
  active: string | null;
};

export type TabsAction =
  /** A project was opened or created: add its tab (or refresh it) and show it. */
  | { type: "opened"; project: OpenedProject; activate: boolean }
  /** Show a project tab, or Home with `null`. */
  | { type: "activate"; dir: string | null }
  /** A project tab was closed. */
  | { type: "closed"; dir: string }
  /** The project's data changed (e.g. its site folder was located). */
  | { type: "updated"; project: OpenedProject };

export const initialTabs: TabsState = { projects: [], active: null };

/**
 * True if two folder paths name the same project. Paths come back from Rust
 * as typed or picked, so slashes, a trailing separator and (on Windows)
 * letter case may differ.
 */
export function sameDir(a: string, b: string): boolean {
  const key = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  return key(a) === key(b);
}

export function tabsReducer(state: TabsState, action: TabsAction): TabsState {
  switch (action.type) {
    case "opened": {
      const { project } = action;
      const exists = state.projects.some((p) => sameDir(p.dir, project.dir));
      const projects = exists
        ? state.projects.map((p) => (sameDir(p.dir, project.dir) ? project : p))
        : [...state.projects, project];
      const existing = state.projects.find((p) => sameDir(p.dir, project.dir));
      const dir = existing ? existing.dir : project.dir;
      return { projects, active: action.activate ? dir : state.active };
    }
    case "activate": {
      if (action.dir === null) return { ...state, active: null };
      const found = state.projects.find((p) => sameDir(p.dir, action.dir ?? ""));
      return found ? { ...state, active: found.dir } : state;
    }
    case "closed": {
      const index = state.projects.findIndex((p) => sameDir(p.dir, action.dir));
      if (index < 0) return state;
      const projects = state.projects.filter((_, i) => i !== index);
      if (state.active === null || !sameDir(state.active, action.dir)) {
        return { ...state, projects };
      }
      // The shown tab was closed: show its right neighbour, else the left
      // one, else Home — the same as closing a file tab in Figma.
      const next = projects[index] ?? projects[index - 1];
      return { projects, active: next ? next.dir : null };
    }
    case "updated":
      return {
        ...state,
        projects: state.projects.map((p) =>
          sameDir(p.dir, action.project.dir) ? action.project : p,
        ),
      };
  }
}

/** The project shown right now, or `undefined` on the Home tab. */
export function activeProject(state: TabsState): OpenedProject | undefined {
  return state.active === null
    ? undefined
    : state.projects.find((p) => sameDir(p.dir, state.active ?? ""));
}
