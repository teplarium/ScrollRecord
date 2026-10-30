import { describe, expect, it } from "vitest";

import type { AppError } from "../../shared/ipc/generated/AppError";
import type { Flow } from "../../shared/ipc/generated/Flow";
import type { FlowNode } from "../../shared/ipc/generated/FlowNode";
import type { SiteMap } from "../../shared/ipc/generated/SiteMap";
import { initialSiteMap, siteMapReducer, type SiteMapState } from "./siteMapState";
import { siteMapKey } from "./useSiteMap";

const map = (target: string) => ({ target, pages: [] }) as unknown as SiteMap;
const error = (code: string) => ({ code, message: "", params: {} }) as AppError;

function run(...steps: Parameters<typeof siteMapReducer>[1][]): SiteMapState {
  return steps.reduce(siteMapReducer, initialSiteMap);
}

describe("site map state", () => {
  it("shows progress, then the new map", () => {
    const s = run(
      { type: "started" },
      {
        type: "event",
        event: {
          kind: "progress",
          progress: { done: 1, total: 3, stage: "openingPage", page: "home" },
        },
      },
    );
    expect(s.running).toBe(true);
    expect(s.progress?.done).toBe(1);
    const done = siteMapReducer(s, {
      type: "event",
      event: { kind: "done", map: map("desktop"), fromCache: false },
    });
    expect(done).toMatchObject({ running: false, progress: null, stale: false });
    expect(done.map?.target).toBe("desktop");
  });

  it("keeps the previous map, marked stale, while a new run goes", () => {
    const s = run(
      { type: "event", event: { kind: "done", map: map("desktop"), fromCache: true } },
      { type: "started" },
    );
    expect(s.map?.target).toBe("desktop");
    expect(s.stale).toBe(true);
    const shown = siteMapReducer(s, {
      type: "event",
      event: { kind: "stale", map: map("mobile") },
    });
    expect(shown).toMatchObject({ stale: true, running: true });
  });

  it("shows failures but not a cancel", () => {
    const failed = run(
      { type: "started" },
      { type: "event", event: { kind: "failed", error: error("SR-3001") } },
    );
    expect(failed.error?.code).toBe("SR-3001");
    expect(failed.running).toBe(false);
    const cancelled = run(
      { type: "started" },
      { type: "event", event: { kind: "failed", error: error("SR-0004") } },
    );
    expect(cancelled.error).toBeNull();
    expect(siteMapReducer(failed, { type: "dismissError" }).error).toBeNull();
  });
});

describe("site map key", () => {
  it("changes with blocks and pages, not with arrows or other shapes", () => {
    const base = { meta: { name: "p" }, pages: [], prepare: {}, targets: [] };
    const block = (id: string) =>
      ({ kind: "block", id, page: "home", target: [{ sr: id }] }) as FlowNode;
    const start = { kind: "start", id: "start" } as FlowNode;
    const end = { kind: "end", id: "end" } as FlowNode;
    const chain = (...ids: string[]) => ids.slice(1).map((to, i) => ({ from: ids[i], to }));
    const make = (nodes: FlowNode[], edges: unknown[]) =>
      ({ ...base, nodes, edges }) as unknown as Flow;
    const key = siteMapKey(make([block("a")], []));
    expect(siteMapKey(make([block("a"), start, end], chain("start", "a", "end")))).toBe(key);
    expect(siteMapKey(make([block("a"), block("b")], []))).not.toBe(key);
  });
});
