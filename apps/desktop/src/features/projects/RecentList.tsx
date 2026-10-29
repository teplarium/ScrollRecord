/**
 * RecentList — the last 10 projects on the start screen (FR-PRJ-03).
 * A project whose folder is gone (moved, deleted, unplugged drive) stays
 * visible but greyed out with "Remove from list", so the user understands
 * why it can't be opened instead of it silently disappearing.
 */
import { useTranslation } from "react-i18next";

import type { RecentProject } from "../../shared/ipc/generated/RecentProject";
import { CloseIcon } from "../../shared/ui/icons";

type Props = {
  recent: RecentProject[];
  onOpen: (dir: string) => void;
  onForget: (dir: string) => void;
};

export function RecentList({ recent, onOpen, onForget }: Props) {
  const { t, i18n } = useTranslation();

  if (recent.length === 0) {
    return <p className="start__recent-empty">{t("start.recentEmpty")}</p>;
  }

  const when = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });

  return (
    <ul className="recent">
      {recent.map((r) => (
        <li key={r.path} className={`recent__item${r.exists ? "" : " recent__item--missing"}`}>
          <button
            type="button"
            className="recent__open"
            onClick={() => onOpen(r.path)}
            disabled={!r.exists}
            title={r.path}
          >
            <span className="recent__name">{r.name}</span>
            <span className="recent__path">{r.path}</span>
          </button>
          <span className="recent__when">
            {r.exists ? when.format(new Date(r.openedMs)) : t("recent.missing")}
          </span>
          <button
            type="button"
            className="recent__forget"
            onClick={() => onForget(r.path)}
            title={t("recent.forget")}
            aria-label={t("recent.forget")}
          >
            <CloseIcon size={12} />
          </button>
        </li>
      ))}
    </ul>
  );
}
