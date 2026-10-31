/**
 * Scenario → React Flow shapes and arrows.
 *
 * Why a pure function: what the canvas shows is decided here and unit-tested
 * without a browser; `Canvas.tsx` only renders the result. Positions come
 * from `layout.json` where saved, otherwise from `autoLayout` (FR-CNV-12).
 * Block cards take the current device's shape (D-39), so switching the device
 * rebuilds the shapes; the arrows stay the same. Shapes that would overlap on
 * this device are shown pushed apart (`separate.ts`, never saved).
 * Each arrow leaves and enters on the sides that face each other, so a column
 * reads top to bottom and a branch to the right reads left to right.
 * Problems of the scenario (`ShapeProblem`, from Rust's checks and the site
 * map) mark their shapes and arrows (FR-CNV-09).
 */
import { MarkerType, Position, type Edge, type Node } from "@xyflow/react";

import type { Flow } from "../../shared/ipc/generated/Flow";
import type { FlowNode } from "../../shared/ipc/generated/FlowNode";
import { autoLayout } from "./autoLayout";
import { shapeSize, type Size } from "./cardSize";
import type { CanvasLayout } from "../../shared/store/layout";
import { separate } from "./separate";

/** A problem to show on the canvas, already in the user's language. */
export type ShapeProblem = {
  severity: "error" | "warning";
  text: string;
  /** The shape it is about, or the arrow (index in `flow.edges`), or neither. */
  node: string | null;
  edge: number | null;
};

/** What a shape shows of its problems: the worst severity and every text. */
export type ProblemMark = { severity: "error" | "warning"; text: string } | null;

type Marked = { problem: ProblemMark };
export type TerminalData = Marked & { kind: "start" | "end"; holdMs: number | null };
export type BlockData = Marked & { name: string; holdMs: number; actions: number };
export type NavigateData = Marked & { toPage: string | null };

export type CanvasNode =
  Node<TerminalData, "terminal"> | Node<BlockData, "block"> | Node<NavigateData, "navigate">;

export type CanvasGraph = { nodes: CanvasNode[]; edges: Edge[] };

function nodeData(
  node: FlowNode,
  flow: Flow,
  problem: ProblemMark,
): Pick<CanvasNode, "type" | "data"> {
  switch (node.kind) {
    case "start":
      return { type: "terminal", data: { problem, kind: "start", holdMs: null } };
    case "end":
      return { type: "terminal", data: { problem, kind: "end", holdMs: node.hold_ms } };
    case "block":
      return {
        type: "block",
        data: {
          problem,
          name: node.id,
          // `null` = the scenario default (Rust applies the same rule).
          holdMs: node.hold_ms ?? flow.defaults.hold_ms,
          actions: node.actions?.length ?? 0,
        },
      };
    case "navigate":
      return { type: "navigate", data: { problem, toPage: node.to_page ?? null } };
  }
}

/** The marks of the shapes (by id) or arrows (by index) that have problems. */
export function problemMarks<K>(problems: ShapeProblem[], keyOf: (p: ShapeProblem) => K | null) {
  const marks = new Map<K, NonNullable<ProblemMark>>();
  for (const p of problems) {
    const key = keyOf(p);
    if (key === null) continue;
    const old = marks.get(key);
    marks.set(key, {
      severity: old?.severity === "error" || p.severity === "error" ? "error" : "warning",
      text: old ? `${old.text}\n${p.text}` : p.text,
    });
  }
  return marks;
}

/** Builds the canvas for one device; `card` is that device's block card size. */
export function buildCanvas(
  flow: Flow,
  layout: CanvasLayout,
  card: Size,
  problems: ShapeProblem[] = [],
): CanvasGraph {
  const sizeOf = (node: FlowNode) => shapeSize(node, card);
  const nodeMarks = problemMarks(problems, (p) => p.node);
  const edgeMarks = problemMarks(problems, (p) => p.edge);
  const auto = autoLayout(flow.nodes, flow.edges, sizeOf);

  const placed = flow.nodes.map((node): CanvasNode => {
    const { width, height } = sizeOf(node);
    return {
      id: node.id,
      position: layout.nodes[node.id] ?? auto[node.id] ?? { x: 0, y: 0 },
      width,
      height,
      ...nodeData(node, flow, nodeMarks.get(node.id) ?? null),
    } as CanvasNode;
  });
  const pushed = separate(
    placed.map((n) => ({ ...n, width: n.width ?? 0, height: n.height ?? 0 })),
  );
  const nodes = placed.map((n) => {
    const position = pushed.get(n.id);
    return position ? { ...n, position } : n;
  });

  // Arrows to a missing node are skipped here; validation reports them.
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const edges: Edge[] = [];
  flow.edges.forEach((e, i) => {
    const from = byId.get(e.from);
    const to = byId.get(e.to);
    if (!from || !to) return;
    const [out, into] = facingSides(from, to);
    edges.push({
      // The index keeps ids unique and tells edits which arrow it is.
      id: edgeId(i, e.from, e.to),
      source: e.from,
      target: e.to,
      sourceHandle: out,
      targetHandle: `${into}-in`,
      markerEnd: { type: MarkerType.ArrowClosed, width: 18, height: 18 },
      ...(edgeMarks.has(i) ? { className: `canvas-edge--${edgeMarks.get(i)?.severity}` } : {}),
    });
  });

  return { nodes, edges };
}

/** Arrow id on the canvas: `<index in flow.edges>:<from>-><to>`. */
export function edgeId(index: number, from: string, to: string) {
  return `${index}:${from}->${to}`;
}

/** Index in `flow.edges` of a canvas arrow id (see `edgeId`). */
export function edgeIndex(id: string): number | null {
  const index = Number.parseInt(id, 10);
  return Number.isInteger(index) && index >= 0 ? index : null;
}

/**
 * Which side an arrow leaves `from` and enters `to`: below → bottom/top,
 * above → top/bottom, otherwise right/left or left/right.
 */
export function facingSides(from: CanvasNode, to: CanvasNode): [Position, Position] {
  const a = box(from);
  const b = box(to);
  if (b.top >= a.bottom) return [Position.Bottom, Position.Top];
  if (b.bottom <= a.top) return [Position.Top, Position.Bottom];
  return b.left + b.right >= a.left + a.right
    ? [Position.Right, Position.Left]
    : [Position.Left, Position.Right];
}

function box(n: CanvasNode) {
  const { x, y } = n.position;
  return { left: x, top: y, right: x + (n.width ?? 0), bottom: y + (n.height ?? 0) };
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
