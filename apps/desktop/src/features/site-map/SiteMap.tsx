/**
 * SiteMap — the editor's left panel (HANDOFF D-38, FR-CNV-02, list mode).
 *
 * Shows what can go onto the canvas: Start and End at the top, then a tree
 * page → blocks → interactive elements, rows "icon + name" that expand and
 * collapse like Figma layers. Blocks and elements come from the site itself
 * (`sr-analyzer`, run by `useSiteMap`): `data-sr` blocks plus the blocks the
 * scenario uses; blocks already in the scenario are marked, blocks the
 * scenario names but this device doesn't have get a warning. Pictures: 3.6.
 *
 * Putting things on the canvas (FR-CNV-07): drag Start, End or a block onto
 * it, or click the row — the shape appears in the middle of the visible
 * canvas. A row already on the canvas (Start, End, a used block) is shown
 * there on click instead; a second copy of a block is added by dragging.
 * The chevron expands a block; its elements become cursor actions in 3.5.
 *
 * The project's website is shown at the top; a site folder that was moved
 * (`siteMissing`) is reported here with "Locate site folder…".
 */
import { useState, type DragEvent } from "react";
import { useTranslation } from "react-i18next";

import type { ElementKind } from "../../shared/ipc/generated/ElementKind";
import type { Flow } from "../../shared/ipc/generated/Flow";
import type { PageMap } from "../../shared/ipc/generated/PageMap";
import type { SiteBlock } from "../../shared/ipc/generated/SiteBlock";
import { NODE_SOURCE_MIME, type NodeSource } from "../../shared/store/edits";
import { ErrorBanner } from "../../shared/ui/ErrorBanner";
import { WarningIcon } from "../../shared/ui/icons";
import { KindIcon, RescanIcon, ChevronIcon } from "./icons";
import type { useSiteMap } from "./useSiteMap";
import "./SiteMap.css";

type Props = {
  flow: Flow;
  siteMissing: boolean;
  /** "Locate site folder…" was clicked. */
  onLocateSite: () => void;
  /** The site map job of this tab (`useSiteMap`). */
  siteMap: ReturnType<typeof useSiteMap>;
  /** A row was clicked: put it in the middle of the visible canvas. */
  onAdd: (source: NodeSource) => void;
  /** A row already on the canvas was clicked: select and show that node. */
  onShow: (nodeId: string) => void;
};

/** Starts dragging a row onto the canvas (`Canvas.tsx` reads it on drop). */
function dragSource(source: NodeSource) {
  return (event: DragEvent) => {
    event.dataTransfer.setData(NODE_SOURCE_MIME, JSON.stringify(source));
    event.dataTransfer.effectAllowed = "copy";
  };
}

/**
 * The scenario node a site block is, if any: the node the analyzer matched,
 * or (added since the last read) a block on that page with the same locators.
 */
function usedNode(flow: Flow, pageId: string, block: SiteBlock): string | null {
  if (block.node && flow.nodes.some((n) => n.id === block.node)) return block.node;
  const locators = JSON.stringify(block.locators);
  const node = flow.nodes.find(
    (n) => n.kind === "block" && n.page === pageId && JSON.stringify(n.target) === locators,
  );
  return node?.id ?? null;
}

export function SiteMap({ flow, siteMissing, onLocateSite, siteMap, onAdd, onShow }: Props) {
  const { t } = useTranslation();
  const { state } = siteMap;
  const site = flow.meta.site;
  // Expanded rows: pages start open, blocks closed (like Figma layers).
  const [closedPages, setClosedPages] = useState<Set<string>>(new Set());
  const [openBlocks, setOpenBlocks] = useState<Set<string>>(new Set());
  const toggle = (set: Set<string>, key: string) => {
    const next = new Set(set);
    if (!next.delete(key)) next.add(key);
    return next;
  };

  return (
    <aside className="site-map" aria-label={t("siteMap.title")}>
      <div className="site-map__head">
        <h2 className="site-map__title">{t("siteMap.title")}</h2>
        <button
          type="button"
          className="site-map__icon-button"
          onClick={siteMap.rescan}
          disabled={state.running || siteMissing}
          title={t("siteMap.rescan")}
          aria-label={t("siteMap.rescan")}
        >
          <RescanIcon />
        </button>
      </div>

      <div className="site-map__site" title={site ?? undefined}>
        <span className="site-map__label">{t("siteMap.site")}</span>
        <span className="site-map__path">{site ?? t("siteMap.noSite")}</span>
      </div>
      {siteMissing && (
        <div className="site-map__warning" role="alert">
          <WarningIcon size={16} />
          <span>{t("siteMap.siteMissing")}</span>
          <button type="button" className="button" onClick={onLocateSite}>
            {t("siteMap.locate")}
          </button>
        </div>
      )}

      {state.running && (
        <div className="site-map__status" role="status">
          <span>
            {state.progress
              ? t("siteMap.reading", {
                  done: state.progress.done + 1,
                  total: state.progress.total,
                })
              : t("siteMap.starting")}
          </span>
          <button type="button" className="button" onClick={siteMap.cancel}>
            {t("siteMap.cancel")}
          </button>
        </div>
      )}
      {state.error && <ErrorBanner error={state.error} onDismiss={siteMap.dismissError} />}

      <ul className="site-map__list">
        {(["start", "end"] as const).map((kind) => {
          const existing = flow.nodes.find((n) => n.kind === kind);
          return (
            <li key={kind}>
              <button
                type="button"
                className="site-map__row site-map__row--toggle site-map__row--source"
                draggable={!existing}
                onDragStart={dragSource({ kind })}
                onClick={() => (existing ? onShow(existing.id) : onAdd({ kind }))}
                title={existing ? t("siteMap.showOnCanvas") : t("siteMap.addHint")}
              >
                <span
                  className={`site-map__oval${kind === "start" ? " site-map__oval--start" : ""}`}
                />
                {kind === "start" ? t("canvas.start") : t("canvas.end")}
                {existing && <span className="site-map__used" />}
              </button>
            </li>
          );
        })}
      </ul>

      <h3 className="site-map__section">{t("siteMap.pages")}</h3>
      <ul className={`site-map__list${state.stale ? " site-map__list--stale" : ""}`} role="tree">
        {flow.pages.map((page) => {
          const open = !closedPages.has(page.id);
          const read = state.map?.pages.find((p) => p.page === page.id) ?? null;
          return (
            <li key={page.id} role="treeitem" aria-expanded={open}>
              <button
                type="button"
                className="site-map__row site-map__row--toggle"
                onClick={() => setClosedPages((s) => toggle(s, page.id))}
                title={page.url}
              >
                <ChevronIcon open={open} />
                <span className="site-map__page">{page.id}</span>
                <span className="site-map__url">{page.url}</span>
              </button>
              {open && (
                <PageBlocks
                  flow={flow}
                  onAdd={onAdd}
                  onShow={onShow}
                  page={read}
                  loading={state.running}
                  openBlocks={openBlocks}
                  onToggle={(key) => setOpenBlocks((s) => toggle(s, key))}
                  pageId={page.id}
                />
              )}
            </li>
          );
        })}
      </ul>

      <button type="button" className="site-map__link" onClick={() => void siteMap.clearCache()}>
        {t("siteMap.clearCache")}
      </button>
    </aside>
  );
}

type PageBlocksProps = {
  flow: Flow;
  onAdd: (source: NodeSource) => void;
  onShow: (nodeId: string) => void;
  pageId: string;
  page: PageMap | null;
  loading: boolean;
  openBlocks: Set<string>;
  onToggle: (key: string) => void;
};

function PageBlocks(props: PageBlocksProps) {
  const { flow, onAdd, onShow, pageId, page, loading, openBlocks, onToggle } = props;
  const { t } = useTranslation();
  if (!page) {
    return (
      <p className="site-map__note">{loading ? t("siteMap.loading") : t("siteMap.notRead")}</p>
    );
  }
  if (page.blocks.length === 0) {
    return <p className="site-map__note">{t("siteMap.noBlocks")}</p>;
  }
  return (
    <ul className="site-map__list site-map__list--nested" role="group">
      {page.blocks.map((block) => {
        const key = `${pageId}/${block.name}/${block.node ?? ""}`;
        return (
          <BlockRow
            key={key}
            block={block}
            used={usedNode(flow, pageId, block)}
            source={{ kind: "block", page: pageId, name: block.name, locators: block.locators }}
            onAdd={onAdd}
            onShow={onShow}
            open={openBlocks.has(key)}
            onToggle={() => onToggle(key)}
          />
        );
      })}
    </ul>
  );
}

type BlockRowProps = {
  block: SiteBlock;
  /** Scenario node of this block, if it is on the canvas. */
  used: string | null;
  source: NodeSource;
  onAdd: (source: NodeSource) => void;
  onShow: (nodeId: string) => void;
  open: boolean;
  onToggle: () => void;
};

function BlockRow({ block, used, source, onAdd, onShow, open, onToggle }: BlockRowProps) {
  const { t } = useTranslation();
  const missing = block.status !== "found";
  const hasElements = block.elements.length > 0;
  // A block the scenario names but this device lacks can't be added again.
  const addable = !missing && block.locators.length > 0;
  const title = missing
    ? t("siteMap.blockMissing", { tried: block.tried.join("; ") })
    : used
      ? t("siteMap.inScenario")
      : t("siteMap.addHint");

  return (
    <li role="treeitem" aria-expanded={hasElements ? open : undefined}>
      <div className="site-map__line">
        {hasElements ? (
          <button
            type="button"
            className="site-map__chevron-button"
            onClick={onToggle}
            aria-label={open ? t("siteMap.collapse") : t("siteMap.expand")}
          >
            <ChevronIcon open={open} />
          </button>
        ) : (
          <span className="site-map__chevron-space" />
        )}
        <button
          type="button"
          className={`site-map__row site-map__row--toggle site-map__row--source${
            missing ? " site-map__row--missing" : ""
          }`}
          draggable={addable}
          onDragStart={dragSource(source)}
          onClick={() => {
            if (used) onShow(used);
            else if (addable) onAdd(source);
          }}
          disabled={!used && !addable}
          title={title}
        >
          {missing ? <WarningIcon size={14} /> : <KindIcon kind="block" />}
          <span className="site-map__name">{block.name}</span>
          {used && !missing && <span className="site-map__used" />}
        </button>
      </div>
      {open && hasElements && (
        <ul className="site-map__list site-map__list--nested" role="group">
          {block.elements.map((element, i) => (
            <li
              key={i}
              role="treeitem"
              className={`site-map__row site-map__row--element${
                element.locators.length === 0 ? " site-map__row--weak" : ""
              }`}
              title={
                element.locators.length === 0
                  ? t("siteMap.noLocator")
                  : t("siteMap.elementTitle", {
                      kind: kindName(t, element.kind),
                      label: element.label,
                    })
              }
            >
              <KindIcon kind={element.kind} />
              <span className="site-map__name">{element.label}</span>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

function kindName(t: ReturnType<typeof useTranslation>["t"], kind: ElementKind): string {
  return {
    button: t("siteMap.kind.button"),
    link: t("siteMap.kind.link"),
    input: t("siteMap.kind.input"),
    checkbox: t("siteMap.kind.checkbox"),
    radio: t("siteMap.kind.radio"),
    select: t("siteMap.kind.select"),
    textarea: t("siteMap.kind.textarea"),
    tab: t("siteMap.kind.tab"),
    slider: t("siteMap.kind.slider"),
    draggable: t("siteMap.kind.draggable"),
    other: t("siteMap.kind.other"),
  }[kind];
}
