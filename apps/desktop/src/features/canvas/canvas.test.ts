import { describe, expect, it } from "vitest";

import type { Edge } from "../../shared/ipc/generated/Edge";
import type { Flow } from "../../shared/ipc/generated/Flow";
import type { FlowNode } from "../../shared/ipc/generated/FlowNode";
import { autoLayout, ROW_GAP } from "./autoLayout";
import { cardSize, screenOf, shapeSize, TERMINAL_SIZE } from "./cardSize";
import { emptyLayout, readLayout } from "./layout";
import { buildCanvas, openingShapes } from "./toReactFlow";

// ── Small scenario builders (only the fields the canvas reads) ──

function block(id: string, hold_ms?: number): FlowNode {
  return {
    kind: "block",
    id,
    page: "home",
    target: [],
    align: "top",
    offset_px: 0,
    ...(hold_ms === undefined ? {} : { hold_ms }),
  } as FlowNode;
}
const start: FlowNode = { kind: "start", id: "start" } as FlowNode;
const end: FlowNode = { kind: "end", id: "end", hold_ms: 800 } as FlowNode;

function edge(from: string, to: string): Edge {
  return { from, to } as Edge;
}

function chain(...ids: string[]): Edge[] {
  return ids.slice(1).map((to, i) => edge(ids[i]!, to));
}

function flow(nodes: FlowNode[], edges: Edge[]): Flow {
  return { nodes, edges, defaults: { hold_ms: 1200 } } as Flow;
}

const card = cardSize({ width: 1920, height: 1080 });
const sizeOf = (n: FlowNode) => shapeSize(n, card);

describe("readLayout", () => {
  it("keeps valid positions and the viewport", () => {
    const layout = readLayout({
      version: 1,
      nodes: { hero: { x: 10, y: 20 } },
      viewport: { x: 1, y: 2, zoom: 0.5 },
    });
    expect(layout.nodes).toEqual({ hero: { x: 10, y: 20 } });
    expect(layout.viewport).toEqual({ x: 1, y: 2, zoom: 0.5 });
  });

  it("reads anything broken as no positions instead of failing", () => {
    expect(readLayout(null)).toEqual(emptyLayout);
    expect(readLayout({})).toEqual(emptyLayout);
    expect(readLayout({ version: 2, nodes: { a: { x: 1, y: 1 } } })).toEqual(emptyLayout);
    expect(readLayout([1, 2])).toEqual(emptyLayout);
  });

  it("drops only the broken entries", () => {
    const layout = readLayout({
      version: 1,
      nodes: { ok: { x: 0, y: 5 }, text: { x: "1", y: 0 }, nan: { x: NaN, y: 0 }, none: null },
      viewport: { x: 0, y: 0, zoom: 0 },
    });
    expect(layout.nodes).toEqual({ ok: { x: 0, y: 5 } });
    expect(layout.viewport).toBeUndefined();
  });
});

describe("card sizes (D-39: device shape, long side 800)", () => {
  it("scales the device screen", () => {
    expect(cardSize({ width: 1920, height: 1080 })).toEqual({ width: 800, height: 450 });
    expect(cardSize({ width: 393, height: 852 })).toEqual({ width: 369, height: 800 });
  });

  it("falls back to a desktop screen when the device is unknown", () => {
    expect(cardSize(null)).toEqual({ width: 800, height: 450 });
    expect(cardSize({ width: 0, height: 0 })).toEqual({ width: 800, height: 450 });
  });

  it("finds presets by name and takes custom viewports as they are", () => {
    const presets = [
      { name: "iphone-15", viewport: { width: 393, height: 852, scale: 3, mobile: true } },
    ];
    expect(screenOf("iphone-15", presets)).toEqual({ width: 393, height: 852 });
    expect(screenOf("unknown", presets)).toBeNull();
    expect(screenOf({ width: 500, height: 900, scale: 1, mobile: true }, [])).toEqual({
      width: 500,
      height: 900,
    });
  });
});

describe("autoLayout (FR-CNV-12)", () => {
  it("puts the path in one column, top to bottom, in arrow order", () => {
    // Node order in the file differs from the path order on purpose.
    const nodes = [end, block("b"), start, block("a")];
    const pos = autoLayout(nodes, chain("start", "a", "b", "end"), sizeOf);
    expect(new Set([pos.start!.x + 180, pos.a!.x + 400, pos.end!.x + 180]).size).toBe(1);
    expect(pos.start!.y).toBe(0);
    expect(pos.a!.y).toBe(TERMINAL_SIZE.height + ROW_GAP);
    expect(pos.b!.y).toBe(pos.a!.y + card.height + ROW_GAP);
    expect(pos.end!.y).toBe(pos.b!.y + card.height + ROW_GAP);
  });

  it("moves a branch into the next column, below its parent", () => {
    const nodes = [start, block("a"), block("b"), block("c")];
    const pos = autoLayout(nodes, [...chain("start", "a", "b"), edge("a", "c")], sizeOf);
    expect(pos.c!.x).toBeGreaterThan(pos.b!.x);
    expect(pos.c!.y).toBe(pos.b!.y);
  });

  it("gives unconnected shapes their own column", () => {
    const nodes = [start, block("a"), block("loose")];
    const pos = autoLayout(nodes, chain("start", "a"), sizeOf);
    expect(pos.loose!.x).toBeGreaterThan(pos.a!.x);
    expect(pos.loose!.y).toBe(0);
  });

  it("ends on loops and places every node", () => {
    const nodes = [block("a"), block("b"), block("c")];
    const pos = autoLayout(nodes, [...chain("a", "b", "c"), edge("c", "a")], sizeOf);
    expect(Object.keys(pos).sort()).toEqual(["a", "b", "c"]);
    const ring = autoLayout(nodes, [...chain("a", "b", "c", "a")], sizeOf);
    expect(Object.keys(ring).sort()).toEqual(["a", "b", "c"]);
  });

  it("lays out 250 shapes quickly (NFR-05)", () => {
    const ids = ["start", ...Array.from({ length: 248 }, (_, i) => `b${i}`), "end"];
    const nodes = ids.map((id) => (id === "start" ? start : id === "end" ? end : block(id)));
    const begin = performance.now();
    const pos = autoLayout(nodes, chain(...ids), sizeOf);
    expect(performance.now() - begin).toBeLessThan(100);
    expect(Object.keys(pos)).toHaveLength(250);
  });
});

describe("buildCanvas", () => {
  const scenario = flow(
    [start, block("hero", 1500), block("plain"), end],
    [...chain("start", "hero", "plain", "end"), edge("hero", "missing")],
  );

  it("makes shapes with the scenario's data and the device's card size", () => {
    const { nodes } = buildCanvas(scenario, emptyLayout, card);
    const hero = nodes.find((n) => n.id === "hero")!;
    expect(hero.type).toBe("block");
    expect(hero.data).toEqual({ name: "hero", holdMs: 1500, actions: 0 });
    expect([hero.width, hero.height]).toEqual([800, 450]);
    // No `hold_ms` → the scenario default.
    expect(nodes.find((n) => n.id === "plain")!.data).toMatchObject({ holdMs: 1200 });
    expect(nodes.find((n) => n.id === "end")!.data).toEqual({ kind: "end", holdMs: 800 });
  });

  it("prefers saved positions and auto-lays out the rest", () => {
    const layout = readLayout({ version: 1, nodes: { hero: { x: 5000, y: 7 } } });
    const { nodes } = buildCanvas(scenario, layout, card);
    expect(nodes.find((n) => n.id === "hero")!.position).toEqual({ x: 5000, y: 7 });
    expect(nodes.find((n) => n.id === "start")!.position.y).toBe(0);
  });

  it("keeps the arrows when the device changes and skips broken ones", () => {
    const desktop = buildCanvas(scenario, emptyLayout, card);
    const phone = buildCanvas(scenario, emptyLayout, cardSize({ width: 393, height: 852 }));
    expect(phone.edges).toEqual(desktop.edges);
    expect(desktop.edges.map((e) => e.id)).toEqual([
      "0:start->hero",
      "1:hero->plain",
      "2:plain->end",
    ]);
    expect(phone.nodes.find((n) => n.id === "hero")!.height).toBe(800);
  });
});

describe("openingShapes", () => {
  it("frames the top of a long scenario and all of a short one", () => {
    const ids = ["start", ...Array.from({ length: 30 }, (_, i) => `b${i}`), "end"];
    const long = flow(
      ids.map((id) => (id === "start" ? start : id === "end" ? end : block(id))),
      chain(...ids),
    );
    const framed = openingShapes(buildCanvas(long, emptyLayout, card).nodes);
    expect(framed.map((n) => n.id)).toEqual(ids.slice(0, 12));

    const short = flow([start, block("a"), end], chain("start", "a", "end"));
    expect(openingShapes(buildCanvas(short, emptyLayout, card).nodes)).toHaveLength(3);
  });
});
