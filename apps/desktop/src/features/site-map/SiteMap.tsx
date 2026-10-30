/**
 * SiteMap — the editor's left panel (HANDOFF D-38, FR-CNV-02, list mode).
 *
 * Shows what can go onto the canvas: Start and End at the top, then a tree
 * page → blocks → interactive elements, rows "icon + name" that expand and
 * collapse like Figma layers. Blocks and elements come from the site itself
 * (`sr-analyzer`, run by `useSiteMap`): `data-sr` blocks plus the blocks the
 * scenario uses; blocks already in the scenario are marked, blocks the
 * scenario names but this device doesn't have get a warning.
 * Dragging rows onto the canvas comes in step 3.4, pictures in 3.6.
 *
 * The project's website is shown at the top; a site folder that was moved
 * (`siteMissing`) is reported here with "Locate site folder…".
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { ElementKind } from "../../shared/ipc/generated/ElementKind";
import type { Flow } from "../../shared/ipc/generated/Flow";
import type { PageMap } from "../../shared/ipc/generated/PageMap";
import type { SiteBlock } from "../../shared/ipc/generated/SiteBlock";
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
};

export function SiteMap({ flow, siteMissing, onLocateSite, siteMap }: Props) {
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
        <li className="site-map__row">
          <span className="site-map__oval site-map__oval--start" />
          {t("canvas.start")}
        </li>
        <li className="site-map__row">
          <span className="site-map__oval" />
          {t("canvas.end")}
        </li>
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
  pageId: string;
  page: PageMap | null;
  loading: boolean;
  openBlocks: Set<string>;
  onToggle: (key: string) => void;
};

function PageBlocks({ pageId, page, loading, openBlocks, onToggle }: PageBlocksProps) {
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
            open={openBlocks.has(key)}
            onToggle={() => onToggle(key)}
          />
        );
      })}
    </ul>
  );
}

function BlockRow({
  block,
  open,
  onToggle,
}: {
  block: SiteBlock;
  open: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  const missing = block.status !== "found";
  const hasElements = block.elements.length > 0;
  const title = missing
    ? t("siteMap.blockMissing", { tried: block.tried.join("; ") })
    : block.node
      ? t("siteMap.inScenario")
      : undefined;

  return (
    <li role="treeitem" aria-expanded={hasElements ? open : undefined}>
      <button
        type="button"
        className={`site-map__row site-map__row--toggle${missing ? " site-map__row--missing" : ""}`}
        onClick={onToggle}
        disabled={!hasElements}
        title={title}
      >
        {hasElements ? <ChevronIcon open={open} /> : <span className="site-map__chevron-space" />}
        {missing ? <WarningIcon size={14} /> : <KindIcon kind="block" />}
        <span className="site-map__name">{block.name}</span>
        {block.node && !missing && <span className="site-map__used" />}
      </button>
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
