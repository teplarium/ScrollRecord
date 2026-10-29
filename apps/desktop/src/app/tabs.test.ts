import { describe, expect, it } from "vitest";

import type { OpenedProject } from "../shared/ipc/generated/OpenedProject";
import { activeProject, initialTabs, sameDir, tabsReducer, type TabsState } from "./tabs";

// Only `dir` and the name matter to the tab rules; the rest is filler.
function project(dir: string, name = dir): OpenedProject {
  return {
    dir,
    manifest: { name } as OpenedProject["manifest"],
    flow: {} as OpenedProject["flow"],
    layout: {},
    siteMissing: false,
  };
}

function open(state: TabsState, dir: string, activate = true): TabsState {
  return tabsReducer(state, { type: "opened", project: project(dir), activate });
}

describe("project tabs", () => {
  it("opening adds a tab and shows it", () => {
    const s = open(open(initialTabs, "C:\\P\\A"), "C:\\P\\B");
    expect(s.projects.map((p) => p.dir)).toEqual(["C:\\P\\A", "C:\\P\\B"]);
    expect(s.active).toBe("C:\\P\\B");
  });

  it("opening an already open project switches to its tab, no duplicate", () => {
    let s = open(open(initialTabs, "C:\\P\\A"), "C:\\P\\B");
    s = open(s, "c:/p/a/");
    expect(s.projects).toHaveLength(2);
    expect(s.active).toBe("C:\\P\\A");
  });

  it("restoring tabs without activating keeps Home shown", () => {
    const s = open(initialTabs, "/p/a", false);
    expect(s.active).toBeNull();
    expect(activeProject(s)).toBeUndefined();
  });

  it("closing the shown tab shows the right neighbour, then the left, then Home", () => {
    let s = open(open(open(initialTabs, "a"), "b"), "c");
    s = tabsReducer(s, { type: "activate", dir: "b" });
    s = tabsReducer(s, { type: "closed", dir: "b" });
    expect(s.active).toBe("c");
    s = tabsReducer(s, { type: "closed", dir: "c" });
    expect(s.active).toBe("a");
    s = tabsReducer(s, { type: "closed", dir: "a" });
    expect(s.active).toBeNull();
    expect(s.projects).toHaveLength(0);
  });

  it("closing a background tab keeps the shown one", () => {
    let s = open(open(initialTabs, "a"), "b");
    s = tabsReducer(s, { type: "closed", dir: "a" });
    expect(s.active).toBe("b");
  });

  it("an update replaces the project data in its tab", () => {
    let s = open(initialTabs, "a");
    s = tabsReducer(s, { type: "updated", project: { ...project("a"), siteMissing: true } });
    expect(activeProject(s)?.siteMissing).toBe(true);
  });

  it("compares folders like the OS does", () => {
    expect(sameDir("C:\\Проекты\\Promo", "c:/проекты/promo/")).toBe(true);
    expect(sameDir("/a/b", "/a/c")).toBe(false);
  });
});
