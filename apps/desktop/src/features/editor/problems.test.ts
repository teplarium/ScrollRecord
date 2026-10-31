import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";

import type { Flow } from "../../shared/ipc/generated/Flow";
import type { Issue } from "../../shared/ipc/generated/Issue";
import type { SiteMap } from "../../shared/ipc/generated/SiteMap";
import { toProblems, UNTIL_RENDER } from "./problems";

// The key and values instead of a text, so the test sees what was asked for.
const t = ((key: string, options?: Record<string, unknown>) =>
  `${key}${options?.page ? `:${String(options.page)}` : ""}`) as unknown as TFunction;

const flow = { nodes: [{ kind: "block", id: "hero" }] } as unknown as Flow;

function issue(part: Partial<Issue>): Issue {
  return {
    severity: "error",
    kind: "badSpeed",
    message: "",
    params: {},
    node: null,
    edge: null,
    ...part,
  };
}

describe("problems on the canvas", () => {
  it("translates Rust issues with their values and keeps their shape or arrow", () => {
    const problems = toProblems(
      [
        issue({ severity: "warning", kind: "beforeIgnored", node: "hero" }),
        issue({ kind: "unknownPage", node: "hero", params: { page: "faq" } }),
        issue({ kind: "badSpeed", edge: 2 }),
      ],
      null,
      flow,
      t,
    );
    expect(problems).toEqual([
      { severity: "error", text: "issues.unknownPage:faq", node: "hero", edge: null },
      { severity: "error", text: "issues.badSpeed", node: null, edge: 2 },
      { severity: "warning", text: "issues.beforeIgnored", node: "hero", edge: null },
    ]);
  });

  it("leaves an unfinished path (no End yet, a loose block, a red Start) for the render", () => {
    const unfinished = [
      issue({ kind: "endCount" }),
      issue({ kind: "anchorMissing", node: "start" }),
      issue({ kind: "pathNotToEnd", node: "hero" }),
      issue({ severity: "warning", kind: "notOnPath", node: "header" }),
    ];
    expect(unfinished.every((i) => UNTIL_RENDER.has(i.kind))).toBe(true);
    expect(toProblems(unfinished, null, flow, t)).toEqual([]);
    // A broken anchor (its block was deleted) is a real problem of the shape.
    expect(toProblems([issue({ kind: "anchorNotBlock", node: "end" })], null, flow, t)).toEqual([
      { severity: "error", text: "issues.anchorNotBlock", node: "end", edge: null },
    ]);
  });

  it("marks blocks the site doesn't have on this device, if they are on the canvas", () => {
    const map = {
      pages: [
        {
          blocks: [
            { status: "not_found", node: "hero" },
            { status: "not_found", node: "deleted-meanwhile" },
            { status: "found", node: null },
          ],
        },
      ],
    } as unknown as SiteMap;
    expect(toProblems([], map, flow, t)).toEqual([
      { severity: "error", text: "problems.notFound", node: "hero", edge: null },
    ]);
  });
});
