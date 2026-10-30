/**
 * RecentList — the last 10 projects on the start screen (FR-PRJ-03).
 *
 * Each row's button depends on the project's state:
 * - folder exists → trash can "Delete project": asks first, then moves the
 *   folder to the Recycle Bin (recoverable). Not offered while the project is
 *   open in a tab — close the tab first (owner decision, step 3.2);
 * - folder gone (moved, deleted, unplugged drive) → greyed out with ×
 *   "Remove from list", so the user sees why it can't be opened instead of
 *   it silently disappearing. Nothing can be lost there.
 * Closing a project is only the × on its tab.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { RecentProject } from "../../shared/ipc/generated/RecentProject";
import { CloseIcon, TrashIcon } from "../../shared/ui/icons";

type Props = {
  recent: RecentProject[];
  /** Whether a project is open in a tab. */
  isOpen: (dir: string) => boolean;
  onOpen: (dir: string) => void;
  /** Remove a missing project from the list. */
  onForget: (dir: string) => void;
  /** Move a project to the Recycle Bin (already confirmed). */
  onTrash: (dir: string) => void;
};

export function RecentList({ recent, isOpen, onOpen, onForget, onTrash }: Props) {
  const { t, i18n } = useTranslation();
  // The row asking "Move … to the Recycle Bin?", if any.
  const [confirming, setConfirming] = useState<string | null>(null);

  if (recent.length === 0) {
    return <p className="start__recent-empty">{t("start.recentEmpty")}</p>;
  }

  const when = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });

  return (
    <ul className="recent">
      {recent.map((r) => {
        const open = isOpen(r.path);
        return (
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

            {confirming === r.path ? (
              <div className="recent__confirm" role="alertdialog" aria-label={t("recent.trash")}>
                <span>{t("recent.trashConfirm", { name: r.name })}</span>
                <button
                  type="button"
                  className="button button--danger"
                  onClick={() => {
                    setConfirming(null);
                    onTrash(r.path);
                  }}
                >
                  {t("recent.trashYes")}
                </button>
                <button
                  type="button"
                  className="button"
                  autoFocus
                  onClick={() => setConfirming(null)}
                >
                  {t("recent.trashNo")}
                </button>
              </div>
            ) : (
              <>
                <span className="recent__when">
                  {r.exists ? when.format(new Date(r.openedMs)) : t("recent.missing")}
                </span>
                {r.exists ? (
                  <button
                    type="button"
                    className="recent__action"
                    onClick={() => setConfirming(r.path)}
                    disabled={open}
                    title={open ? t("recent.trashOpen") : t("recent.trash")}
                    aria-label={t("recent.trash")}
                  >
                    <TrashIcon />
                  </button>
                ) : (
                  <button
                    type="button"
                    className="recent__action"
                    onClick={() => onForget(r.path)}
                    title={t("recent.forget")}
                    aria-label={t("recent.forget")}
                  >
                    <CloseIcon size={12} />
                  </button>
                )}
              </>
            )}
          </li>
        );
      })}
    </ul>
  );
}
