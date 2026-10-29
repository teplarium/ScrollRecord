/**
 * TabBar — the strip at the very top of the window: Home, one tab per open
 * project, and "+" for a new project (HANDOFF D-38, like file tabs in Figma).
 * Why here and not in a feature: tabs decide which screen the window shows,
 * which is the job of `app/` (features never import each other).
 */
import { useTranslation } from "react-i18next";

import type { OpenedProject } from "../shared/ipc/generated/OpenedProject";
import { CloseIcon, HomeIcon, PlusIcon } from "../shared/ui/icons";
import { sameDir } from "./tabs";
import "./TabBar.css";

type Props = {
  projects: OpenedProject[];
  /** Folder of the shown project, `null` = Home. */
  active: string | null;
  onHome: () => void;
  onSelect: (dir: string) => void;
  onClose: (dir: string) => void;
  onNew: () => void;
};

export function TabBar({ projects, active, onHome, onSelect, onClose, onNew }: Props) {
  const { t } = useTranslation();

  return (
    <nav className="tabbar" aria-label={t("tabs.label")}>
      <button
        type="button"
        className={`tabbar__home${active === null ? " tabbar__tab--active" : ""}`}
        onClick={onHome}
        title={t("tabs.home")}
        aria-label={t("tabs.home")}
      >
        <HomeIcon />
      </button>

      <div className="tabbar__tabs" role="tablist">
        {projects.map((p) => {
          const isActive = active !== null && sameDir(active, p.dir);
          return (
            <div
              key={p.dir}
              className={`tabbar__tab${isActive ? " tabbar__tab--active" : ""}`}
              title={p.dir}
            >
              <button
                type="button"
                role="tab"
                aria-selected={isActive}
                className="tabbar__tab-name"
                onClick={() => onSelect(p.dir)}
                // Middle click closes a tab, as in browsers and Figma.
                onAuxClick={(e) => {
                  if (e.button === 1) onClose(p.dir);
                }}
              >
                {p.manifest.name}
              </button>
              <button
                type="button"
                className="tabbar__tab-close"
                onClick={() => onClose(p.dir)}
                aria-label={t("tabs.close", { name: p.manifest.name })}
                title={t("tabs.close", { name: p.manifest.name })}
              >
                <CloseIcon size={12} />
              </button>
            </div>
          );
        })}
      </div>

      <button
        type="button"
        className="tabbar__new"
        onClick={onNew}
        title={t("tabs.newProject")}
        aria-label={t("tabs.newProject")}
      >
        <PlusIcon size={16} />
      </button>
    </nav>
  );
}
