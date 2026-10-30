/**
 * Canvas — the scenario as shapes and arrows (centre of the editor, D-38).
 *
 * Editing (step 3.4): shapes are moved by dragging, arrows are pulled out of a
 * shape's side and dropped on another shape (FR-CNV-08), Delete removes the
 * selection, and shapes dragged in from the site map land where they are
 * dropped (FR-CNV-07). Every change goes to the tab's project store as one
 * undo step (`shared/store`); the canvas never changes the scenario itself.
 *
 * Navigation like FigJam: the wheel / touchpad pans, Ctrl+wheel or a pinch
 * zooms, dragging the empty canvas pans. Only shapes in view are drawn, so
 * long scenarios stay smooth (NFR-05).
 */
import "@xyflow/react/dist/style.css";
import "./Canvas.css";

import {
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  useStore,
  type Connection,
  type Edge,
  type EdgeChange,
  type FinalConnectionState,
  type NodeChange,
  type OnSelectionChangeParams,
  type Viewport,
} from "@xyflow/react";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type DragEvent,
} from "react";
import { useTranslation } from "react-i18next";

import {
  addFromSource,
  connect,
  moveNodes,
  NODE_SOURCE_MIME,
  readNodeSource,
  remove,
  type NodeSource,
} from "../../shared/store/edits";
import type { Point } from "../../shared/store/layout";
import { useProject, type ProjectStore } from "../../shared/store/projectStore";
import { cardSize, sourceSize, type Size } from "./cardSize";
import { nodeTypes } from "./nodes";
import { freeSpot } from "./separate";
import {
  buildCanvas,
  edgeIndex,
  openingShapes,
  type CanvasGraph,
  type CanvasNode,
} from "./toReactFlow";

type Props = {
  /** The tab's project: what is shown and where edits go. */
  store: ProjectStore;
  /** Screen of the current device; block cards take its shape. */
  screen: Size | null;
};

export function Canvas(props: Props) {
  return (
    <ReactFlowProvider>
      <CanvasView {...props} />
    </ReactFlowProvider>
  );
}

function CanvasView({ store, screen }: Props) {
  const { t } = useTranslation();
  const { fitView, screenToFlowPosition, setCenter, getZoom } = useReactFlow();
  const { flow, layout } = useProject(store, (s) => s.history.present);
  const focus = useProject(store, (s) => s.focus);
  const box = useRef<HTMLDivElement>(null);
  const [connecting, setConnecting] = useState(false);
  // Handles keep one size on screen at any zoom (see Canvas.css).
  const zoom = useStore((s) => s.transform[2]);

  // ── Build shapes for the current device ──
  // Keyed on the numbers, not the object, so an equal screen doesn't rebuild.
  const width = screen?.width ?? 0;
  const height = screen?.height ?? 0;
  const card = useMemo(() => cardSize({ width, height }), [width, height]);
  const built = useMemo(() => buildCanvas(flow, layout, card), [flow, layout, card]);
  // The viewport saved in the project, read once when the canvas opens.
  const [savedViewport] = useState(() => store.getState().viewport);

  // ── React Flow's working copy of the shapes and arrows ──
  // It adds measurements, selection and drag positions. A rebuild (an edit,
  // undo, another device) replaces it during render, keeping the selection.
  const [shown, setShown] = useState<{ built: CanvasGraph; nodes: CanvasNode[]; edges: Edge[] }>({
    built,
    nodes: built.nodes,
    edges: built.edges,
  });
  if (shown.built !== built) {
    const nodeSel = new Set(shown.nodes.filter((n) => n.selected).map((n) => n.id));
    const edgeSel = new Set(shown.edges.filter((e) => e.selected).map((e) => e.id));
    setShown({
      built,
      nodes: built.nodes.map((n) => (nodeSel.has(n.id) ? { ...n, selected: true } : n)),
      edges: built.edges.map((e) => (edgeSel.has(e.id) ? { ...e, selected: true } : e)),
    });
  }

  const onNodesChange = useCallback((changes: NodeChange<CanvasNode>[]) => {
    setShown((s) => ({ ...s, nodes: applyNodeChanges(changes, s.nodes) }));
  }, []);
  const onEdgesChange = useCallback((changes: EdgeChange[]) => {
    setShown((s) => ({ ...s, edges: applyEdgeChanges(changes, s.edges) }));
  }, []);

  const onSelectionChange = useCallback(
    ({ nodes }: OnSelectionChangeParams) =>
      store.getState().setView({ selectedId: nodes[0]?.id ?? null }),
    [store],
  );

  // ── Edits: each is one undo step in the project store ──
  const edit = store.getState().edit;

  // A drag of one or several shapes is saved once, where they were dropped.
  const onNodeDragStop = useCallback(
    (_: unknown, __: unknown, nodes: CanvasNode[]) => {
      const positions = Object.fromEntries(
        nodes.map((n) => [n.id, { x: Math.round(n.position.x), y: Math.round(n.position.y) }]),
      );
      edit((doc) => moveNodes(doc, positions));
    },
    [edit],
  );

  const onConnect = useCallback(
    ({ source, target }: Connection) => edit((doc) => connect(doc, source, target)),
    [edit],
  );

  // An arrow dropped on a shape but not on its handle still connects to it.
  const onConnectEnd = useCallback(
    (event: MouseEvent | TouchEvent, state: FinalConnectionState) => {
      setConnecting(false);
      if (state.isValid || !state.fromNode) return;
      const point = "changedTouches" in event ? event.changedTouches[0] : event;
      if (!point) return;
      const under = document.elementFromPoint(point.clientX, point.clientY);
      const target = under?.closest(".react-flow__node")?.getAttribute("data-id");
      const from = state.fromNode.id;
      if (target && target !== from) edit((doc) => connect(doc, from, target));
    },
    [edit],
  );

  const onDelete = useCallback(
    ({ nodes, edges }: { nodes: CanvasNode[]; edges: Edge[] }) => {
      const nodeIds = nodes.map((n) => n.id);
      const edgeIndexes = edges.flatMap((e) => edgeIndex(e.id) ?? []);
      edit((doc) => remove(doc, nodeIds, edgeIndexes));
    },
    [edit],
  );

  // ── Shapes dragged in from the site map ──
  const onDragOver = useCallback((event: DragEvent) => {
    if (!event.dataTransfer.types.includes(NODE_SOURCE_MIME)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  }, []);
  const onDrop = useCallback(
    (event: DragEvent) => {
      const source = readNodeSource(event.dataTransfer.getData(NODE_SOURCE_MIME));
      if (!source) return;
      event.preventDefault();
      // Centred where it was dropped: the user chose the spot.
      const point = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      place(store, source, centred(point, sourceSize(source.kind, card)));
    },
    [store, screenToFlowPosition, card],
  );

  // ── Rows clicked in the site map: a free spot near the middle of the view ──
  const addRequest = useProject(store, (s) => s.addRequest);
  const doneAdd = useRef(addRequest?.seq ?? 0);
  useEffect(() => {
    const rect = box.current?.getBoundingClientRect();
    if (!addRequest || addRequest.seq === doneAdd.current || !rect) return;
    doneAdd.current = addRequest.seq;
    const middle = screenToFlowPosition({
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    });
    const size = sourceSize(addRequest.source.kind, card);
    const boxes = shown.nodes.map((n) => ({ ...n, width: n.width ?? 0, height: n.height ?? 0 }));
    place(store, addRequest.source, freeSpot(boxes, centred(middle, size), size), true);
  }, [addRequest, store, screenToFlowPosition, card, shown.nodes]);

  const rememberView = useCallback(
    (viewport: Viewport) => store.getState().setViewport(viewport),
    [store],
  );

  // ── Framing: the start of the scenario on open and after a device switch ──
  // (cards change size with the device). A saved viewport wins on open.
  const frameStart = useCallback(
    (duration = 0) =>
      void fitView({ nodes: openingShapes(built.nodes), padding: 0.1, maxZoom: 1, duration }),
    [fitView, built],
  );
  const onInit = useCallback(() => {
    if (!savedViewport) frameStart();
  }, [savedViewport, frameStart]);

  const firstCard = useRef(card);
  useEffect(() => {
    if (firstCard.current === card) return;
    firstCard.current = card;
    const frame = requestAnimationFrame(() => frameStart(250));
    return () => cancelAnimationFrame(frame);
  }, [card, frameStart]);

  // ── "Show this shape" from the site map: select it and bring it into view ──
  const shownFocus = useRef(focus?.seq ?? 0);
  useEffect(() => {
    if (!focus || focus.seq === shownFocus.current) return;
    shownFocus.current = focus.seq;
    setShown((s) => ({
      ...s,
      nodes: s.nodes.map((n) => ({ ...n, selected: n.id === focus.id })),
      edges: s.edges.map((e) => (e.selected ? { ...e, selected: false } : e)),
    }));
    const node = built.nodes.find((n) => n.id === focus.id);
    if (focus.show && node) {
      const x = node.position.x + (node.width ?? 0) / 2;
      const y = node.position.y + (node.height ?? 0) / 2;
      void setCenter(x, y, { zoom: getZoom(), duration: 250 });
    }
  }, [focus, built, setCenter, getZoom]);

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
    <div
      ref={box}
      className={connecting ? "canvas canvas--connecting" : "canvas"}
      style={{ "--canvas-zoom": zoom } as CSSProperties}
      onDragOver={onDragOver}
      onDrop={onDrop}
    >
      <ReactFlow
        nodes={shown.nodes}
        edges={shown.edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onSelectionChange={onSelectionChange}
        onNodeDragStop={onNodeDragStop}
        onConnect={onConnect}
        onConnectStart={() => setConnecting(true)}
        onConnectEnd={onConnectEnd}
        onDelete={onDelete}
        onMoveEnd={(_, viewport) => rememberView(viewport)}
        deleteKeyCode={DELETE_KEYS}
        defaultEdgeOptions={EDGE_OPTIONS}
        connectionRadius={40}
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
        {...(savedViewport ? { defaultViewport: savedViewport } : {})}
      >
        <Background variant={BackgroundVariant.Dots} gap={32} size={1.5} />
        <Controls showInteractive={false} position="bottom-left" />
        <MiniMap position="bottom-right" pannable zoomable nodeClassName={minimapClass} />
      </ReactFlow>
      {flow.nodes.length === 0 && <p className="canvas__empty">{t("canvas.empty")}</p>}
    </div>
  );
}

const DELETE_KEYS = ["Delete", "Backspace"];
// A wide invisible stroke makes thin arrows easy to click when zoomed out.
const EDGE_OPTIONS = { interactionWidth: 40 };

/** Top-left of a shape of `size` centred on `point`, in whole pixels. */
function centred(point: Point, size: Size): Point {
  return { x: Math.round(point.x - size.width / 2), y: Math.round(point.y - size.height / 2) };
}

/**
 * Adds a shape from the site map at `at` (top-left, canvas px) as one undo
 * step and selects it; `show` also pans to it (a clicked row may land below
 * the view). Start/End exist once: if there already, it is selected.
 */
function place(store: ProjectStore, source: NodeSource, at: Point, show = false) {
  let id = "";
  store.getState().edit((doc) => {
    const added = addFromSource(doc, source, at);
    id = added.id;
    return added.doc;
  });
  store.getState().focusNode(id, show);
}

function minimapClass(node: CanvasNode) {
  return `canvas-minimap--${node.type}`;
}
