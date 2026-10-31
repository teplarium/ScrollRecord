/**
 * Edits of a project document — every change the canvas and the site map can
 * make to the scenario and its layout, as pure functions (old doc → new doc).
 *
 * Why pure: each edit becomes one undo step in the project store, and the
 * rules below are unit-tested without React or a browser. Edits never throw
 * and never check the whole scenario: Rust does that (`validate_flow`, step
 * 3.4b) and the canvas shows the problems; here we only keep the shape of the
 * document sane (unique ids, no arrows to deleted nodes).
 *
 * Rules (ADR 0006, step 3.4):
 * - one video is one path (HANDOFF Q-05, no branches yet), so a new arrow out
 *   of a node replaces its old outgoing arrow, and likewise for incoming;
 * - Start's scroll position is anchored to a block: the first arrow from Start
 *   to a block anchors it there if it isn't anchored to a block yet;
 * - unchanged input returns the same object, so the history skips no-ops.
 */
import type { Edge } from "../ipc/generated/Edge";
import type { Flow } from "../ipc/generated/Flow";
import type { FlowNode } from "../ipc/generated/FlowNode";
import type { Locator } from "../ipc/generated/Locator";
import type { CanvasLayout, Point } from "./layout";

/** What a project tab edits: the scenario and the canvas positions. */
export type ProjectDoc = { flow: Flow; layout: CanvasLayout };

/** A block picked in the site map. */
export type BlockSource = { page: string; name: string; locators: Locator[] };

/**
 * What the site map hands to the canvas (by click, or dragged as JSON under
 * `NODE_SOURCE_MIME`): Start, End or one of the site's blocks.
 */
export type NodeSource = { kind: "start" } | { kind: "end" } | ({ kind: "block" } & BlockSource);

/** Drag-and-drop data type of a `NodeSource` (site map → canvas). */
export const NODE_SOURCE_MIME = "application/x-scrollrecord-node";

/** Reads a dropped `NodeSource`; anything else (files, text) gives `null`. */
export function readNodeSource(json: string): NodeSource | null {
  try {
    const value: unknown = JSON.parse(json);
    if (typeof value !== "object" || value === null || !("kind" in value)) return null;
    const v = value as Record<string, unknown>;
    if (v.kind === "start" || v.kind === "end") return { kind: v.kind };
    if (
      v.kind === "block" &&
      typeof v.page === "string" &&
      typeof v.name === "string" &&
      Array.isArray(v.locators)
    ) {
      return { kind: "block", page: v.page, name: v.name, locators: v.locators as Locator[] };
    }
  } catch {
    // Not ours: ignore.
  }
  return null;
}

/** Adds what the site map gave at `at` (top-left). Returns the node id. */
export function addFromSource(doc: ProjectDoc, source: NodeSource, at: Point) {
  return source.kind === "block" ? addBlock(doc, source, at) : addTerminal(doc, source.kind, at);
}

/** `base`, or `base-2`, `base-3`… — the first id no node uses. */
export function freeId(flow: Flow, base: string): string {
  const taken = new Set(flow.nodes.map((n) => n.id));
  const clean = base.trim() || "block";
  if (!taken.has(clean)) return clean;
  for (let i = 2; ; i++) {
    const id = `${clean}-${i}`;
    if (!taken.has(id)) return id;
  }
}

function withNode(doc: ProjectDoc, node: FlowNode, at: Point): ProjectDoc {
  return {
    flow: { ...doc.flow, nodes: [...doc.flow.nodes, node] },
    layout: { ...doc.layout, nodes: { ...doc.layout.nodes, [node.id]: at } },
  };
}

/** Adds a block node at `at` (top-left, canvas px). Returns the new id. */
export function addBlock(doc: ProjectDoc, source: BlockSource, at: Point) {
  const id = freeId(doc.flow, source.name);
  const node: FlowNode = {
    kind: "block",
    id,
    page: source.page,
    target: source.locators,
    align: "top",
    offset_px: 0,
  };
  return { doc: withNode(doc, node, at), id };
}

/**
 * Adds the Start or End node. There is only one of each: if it exists,
 * nothing changes and its id is returned.
 */
export function addTerminal(doc: ProjectDoc, kind: "start" | "end", at: Point) {
  const existing = doc.flow.nodes.find((n) => n.kind === kind);
  if (existing) return { doc, id: existing.id };
  const id = freeId(doc.flow, kind);
  const node: FlowNode =
    kind === "start"
      ? // Not anchored yet: the first arrow to a block anchors it (see `connect`).
        { kind, id, at: { block: "", anchor: "top", offset_px: 0 }, warmup_ms: 0 }
      : { kind, id, hold_ms: 1000 };
  return { doc: withNode(doc, node, at), id };
}

/** Moves nodes to new top-left positions (one drag = one call). */
export function moveNodes(doc: ProjectDoc, positions: Record<string, Point>): ProjectDoc {
  const changed = Object.entries(positions).filter(([id, p]) => {
    const old = doc.layout.nodes[id];
    return !old || old.x !== p.x || old.y !== p.y;
  });
  if (changed.length === 0) return doc;
  return {
    ...doc,
    layout: { ...doc.layout, nodes: { ...doc.layout.nodes, ...Object.fromEntries(changed) } },
  };
}

/**
 * Draws an arrow `from` → `to`. Ignored: unknown nodes, an arrow to itself,
 * an arrow into Start or out of End, an arrow that already exists.
 */
export function connect(doc: ProjectDoc, from: string, to: string): ProjectDoc {
  const { flow } = doc;
  const source = flow.nodes.find((n) => n.id === from);
  const dest = flow.nodes.find((n) => n.id === to);
  if (!source || !dest || from === to) return doc;
  if (dest.kind === "start" || source.kind === "end") return doc;
  if (flow.edges.some((e) => e.from === from && e.to === to)) return doc;

  // ── One path: the new arrow replaces the old ones at both ends ──
  const edges: Edge[] = [
    ...flow.edges.filter((e) => e.from !== from && e.to !== to),
    { from, to, scroll: {} },
  ];

  // ── Anchor an unanchored Start to the first block it points at ──
  const anchored = (block: string) => flow.nodes.some((n) => n.kind === "block" && n.id === block);
  const nodes =
    source.kind === "start" && dest.kind === "block" && !anchored(source.at.block)
      ? flow.nodes.map((n) => (n === source ? { ...source, at: { ...source.at, block: to } } : n))
      : flow.nodes;

  return { ...doc, flow: { ...flow, nodes, edges } };
}

/**
 * Deletes nodes (with every arrow touching them and their positions) and
 * arrows given by their index in `flow.edges`.
 */
export function remove(doc: ProjectDoc, nodeIds: string[], edgeIndexes: number[]): ProjectDoc {
  const gone = new Set(nodeIds.filter((id) => doc.flow.nodes.some((n) => n.id === id)));
  const cut = new Set(edgeIndexes);
  const edges = doc.flow.edges.filter(
    (e, i) => !cut.has(i) && !gone.has(e.from) && !gone.has(e.to),
  );
  if (gone.size === 0 && edges.length === doc.flow.edges.length) return doc;

  const positions = { ...doc.layout.nodes };
  for (const id of gone) delete positions[id];
  const nodes = doc.flow.nodes.filter((n) => !gone.has(n.id)).map((n) => unanchor(n, gone));
  return {
    flow: { ...doc.flow, nodes, edges },
    layout: { ...doc.layout, nodes: positions },
  };
}

/**
 * Start/End tied to a deleted block would point at nothing (a red shape the
 * user can't fix from the canvas). Start becomes unanchored again (its next
 * arrow anchors it, see `connect`); End stays where the last block leaves off.
 */
function unanchor(node: FlowNode, gone: Set<string>): FlowNode {
  if (node.kind === "start" && gone.has(node.at.block)) {
    return { ...node, at: { ...node.at, block: "" } };
  }
  if (node.kind === "end" && node.at && gone.has(node.at.block)) {
    return { ...node, at: null };
  }
  return node;
}

/** Points the scenario at another site folder or address (`meta.site`). */
export function setSite(doc: ProjectDoc, site: string | null): ProjectDoc {
  if ((doc.flow.meta.site ?? null) === site) return doc;
  return { ...doc, flow: { ...doc.flow, meta: { ...doc.flow.meta, site } } };
}
