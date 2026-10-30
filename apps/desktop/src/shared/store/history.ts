/**
 * Undo/redo history of one project tab (FR-PRJ-05, ADR 0006 §3).
 *
 * Why our own and not a library: we need two rules a generic one doesn't
 * give: a drag is one step (the caller commits once, on drag end) and typing
 * in one field is one step (`coalesceKey`: equal keys in a row merge). The
 * states are immutable snapshots with structural sharing, so keeping 200 of
 * them costs little more than the parts that changed.
 *
 * Rules: pure functions, no React. `present` is what the editor shows; a new
 * step clears the redo list; the oldest steps are dropped past the limit.
 */

/** Most undo steps kept per tab. */
export const HISTORY_LIMIT = 200;

export type History<T> = {
  past: T[];
  present: T;
  future: T[];
  /** Key of the last step, to merge the next one with the same key. */
  lastKey: string | null;
};

export function startHistory<T>(present: T): History<T> {
  return { past: [], present, future: [], lastKey: null };
}

/**
 * Makes `next` the present as a new undo step. With a `coalesceKey` equal to
 * the previous step's key the two merge (one undo restores the state before
 * both). An unchanged state (`next === present`) is not a step.
 */
export function commit<T>(history: History<T>, next: T, coalesceKey?: string): History<T> {
  if (next === history.present) return history;
  const key = coalesceKey ?? null;
  if (key !== null && key === history.lastKey) {
    return { ...history, present: next, future: [] };
  }
  const past = [...history.past, history.present];
  if (past.length > HISTORY_LIMIT) past.splice(0, past.length - HISTORY_LIMIT);
  return { past, present: next, future: [], lastKey: key };
}

export function undo<T>(history: History<T>): History<T> {
  const previous = history.past.at(-1);
  if (previous === undefined) return history;
  return {
    past: history.past.slice(0, -1),
    present: previous,
    future: [history.present, ...history.future],
    lastKey: null,
  };
}

export function redo<T>(history: History<T>): History<T> {
  const [next, ...rest] = history.future;
  if (next === undefined) return history;
  return {
    past: [...history.past, history.present],
    present: next,
    future: rest,
    lastKey: null,
  };
}
