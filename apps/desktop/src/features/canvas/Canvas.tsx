/**
 * Canvas — the scenario as shapes and arrows (centre of the editor, D-38).
 *
 * Read-only in step 3.2: pan, zoom, "fit all", minimap and selecting a shape
 * (the properties panel shows it). Moving, adding and connecting shapes are
 * edits and arrive with the document store and undo history in step 3.4.
 *
 * Navigation like FigJam: the wheel / touchpad pans, Ctrl+wheel or a pinch
 * zooms, dragging the empty canvas pans. Only shapes in view are drawn, so
 * long scenarios stay smooth (NFR-05).
 */
import "@xyflow/react/dist/style.css";
import "./Canvas.css";

import {
  applyNodeChanges,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type NodeChange,
  type OnSelectionChangeParams,
} from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import type { Flow } from "../../shared/ipc/generated/Flow";
import { cardSize, type Size } from "./cardSize";
import { readLayout } from "./layout";
import { nodeTypes } from "./nodes";
import { buildCanvas, openingShapes, type CanvasGraph, type CanvasNode } from "./toReactFlow";

type Props = {
  flow: Flow;
  /** `layout.json` content (free JSON, read defensively). */
  layout: unknown;
  /** Screen of the current device; block cards take its shape. */
  screen: Size | null;
  /** The selected shape changed (`null` = nothing selected). */
  onSelect: (nodeId: string | null) => void;
};

export function Canvas(props: Props) {
  return (
    <ReactFlowProvider>
      <CanvasView {...props} />
    </ReactFlowProvider>
  );
}

function CanvasView({ flow, layout, screen, onSelect }: Props) {
  const { t } = useTranslation();
  const { fitView } = useReactFlow();

  // ── Build shapes for the current device ──
  // Keyed on the numbers, not the object, so an equal screen doesn't rebuild.
  const width = screen?.width ?? 0;
  const height = screen?.height ?? 0;
  const card = useMemo(() => cardSize({ width, height }), [width, height]);
  const saved = useMemo(() => readLayout(layout), [layout]);
  const built = useMemo(() => buildCanvas(flow, saved, card), [flow, saved, card]);

  // ── React Flow's working copy of the shapes ──
  // It adds measurements and selection to them. Rebuilt shapes (other device)
  // replace it during render, keeping what was selected.
  const [shown, setShown] = useState<{ built: CanvasGraph; nodes: CanvasNode[] }>({
    built,
    nodes: built.nodes,
  });
  if (shown.built !== built) {
    const selected = new Set(shown.nodes.filter((n) => n.selected).map((n) => n.id));
    setShown({
      built,
      nodes: built.nodes.map((n) => (selected.has(n.id) ? { ...n, selected: true } : n)),
    });
  }

  const onNodesChange = useCallback((changes: NodeChange<CanvasNode>[]) => {
    setShown((s) => ({ ...s, nodes: applyNodeChanges(changes, s.nodes) }));
  }, []);

  const onSelectionChange = useCallback(
    ({ nodes }: OnSelectionChangeParams) => onSelect(nodes[0]?.id ?? null),
    [onSelect],
  );

  // ── Framing: the start of the scenario on open and after a device switch ──
  // (cards change size with the device). A saved viewport wins on open.
  const frameStart = useCallback(
    (duration = 0) =>
      void fitView({ nodes: openingShapes(built.nodes), padding: 0.1, maxZoom: 1, duration }),
    [fitView, built],
  );
  const onInit = useCallback(() => {
    if (!saved.viewport) frameStart();
  }, [saved.viewport, frameStart]);

  const firstCard = useRef(card);
  useEffect(() => {
    if (firstCard.current === card) return;
    firstCard.current = card;
    const frame = requestAnimationFrame(() => frameStart(250));
    return () => cancelAnimationFrame(frame);
  }, [card, frameStart]);

  const ariaLabels = useMemo(
    () => ({
      "controls.zoomIn.ariaLabel": t("canvas.zoomIn"),
      "controls.zoomOut.ariaLabel": t("canvas.zoomOut"),
      "controls.fitView.ariaLabel": t("canvas.fitView"),
      "minimap.ariaLabel": t("canvas.minimap"),
    }),
    [t],
  );

  return (
    <div className="canvas">
      <ReactFlow
        nodes={shown.nodes}
        edges={built.edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onSelectionChange={onSelectionChange}
        nodesDraggable={false}
        nodesConnectable={false}
        edgesFocusable={false}
        deleteKeyCode={null}
        panOnScroll
        panOnDrag
        zoomOnDoubleClick={false}
        minZoom={0.02}
        maxZoom={2}
        onlyRenderVisibleElements
        colorMode="system"
        ariaLabelConfig={ariaLabels}
        onInit={onInit}
        fitViewOptions={{ padding: 0.1, maxZoom: 1 }}
        {...(saved.viewport ? { defaultViewport: saved.viewport } : {})}
      >
        <Background variant={BackgroundVariant.Dots} gap={32} size={1.5} />
        <Controls showInteractive={false} position="bottom-left" />
        <MiniMap position="bottom-right" pannable zoomable nodeClassName={minimapClass} />
      </ReactFlow>
      {flow.nodes.length === 0 && <p className="canvas__empty">{t("canvas.empty")}</p>}
    </div>
  );
}

function minimapClass(node: CanvasNode) {
  return `canvas-minimap--${node.type}`;
}
