/**
 * Canvas shapes: Start/End oval, block card, Navigate shape (HANDOFF D-38).
 *
 * Each is `memo`: React Flow re-renders shapes on every pan and zoom, and a
 * scenario can hold 200+ of them (NFR-05). Sizes are set by `toReactFlow.ts`
 * (block cards in the current device's shape, D-39); here they only fill it.
 * Handles are where arrows attach: one on each side, shown on hover. An arrow
 * is pulled out of any side (FR-CNV-08); under each "out" handle sits an
 * invisible "in" handle (`<side>-in`) where arrows arrive. Start has only
 * "out", End only "in" — the path begins and ends there.
 */
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { memo } from "react";
import { useTranslation } from "react-i18next";

import type { BlockData, NavigateData, TerminalData } from "./toReactFlow";

const SIDES = [Position.Top, Position.Right, Position.Bottom, Position.Left];

/** The handles of a shape: arrows out (`out`), arrows in (`in`), or both. */
function Handles({ out = true, into = true }: { out?: boolean; into?: boolean }) {
  return (
    <>
      {into &&
        SIDES.map((side) => (
          <Handle
            key={`${side}-in`}
            id={`${side}-in`}
            type="target"
            position={side}
            className="canvas-handle--in"
          />
        ))}
      {out && SIDES.map((side) => <Handle key={side} id={side} type="source" position={side} />)}
    </>
  );
}

/** "1.5 s" / "No stop" — how long the video stays on a shape. */
function useHoldText() {
  const { t } = useTranslation();
  return (ms: number) => (ms === 0 ? t("canvas.noStop") : t("canvas.hold", { seconds: ms / 1000 }));
}

export const TerminalShape = memo(function TerminalShape({
  data,
}: NodeProps<Node<TerminalData, "terminal">>) {
  const { t } = useTranslation();
  const holdText = useHoldText();
  const isStart = data.kind === "start";
  return (
    <div className={`canvas-terminal canvas-terminal--${data.kind}`}>
      <Handles out={isStart} into={!isStart} />
      <span className="canvas-terminal__name">{isStart ? t("canvas.start") : t("canvas.end")}</span>
      {data.holdMs !== null && data.holdMs > 0 && (
        <span className="canvas-terminal__hold">{holdText(data.holdMs)}</span>
      )}
    </div>
  );
});

export const BlockShape = memo(function BlockShape({ data }: NodeProps<Node<BlockData, "block">>) {
  const { t } = useTranslation();
  const holdText = useHoldText();
  return (
    <div className="canvas-block">
      <Handles />
      <header className="canvas-block__header">
        <span className="canvas-block__name">{data.name}</span>
        <span className="canvas-block__hold">{holdText(data.holdMs)}</span>
      </header>
      {/* The block's picture arrives with the site map pictures (step 3.6). */}
      <div className="canvas-block__picture" />
      {data.actions > 0 && (
        <footer className="canvas-block__footer">
          {t("canvas.actions", { count: data.actions })}
        </footer>
      )}
    </div>
  );
});

export const NavigateShape = memo(function NavigateShape({
  data,
}: NodeProps<Node<NavigateData, "navigate">>) {
  const { t } = useTranslation();
  return (
    <div className="canvas-navigate">
      <Handles />
      <span>
        {data.toPage ? t("canvas.navigateTo", { page: data.toPage }) : t("canvas.navigate")}
      </span>
    </div>
  );
});

/** Shape components by React Flow node type (module constant: React Flow requires a stable object). */
export const nodeTypes = {
  terminal: TerminalShape,
  block: BlockShape,
  navigate: NavigateShape,
};
