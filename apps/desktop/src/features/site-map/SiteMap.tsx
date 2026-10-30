/**
 * SiteMap — the editor's left panel (HANDOFF D-38, FR-CNV-02).
 *
 * Shows what can go onto the canvas: Start and End at the top, then the
 * site's pages. Step 3.3 adds each page's blocks and their interactive
 * elements (found on the site by `sr-analyzer`), 3.4 lets them be dragged or
 * clicked onto the canvas, 3.6 adds pictures. For now it is a read-only list.
 *
 * The project's website is shown at the top; a site folder that was moved
 * (`siteMissing`) is reported here with "Locate site folder…".
 */
import { useTranslation } from "react-i18next";

import type { Flow } from "../../shared/ipc/generated/Flow";
import { WarningIcon } from "../../shared/ui/icons";
import "./SiteMap.css";

type Props = {
  flow: Flow;
  siteMissing: boolean;
  /** "Locate site folder…" was clicked. */
  onLocateSite: () => void;
};

export function SiteMap({ flow, siteMissing, onLocateSite }: Props) {
  const { t } = useTranslation();
  const site = flow.meta.site;

  return (
    <aside className="site-map" aria-label={t("siteMap.title")}>
      <h2 className="site-map__title">{t("siteMap.title")}</h2>

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
      <ul className="site-map__list">
        {flow.pages.map((page) => (
          <li key={page.id} className="site-map__row" title={page.url}>
            <span className="site-map__page">{page.id}</span>
            <span className="site-map__url">{page.url}</span>
          </li>
        ))}
      </ul>
      <p className="site-map__later">{t("siteMap.blocksLater")}</p>
    </aside>
  );
}
