/**
 * Canvas layout — the editor-only part of a project, stored in `layout.json`
 * (part of the project document in the store; drawn by `features/canvas`).
 *
 * Why separate from the scenario: positions are noise for the renderer and for
 * AI skills reading the scenario (ADR 0006 §1). Rust treats this file as free
 * JSON, so the UI owns its format and must survive anything in it: a
 * hand-edited, older or broken file reads as "no positions" (auto-layout,
 * FR-CNV-12) instead of breaking the editor.
 *
 * Format v1: `{ "version": 1, "nodes": { "<node id>": { "x": 0, "y": 0 } },
 * "viewport"?: { "x": 0, "y": 0, "zoom": 1 } }` — positions are the top-left
 * corner of a shape in canvas pixels.
 */

export type Point = { x: number; y: number };

export type CanvasViewport = { x: number; y: number; zoom: number };

export type CanvasLayout = {
  version: 1;
  /** Saved positions by node id. Nodes missing here are auto-laid out. */
  nodes: Record<string, Point>;
  viewport?: CanvasViewport;
};

export const emptyLayout: CanvasLayout = { version: 1, nodes: {} };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function readPoint(value: unknown): Point | null {
  if (!isRecord(value) || !isFiniteNumber(value.x) || !isFiniteNumber(value.y)) return null;
  return { x: value.x, y: value.y };
}

/**
 * Reads `layout.json` content. Never throws: unknown versions and broken
 * entries are dropped (one bad position doesn't lose the others).
 */
export function readLayout(value: unknown): CanvasLayout {
  if (!isRecord(value) || value.version !== 1) return emptyLayout;

  const nodes: Record<string, Point> = {};
  if (isRecord(value.nodes)) {
    for (const [id, raw] of Object.entries(value.nodes)) {
      const point = readPoint(raw);
      if (point) nodes[id] = point;
    }
  }

  const layout: CanvasLayout = { version: 1, nodes };
  const viewport = value.viewport;
  if (isRecord(viewport) && isFiniteNumber(viewport.zoom) && viewport.zoom > 0) {
    const point = readPoint(viewport);
    if (point) layout.viewport = { ...point, zoom: viewport.zoom };
  }
  return layout;
}
