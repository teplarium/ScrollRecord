/**
 * Pushes overlapping shapes apart for display (ADR 0006, decided in 3.4).
 *
 * Why: positions in `layout.json` are shared by all devices, but a block card
 * takes the shape of the current device (D-39): a column laid out with wide
 * desktop cards overlaps when the cards turn into tall phone screens. Instead
 * of keeping positions per device, the canvas shows each shape at its saved
 * place unless that would overlap a shape above it; then it moves down just
 * below. Nothing here is saved: only a shape the user drags gets a new saved
 * position (where it was dropped).
 *
 * Shapes are taken top to bottom (saved y, then x), so the upper shape stays
 * and the lower one moves; a moved shape can push the ones below it in turn.
 */
import type { Point } from "../../shared/store/layout";
import type { Size } from "./cardSize";

/** Least room kept between a shape and the one it was pushed under (arrow). */
export const MIN_GAP = 40;

export type Box = { id: string; position: Point } & Size;

/** Display positions by id; only shapes that had to move are listed. */
export function separate(boxes: Box[]): Map<string, Point> {
  const order = [...boxes].sort(
    (a, b) => a.position.y - b.position.y || a.position.x - b.position.x,
  );
  const placed: { x: number; y: number; right: number; bottom: number }[] = [];
  const moved = new Map<string, Point>();

  for (const box of order) {
    const { x } = box.position;
    const right = x + box.width;
    let y = box.position.y;
    // Each hit moves the shape just below the shape it hits. y only grows,
    // and only to one of the placed shapes' bottoms, so this ends.
    let hit = true;
    while (hit) {
      hit = false;
      for (const p of placed) {
        const overlapsX = x < p.right && right > p.x;
        const overlapsY = y < p.bottom + MIN_GAP && y + box.height + MIN_GAP > p.y;
        if (overlapsX && overlapsY) {
          y = p.bottom + MIN_GAP;
          hit = true;
        }
      }
    }
    if (y !== box.position.y) moved.set(box.id, { x, y });
    placed.push({ x, y, right, bottom: y + box.height });
  }
  return moved;
}

/**
 * Where to put a new shape of `size` wanted at `at` (top-left) so it overlaps
 * nothing in `boxes`: straight down from `at`, below the shapes in the way.
 * Why: a shape clicked in from the site map lands in the middle of the view,
 * which is often where the scenario already is.
 */
export function freeSpot(boxes: Box[], at: Point, size: Size): Point {
  const placed = boxes.map((b) => ({
    x: b.position.x,
    y: b.position.y,
    right: b.position.x + b.width,
    bottom: b.position.y + b.height,
  }));
  let y = at.y;
  let hit = true;
  while (hit) {
    hit = false;
    for (const p of placed) {
      const overlapsX = at.x < p.right && at.x + size.width > p.x;
      const overlapsY = y < p.bottom + MIN_GAP && y + size.height + MIN_GAP > p.y;
      if (overlapsX && overlapsY) {
        y = p.bottom + MIN_GAP;
        hit = true;
      }
    }
  }
  return { x: at.x, y };
}
