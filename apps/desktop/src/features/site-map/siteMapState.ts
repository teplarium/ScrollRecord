/**
 * Site map state of one editor tab, as a pure reducer (tested in
 * `siteMapState.test.ts`).
 *
 * Why a reducer: a site map arrives as a stream of job events (progress,
 * cached or stale result, done, failed) over a Tauri `Channel`; keeping the
 * rules here makes them testable without Tauri. `useSiteMap.ts` feeds it.
 */
import type { AnalyzeProgress } from "../../shared/ipc/generated/AnalyzeProgress";
import type { AppError } from "../../shared/ipc/generated/AppError";
import type { SiteMap } from "../../shared/ipc/generated/SiteMap";
import type { SiteMapEvent } from "../../shared/ipc/generated/SiteMapEvent";

export type SiteMapState = {
  /** The latest map to show (may be an older one while a new run is going). */
  map: SiteMap | null;
  /** A run is going; `progress` says how far. */
  running: boolean;
  progress: AnalyzeProgress | null;
  /** `map` is from an earlier run (site is an address: no file fingerprint). */
  stale: boolean;
  error: AppError | null;
};

export type SiteMapAction =
  | { type: "started" }
  | { type: "event"; event: SiteMapEvent }
  | { type: "failedToStart"; error: AppError }
  | { type: "dismissError" };

export const initialSiteMap: SiteMapState = {
  map: null,
  running: false,
  progress: null,
  stale: false,
  error: null,
};

export function siteMapReducer(state: SiteMapState, action: SiteMapAction): SiteMapState {
  switch (action.type) {
    case "started":
      // Keep showing the previous map (other device or before a rescan)
      // until the new one arrives; it is marked stale meanwhile.
      return { ...state, running: true, progress: null, error: null, stale: state.map !== null };
    case "failedToStart":
      return { ...state, running: false, progress: null, error: action.error };
    case "dismissError":
      return { ...state, error: null };
    case "event": {
      const e = action.event;
      switch (e.kind) {
        case "progress":
          return { ...state, progress: e.progress };
        case "stale":
          return { ...state, map: e.map, stale: true };
        case "done":
          return { ...state, map: e.map, stale: false, running: false, progress: null };
        case "failed":
          // A cancelled run keeps the previous map and shows no error.
          return {
            ...state,
            running: false,
            progress: null,
            error: e.error.code === "SR-0004" ? null : e.error,
          };
      }
    }
  }
}
