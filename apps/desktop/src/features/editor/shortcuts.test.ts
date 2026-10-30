import { describe, expect, it } from "vitest";

import { editorShortcut } from "./shortcuts";

const press = (code: string, mods: Partial<KeyboardEvent> = {}) =>
  editorShortcut({
    code,
    ctrlKey: true,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    target: null,
    ...mods,
  });

describe("editor shortcuts", () => {
  it("follow the key position, so a Russian layout works too", () => {
    // With the Russian layout Ctrl+Z reports key "я" but code "KeyZ".
    expect(press("KeyZ")).toBe("undo");
    expect(press("KeyZ", { shiftKey: true })).toBe("redo");
    expect(press("KeyY")).toBe("redo");
    expect(press("KeyS")).toBe("save");
    expect(press("KeyZ", { ctrlKey: false, metaKey: true })).toBe("undo");
  });

  it("ignore other keys and plain letters", () => {
    expect(press("KeyA")).toBeNull();
    expect(press("KeyZ", { ctrlKey: false })).toBeNull();
    expect(press("KeyZ", { altKey: true })).toBeNull();
  });
});
