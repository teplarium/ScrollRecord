/**
 * Scenario → React Flow shapes and arrows.
 *
 * Why a pure function: what the canvas shows is decided here and unit-tested
 * without a browser; `Canvas.tsx` only renders the result. Positions come
 * from `layout.json` where saved, otherwise from `autoLayout` (FR-CNV-12).
 * Block cards take the current device's shape (D-39), so switching the device
 * rebuilds the shapes; the arrows stay the same.
 */
import { MarkerType, type Edge, type Node } from "@xyflow/react";

import type { Flow } from "../../shared/ipc/generated/Flow";
import type { FlowNode } from "../../shared/ipc/generated/FlowNode";
import { autoLayout } from "./autoLayout";
import { shapeSize, type Size } from "./cardSize";
import type { CanvasLayout } from "./layout";

export type TerminalData = { kind: "start" | "end"; holdMs: number | null };
export type BlockData = { name: string; holdMs: number; actions: number };
export type NavigateData = { toPage: string | null };

export type CanvasNode =
  Node<TerminalData, "terminal"> | Node<BlockData, "block"> | Node<NavigateData, "navigate">;

export type CanvasGraph = { nodes: CanvasNode[]; edges: Edge[] };

function nodeData(node: FlowNode, flow: Flow): Pick<CanvasNode, "type" | "data"> {
  switch (node.kind) {
    case "start":
      return { type: "terminal", data: { kind: "start", holdMs: null } };
    case "end":
      return { type: "terminal", data: { kind: "end", holdMs: node.hold_ms } };
    case "block":
      return {
        type: "block",
        data: {
          name: node.id,
          // `null` = the scenario default (Rust applies the same rule).
          holdMs: node.hold_ms ?? flow.defaults.hold_ms,
          actions: node.actions?.length ?? 0,
        },
      };
    case "navigate":
      return { type: "navigate", data: { toPage: node.to_page ?? null } };
  }
}

/** Builds the canvas for one device; `card` is that device's block card size. */
export function buildCanvas(flow: Flow, layout: CanvasLayout, card: Size): CanvasGraph {
  const sizeOf = (node: FlowNode) => shapeSize(node, card);
  const auto = autoLayout(flow.nodes, flow.edges, sizeOf);

  const nodes = flow.nodes.map((node): CanvasNode => {
    const { width, height } = sizeOf(node);
    return {
      id: node.id,
      position: layout.nodes[node.id] ?? auto[node.id] ?? { x: 0, y: 0 },
      width,
      height,
      // Read-only until step 3.4 (moving and connecting are edits).
      draggable: false,
      connectable: false,
      ...nodeData(node, flow),
    } as CanvasNode;
  });

  // Arrows to a missing node are skipped here; validation reports them.
  const ids = new Set(flow.nodes.map((n) => n.id));
  const edges = flow.edges
    .filter((e) => ids.has(e.from) && ids.has(e.to))
    .map((e, i): Edge => ({
      // Two arrows may join the same nodes, so the index keeps ids unique.
      id: `${i}:${e.from}->${e.to}`,
      source: e.from,
      target: e.to,
      markerEnd: { type: MarkerType.ArrowClosed, width: 18, height: 18 },
      // Arrow properties are edited from step 3.5; until then only shapes select.
      selectable: false,
      focusable: false,
    }));

  return { nodes, edges };
}

/** How many shapes the canvas frames when a scenario is opened. */
export const OPENING_SHAPES = 12;

/**
 * The shapes to frame when the canvas opens: the top of the scenario.
 * Why not everything: 250 cards fitted into one window are dots; the start of
 * the path at a readable size is more useful ("Show the whole scenario" still
 * fits everything). Short scenarios are framed whole.
 */
export function openingShapes(nodes: CanvasNode[], count = OPENING_SHAPES): { id: string }[] {
  return [...nodes]
    .sort((a, b) => a.position.y - b.position.y || a.position.x - b.position.x)
    .slice(0, count)
    .map((n) => ({ id: n.id }));
}
