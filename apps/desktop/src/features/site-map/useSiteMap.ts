/**
 * useSiteMap — runs the site map job for the shown device and keeps its state.
 *
 * When: on opening a tab, on switching the device, when the scenario changes,
 * and on "Rescan" (`fresh`: ignore the cache). Rust answers from its cache at
 * once when the site and the scenario's blocks did not change.
 * A run that is no longer wanted (other device, tab closed) is cancelled, and
 * its late events are ignored.
 */
import { useCallback, useEffect, useReducer, useRef, useState } from "react";

import { commands } from "../../shared/ipc/commands";
import type { Flow } from "../../shared/ipc/generated/Flow";
import { initialSiteMap, siteMapReducer } from "./siteMapState";

export function useSiteMap(dir: string, flow: Flow, target: string | null) {
  const [state, dispatch] = useReducer(siteMapReducer, initialSiteMap);
  // Bumped by "Rescan"; `fresh` makes that run skip the cache.
  const [run, setRun] = useState({ id: 0, fresh: false });
  const job = useRef<number | null>(null);

  useEffect(() => {
    if (target === null) return;
    let active = true;
    dispatch({ type: "started" });
    void commands
      .startSiteMap(dir, flow, target, run.fresh, (event) => {
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
  }, [dir, flow, target, run]);

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
