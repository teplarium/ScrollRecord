/**
 * Auto-layout for scenarios without saved positions (FR-CNV-12): an imported
 * file, a scenario written by an AI skill, or nodes added outside the editor.
 *
 * A scenario is a path (Start → blocks → End), so a simple own layout is
 * enough (ADR 0006 §3, no graph-layout dependency):
 * - the main path is one column, top to bottom, in arrow order;
 * - a second arrow out of a node starts a branch in a new column to the right,
 *   just below that node;
 * - nodes that no arrow reaches (not connected yet) start their own column.
 * Arrows going back (loops) are ignored for placement, so it always ends.
 * Linear in the number of nodes and arrows: 250 nodes stay instant (NFR-05).
 */
import type { Edge } from "../../shared/ipc/generated/Edge";
import type { FlowNode } from "../../shared/ipc/generated/FlowNode";
import type { Size } from "./cardSize";
import type { Point } from "./layout";

/** Vertical space between shapes (room for the arrow). */
export const ROW_GAP = 80;
/** Horizontal space between columns. */
export const COLUMN_GAP = 160;

/** Top-left positions by node id, for every node in `nodes`. */
export function autoLayout(
  nodes: FlowNode[],
  edges: Edge[],
  sizeOf: (node: FlowNode) => Size,
): Record<string, Point> {
  const byId = new Map(nodes.map((n) => [n.id, n]));

  // ── Arrows out of each node, in file order; count arrows in ──
  const outgoing = new Map<string, string[]>();
  const incoming = new Map<string, number>();
  for (const edge of edges) {
    if (!byId.has(edge.from) || !byId.has(edge.to)) continue;
    outgoing.set(edge.from, [...(outgoing.get(edge.from) ?? []), edge.to]);
    incoming.set(edge.to, (incoming.get(edge.to) ?? 0) + 1);
  }

  // ── Where columns start: Start first, then nodes nothing points to ──
  // Leftovers (only reachable through a loop) are picked up at the end.
  const roots = [
    ...nodes.filter((n) => n.kind === "start"),
    ...nodes.filter((n) => n.kind !== "start" && !incoming.get(n.id)),
    ...nodes,
  ];

  const placed = new Map<string, { column: number; y: number }>();
  let columns = 0;

  // A column follows the first unplaced arrow of each node; other arrows are
  // queued as branches, each starting a new column below its parent.
  function walk(first: string, column: number, top: number, branches: [string, number][]) {
    let current: string | undefined = first;
    let y = top;
    while (current !== undefined && !placed.has(current)) {
      const node = byId.get(current);
      if (!node) break;
      placed.set(current, { column, y });
      y += sizeOf(node).height + ROW_GAP;
      const next: string[] = (outgoing.get(current) ?? []).filter((id) => !placed.has(id));
      for (const branch of next.slice(1)) branches.push([branch, y]);
      current = next[0];
    }
  }

  for (const root of roots) {
    if (placed.has(root.id)) continue;
    const branches: [string, number][] = [];
    walk(root.id, columns++, 0, branches);
    // Branches of branches are appended while looping, so all get placed.
    for (let i = 0; i < branches.length; i++) {
      const [id, top] = branches[i]!;
      if (!placed.has(id)) walk(id, columns++, top, branches);
    }
  }

  // ── Columns as wide as the widest shape; shapes centred in them ──
  const columnWidth = Math.max(0, ...nodes.map((n) => sizeOf(n).width)) + COLUMN_GAP;
  const positions: Record<string, Point> = {};
  for (const node of nodes) {
    const spot = placed.get(node.id);
    if (!spot) continue;
    const { width } = sizeOf(node);
    positions[node.id] = {
      x: spot.column * columnWidth + Math.round((columnWidth - width) / 2),
      y: spot.y,
    };
  }
  return positions;
}
