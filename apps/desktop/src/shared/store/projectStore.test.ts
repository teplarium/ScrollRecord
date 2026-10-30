import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { commands } from "../ipc/commands";
import type { OpenedProject } from "../ipc/generated/OpenedProject";
import { addBlock, moveNodes } from "./edits";
import {
  AUTOSAVE_DELAY_MS,
  closeProjectStore,
  projectStore,
  saveAllProjects,
} from "./projectStore";

vi.mock("../ipc/commands", () => ({ commands: { saveProject: vi.fn() } }));
const save = vi.mocked(commands.saveProject);

let n = 0;
function opened(): OpenedProject {
  return {
    dir: `C:/p/${++n}`,
    manifest: {} as OpenedProject["manifest"],
    flow: { targets: [{ id: "desktop" }], nodes: [], edges: [], meta: { name: "p" } },
    layout: null,
    siteMissing: false,
  } as unknown as OpenedProject;
}

const hero = { page: "home", name: "hero", locators: [{ sr: "hero" }] };

beforeEach(() => {
  vi.useFakeTimers();
  save.mockReset();
  save.mockResolvedValue({ ok: true, value: null });
});
afterEach(() => vi.useRealTimers());

describe("project store", () => {
  it("autosaves about a second after the last edit, once", async () => {
    const store = projectStore(opened());
    const { edit } = store.getState();
    edit((d) => addBlock(d, hero, { x: 0, y: 0 }).doc);
    edit((d) => moveNodes(d, { hero: { x: 50, y: 0 } }));
    expect(store.getState().save.status).toBe("pending");
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS - 10);
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(20);
    expect(save).toHaveBeenCalledTimes(1);
    const [, , layout] = save.mock.calls[0]!;
    expect(layout).toEqual({ version: 1, nodes: { hero: { x: 50, y: 0 } } });
    expect(store.getState().save.status).toBe("saved");
  });

  it("undo back to what is on disk is saved again, without a write", async () => {
    const store = projectStore(opened());
    store.getState().edit((d) => addBlock(d, hero, { x: 0, y: 0 }).doc);
    store.getState().undo();
    expect(store.getState().save.status).toBe("saved");
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS * 2);
    expect(save).not.toHaveBeenCalled();
    store.getState().redo();
    expect(store.getState().history.present.flow.nodes).toHaveLength(1);
  });

  it("an edit during a write is written by a second write", async () => {
    let finish: () => void = () => {};
    save.mockImplementationOnce(
      () => new Promise((resolve) => (finish = () => resolve({ ok: true, value: null }))),
    );
    const store = projectStore(opened());
    store.getState().edit((d) => addBlock(d, hero, { x: 0, y: 0 }).doc);
    const first = store.getState().saveNow();
    await vi.advanceTimersByTimeAsync(0);
    store.getState().edit((d) => moveNodes(d, { hero: { x: 9, y: 9 } }));
    const second = store.getState().saveNow();
    finish();
    expect(await first).toBe(true);
    expect(await second).toBe(true);
    expect(save).toHaveBeenCalledTimes(2);
    expect(save.mock.calls[1]![2]).toMatchObject({ nodes: { hero: { x: 9, y: 9 } } });
  });

  it("a failed save keeps the tab open and shows the error", async () => {
    save.mockResolvedValue({ ok: false, error: { code: "SR-1002", message: "", params: {} } });
    const project = opened();
    const store = projectStore(project);
    store.getState().edit((d) => addBlock(d, hero, { x: 0, y: 0 }).doc);
    expect(await closeProjectStore(project.dir)).toBe(false);
    expect(store.getState().save).toMatchObject({ status: "failed", error: { code: "SR-1002" } });
    expect(projectStore(project)).toBe(store); // still the same tab state
    expect(await saveAllProjects()).toBe(false);

    save.mockResolvedValue({ ok: true, value: null });
    expect(await closeProjectStore(project.dir)).toBe(true);
    expect(projectStore(project)).not.toBe(store); // dropped: a reopen starts fresh
  });

  it("the viewport is written with the next save, not as an undo step", async () => {
    const store = projectStore(opened());
    store.getState().setViewport({ x: 1, y: 2, zoom: 0.5 });
    expect(store.getState().history.past).toHaveLength(0);
    expect(await store.getState().saveNow()).toBe(true);
    expect(save.mock.calls[0]![2]).toMatchObject({ viewport: { x: 1, y: 2, zoom: 0.5 } });
  });
});
