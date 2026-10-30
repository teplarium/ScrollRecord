/**
 * useSiteMap — runs the site map job for the shown device and keeps its state.
 *
 * When: on opening a tab, on switching the device, on "Rescan" (`fresh`:
 * ignore the cache), and ~0.8 s after an edit that changes what the site map
 * depends on (site, pages, `prepare`, targets, the scenario's blocks) — not
 * after moves or arrows, so editing the canvas doesn't re-read the site.
 * Rust answers from its cache at once when nothing it reads changed.
 * A run that is no longer wanted (other device, tab closed) is cancelled, and
 * its late events are ignored.
 */
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";

import { commands } from "../../shared/ipc/commands";
import type { Flow } from "../../shared/ipc/generated/Flow";
import { initialSiteMap, siteMapReducer } from "./siteMapState";

/** Wait after an edit before reading the site again. */
const RESCAN_DELAY_MS = 800;

/** The parts of a scenario the site map depends on, as a comparable key. */
export function siteMapKey(flow: Flow): string {
  return JSON.stringify({
    site: flow.meta.site ?? null,
    pages: flow.pages,
    prepare: flow.prepare,
    targets: flow.targets,
    blocks: flow.nodes.flatMap((n) =>
      n.kind === "block" ? [{ id: n.id, page: n.page, target: n.target }] : [],
    ),
  });
}

export function useSiteMap(dir: string, flow: Flow, target: string | null) {
  const [state, dispatch] = useReducer(siteMapReducer, initialSiteMap);
  // The scenario last sent to Rust; replaced only when its key changes.
  const key = useMemo(() => siteMapKey(flow), [flow]);
  const [sent, setSent] = useState({ key, flow });
  useEffect(() => {
    if (key === sent.key) return;
    const timer = setTimeout(() => setSent({ key, flow }), RESCAN_DELAY_MS);
    return () => clearTimeout(timer);
  }, [key, flow, sent.key]);
  // Bumped by "Rescan"; `fresh` makes that run skip the cache.
  const [run, setRun] = useState({ id: 0, fresh: false });
  const job = useRef<number | null>(null);

  useEffect(() => {
    if (target === null) return;
    let active = true;
    dispatch({ type: "started" });
    void commands
      .startSiteMap(dir, sent.flow, target, run.fresh, (event) => {
        if (!active) return;
        if (event.kind === "done" || event.kind === "failed") job.current = null;
        dispatch({ type: "event", event });
      })
      .then((started) => {
        if (!started.ok) {
          if (active) dispatch({ type: "failedToStart", error: started.error });
          return;
        }
        if (active) job.current = started.value;
        else void commands.cancelJob(started.value);
      });
    return () => {
      active = false;
      if (job.current !== null) void commands.cancelJob(job.current);
      job.current = null;
    };
  }, [dir, sent, target, run]);

  const cancel = useCallback(() => {
    if (job.current !== null) void commands.cancelJob(job.current);
  }, []);
  const rescan = useCallback(() => setRun((r) => ({ id: r.id + 1, fresh: true })), []);
  const clearCache = useCallback(async () => {
    const cleared = await commands.clearCache(dir);
    if (!cleared.ok) dispatch({ type: "failedToStart", error: cleared.error });
  }, [dir]);
  const dismissError = useCallback(() => dispatch({ type: "dismissError" }), []);

  return { state, cancel, rescan, clearCache, dismissError };
}
