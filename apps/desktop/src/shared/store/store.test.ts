import { describe, expect, it } from "vitest";

import type { Flow } from "../ipc/generated/Flow";
import {
  addBlock,
  addFromSource,
  addTerminal,
  connect,
  freeId,
  moveNodes,
  readNodeSource,
  remove,
  setSite,
  type ProjectDoc,
} from "./edits";
import { commit, HISTORY_LIMIT, redo, startHistory, undo } from "./history";
import { emptyLayout } from "./layout";

describe("history", () => {
  it("undoes and redoes steps; a new step clears redo", () => {
    let h = startHistory(0);
    h = commit(h, 1);
    h = commit(h, 2);
    h = undo(h);
    expect(h.present).toBe(1);
    h = redo(h);
    expect(h.present).toBe(2);
    h = undo(commit(undo(h), 5)); // 2 is gone from redo
    expect(h.present).toBe(1);
    expect(h.future).toEqual([5]);
    expect(redo(h).present).toBe(5);
    expect(redo(redo(h)).present).toBe(5); // nothing more to redo
  });

  it("merges steps with the same key (typing in one field)", () => {
    let h = startHistory("");
    h = commit(h, "a", "field:name");
    h = commit(h, "ab", "field:name");
    h = commit(h, "abc", "field:name");
    expect(undo(h).present).toBe("");
    h = commit(h, "abc!", "field:other");
    expect(undo(h).present).toBe("abc");
  });

  it("skips no-ops and keeps at most the limit", () => {
    let h = startHistory(0);
    expect(commit(h, 0)).toBe(h);
    for (let i = 1; i <= HISTORY_LIMIT + 50; i++) h = commit(h, i);
    expect(h.past.length).toBe(HISTORY_LIMIT);
    while (h.past.length > 0) h = undo(h);
    expect(h.present).toBe(50);
  });
});

function doc(nodes: Flow["nodes"] = [], edges: Flow["edges"] = []): ProjectDoc {
  const flow = {
    format_version: 1,
    meta: { name: "t" },
    targets: [],
    defaults: {},
    prepare: {},
    pages: [{ id: "home", url: "/" }],
    nodes,
    edges,
  } as unknown as Flow;
  return { flow, layout: emptyLayout };
}

const block = (id: string): Flow["nodes"][number] => ({
  kind: "block",
  id,
  page: "home",
  target: [{ sr: id }],
  align: "top",
  offset_px: 0,
});

const arrows = (d: ProjectDoc) => d.flow.edges.map((e) => `${e.from}>${e.to}`);

describe("document edits", () => {
  it("adds blocks with unique ids at the given place", () => {
    const one = addBlock(
      doc(),
      { page: "home", name: "hero", locators: [{ sr: "hero" }] },
      {
        x: 10,
        y: 20,
      },
    );
    expect(one.id).toBe("hero");
    expect(one.doc.layout.nodes.hero).toEqual({ x: 10, y: 20 });
    const two = addBlock(one.doc, { page: "home", name: "hero", locators: [] }, { x: 0, y: 0 });
    expect(two.id).toBe("hero-2");
    expect(freeId(two.doc.flow, " ")).toBe("block");
  });

  it("keeps one Start and one End", () => {
    const first = addTerminal(doc(), "start", { x: 0, y: 0 });
    const again = addTerminal(first.doc, "start", { x: 5, y: 5 });
    expect(again.doc).toBe(first.doc);
    expect(again.id).toBe("start");
    expect(addTerminal(first.doc, "end", { x: 0, y: 0 }).doc.flow.nodes).toHaveLength(2);
  });

  it("keeps one path: a new arrow replaces the old one at each end", () => {
    let d = doc([block("a"), block("b"), block("c")]);
    d = connect(d, "a", "b");
    d = connect(d, "a", "c"); // a's old outgoing arrow goes
    expect(arrows(d)).toEqual(["a>c"]);
    d = connect(d, "b", "c"); // c's old incoming arrow goes
    expect(arrows(d)).toEqual(["b>c"]);
    expect(connect(d, "b", "c")).toBe(d);
    expect(connect(d, "b", "b")).toBe(d);
    expect(connect(d, "b", "nope")).toBe(d);
  });

  it("anchors Start to the first block it points at, and never points into Start", () => {
    let d = addTerminal(doc([block("hero"), block("pricing")]), "start", { x: 0, y: 0 }).doc;
    d = connect(d, "start", "hero");
    const start = () => d.flow.nodes.find((n) => n.kind === "start");
    expect(start()).toMatchObject({ at: { block: "hero" } });
    d = connect(d, "start", "pricing"); // already anchored to a block: kept
    expect(start()).toMatchObject({ at: { block: "hero" } });
    expect(connect(d, "hero", "start")).toBe(d);
    d = addTerminal(d, "end", { x: 0, y: 0 }).doc;
    expect(connect(d, "end", "hero")).toBe(d);
  });

  it("reads only its own drag data", () => {
    const block = { kind: "block", page: "home", name: "hero", locators: [{ sr: "hero" }] };
    expect(readNodeSource(JSON.stringify(block))).toEqual(block);
    expect(readNodeSource('{"kind":"end"}')).toEqual({ kind: "end" });
    expect(readNodeSource('{"kind":"block","name":"x"}')).toBeNull();
    expect(readNodeSource("not json")).toBeNull();
    const added = addFromSource(doc(), { kind: "start" }, { x: 0, y: 0 });
    expect(added.id).toBe("start");
  });

  it("removes nodes with their arrows and positions, and arrows by index", () => {
    let d = doc([block("a"), block("b"), block("c")]);
    d = connect(connect(d, "a", "b"), "b", "c");
    d = moveNodes(d, { b: { x: 1, y: 2 } });
    const cut = remove(d, ["b"], []);
    expect(cut.flow.nodes.map((n) => n.id)).toEqual(["a", "c"]);
    expect(cut.flow.edges).toEqual([]);
    expect(cut.layout.nodes.b).toBeUndefined();
    expect(arrows(remove(d, [], [0]))).toEqual(["b>c"]);
    expect(remove(d, ["zzz"], [])).toBe(d);
  });

  it("moves only what changed and sets the site", () => {
    const d = moveNodes(doc([block("a")]), { a: { x: 3, y: 4 } });
    expect(moveNodes(d, { a: { x: 3, y: 4 } })).toBe(d);
    const sited = setSite(d, "../site");
    expect(sited.flow.meta.site).toBe("../site");
    expect(setSite(sited, "../site")).toBe(sited);
  });
});
