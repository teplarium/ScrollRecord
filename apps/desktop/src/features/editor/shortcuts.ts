/**
 * Editor keyboard shortcuts: Ctrl+Z undo, Ctrl+Shift+Z / Ctrl+Y redo,
 * Ctrl+S save now (Cmd on macOS).
 *
 * Why physical keys (`event.code`) and not characters (`event.key`): with a
 * Russian (or any non-Latin) layout the Z key types "я", so `event.key` never
 * equals "z" and undo silently did nothing (found in the step 3.4 check).
 * Shortcuts belong to key positions, like in Figma.
 */

export type EditorShortcut = "undo" | "redo" | "save";

type KeyInfo = Pick<
  KeyboardEvent,
  "code" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey" | "target"
>;

/** The shortcut a key press means, or `null` (other keys, typing in a field). */
export function editorShortcut(event: KeyInfo): EditorShortcut | null {
  if (!(event.ctrlKey || event.metaKey) || event.altKey || isTyping(event.target)) return null;
  switch (event.code) {
    case "KeyZ":
      return event.shiftKey ? "redo" : "undo";
    case "KeyY":
      return "redo";
    case "KeyS":
      return "save";
    default:
      return null;
  }
}

/** Keys typed into a field belong to the field (its own undo, Ctrl+S…). */
function isTyping(target: EventTarget | null) {
  if (typeof HTMLElement === "undefined" || !(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}
