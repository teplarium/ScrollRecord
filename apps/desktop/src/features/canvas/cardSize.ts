/**
 * Shape sizes on the canvas.
 *
 * Block cards have the shape of their block on the current device, at most
 * ~800 px on the long side (HANDOFF D-39). Until the site is measured (steps
 * 3.3/3.6) a block's height is unknown, so a card is drawn as **one screen of
 * the device**: desktop 1920×1080 → 800×450, iPhone 393×852 → 369×800.
 * Start/End and Navigate have fixed sizes, big enough to read when the canvas
 * is zoomed out to show a whole scenario.
 */
import type { DevicePreset } from "../../shared/ipc/generated/DevicePreset";
import type { FlowNode } from "../../shared/ipc/generated/FlowNode";
import type { ViewportRef } from "../../shared/ipc/generated/ViewportRef";

export type Size = { width: number; height: number };

/** Longest side of a block card in canvas pixels (D-39). */
export const CARD_LONG_SIDE = 800;

/** Used when the device is unknown (custom preset name, presets not loaded). */
const FALLBACK_SCREEN: Size = { width: 1920, height: 1080 };

export const TERMINAL_SIZE: Size = { width: 360, height: 120 };
export const NAVIGATE_SIZE: Size = { width: 420, height: 140 };

/** The device screen of a target's viewport (`"iphone-15"` or a custom object). */
export function screenOf(viewport: ViewportRef | undefined, presets: DevicePreset[]): Size | null {
  if (viewport === undefined) return null;
  if (typeof viewport !== "string") return { width: viewport.width, height: viewport.height };
  const preset = presets.find((p) => p.name === viewport);
  return preset ? { width: preset.viewport.width, height: preset.viewport.height } : null;
}

/** A block card for a device screen, scaled so its long side is `CARD_LONG_SIDE`. */
export function cardSize(screen: Size | null): Size {
  const { width, height } =
    screen && screen.width > 0 && screen.height > 0 ? screen : FALLBACK_SCREEN;
  const scale = CARD_LONG_SIDE / Math.max(width, height);
  return { width: Math.round(width * scale), height: Math.round(height * scale) };
}

/** Size of any shape on the canvas. */
export function shapeSize(node: FlowNode, card: Size): Size {
  switch (node.kind) {
    case "block":
      return card;
    case "navigate":
      return NAVIGATE_SIZE;
    case "start":
    case "end":
      return TERMINAL_SIZE;
  }
}
