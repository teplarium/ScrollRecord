/**
 * StartScreen — the Home tab: new project, open project, recent projects
 * (HANDOFF FR-PRJ-01, FR-PRJ-03, D-37, D-39).
 *
 * Talks to Rust only through `shared/ipc` and reports an opened project to
 * `App` with `onOpened`, which turns it into a tab. Re-mounted every time
 * Home is shown, so the recent list is always fresh.
 * "Already open elsewhere" (SR-1005) gets an "Open anyway" button: the lock
 * is a warning, and after a crash it is the only way back in.
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../../shared/ipc/commands";
import { pickProjectFolder } from "../../shared/ipc/dialogs";
import type { AppError } from "../../shared/ipc/generated/AppError";
import type { OpenedProject } from "../../shared/ipc/generated/OpenedProject";
import type { ProjectHome } from "../../shared/ipc/generated/ProjectHome";
import { ErrorBanner } from "../../shared/ui/ErrorBanner";
import { FolderOpenIcon, PlusIcon } from "../../shared/ui/icons";
import { NewProjectForm } from "./NewProjectForm";
import { RecentList } from "./RecentList";
import "./StartScreen.css";

type Props = {
  /** Open with the new-project form (the "+" tab button). */
  startWithForm: boolean;
  /**
   * A failure from outside this screen, e.g. a tab that could not be
   * restored at startup. It may arrive after the screen is shown, so it is
   * a prop (not initial state); `App` clears it when a project opens.
   */
  notice: OpenFailure | null;
  onNoticeDismiss: () => void;
  /** A project was created or opened: show it as a tab. */
  onOpened: (project: OpenedProject) => void;
  /** Whether a project folder is open in a tab (it can't be deleted then). */
  isOpen: (dir: string) => boolean;
};

/** A failure, and the folder to retry with "Open anyway" for SR-1005. */
export type OpenFailure = { error: AppError; lockedDir?: string };

/** Wraps an open error; SR-1005 remembers the folder for "Open anyway". */
export function openFailure(error: AppError, dir: string): OpenFailure {
  return error.code === "SR-1005" ? { error, lockedDir: dir } : { error };
}

export function StartScreen({ startWithForm, notice, onNoticeDismiss, onOpened, isOpen }: Props) {
  const { t } = useTranslation();
  const [home, setHome] = useState<ProjectHome | null>(null);
  const [showForm, setShowForm] = useState(startWithForm);
  const [failure, setFailure] = useState<OpenFailure | null>(null);
  // This screen's own failure wins over an older notice from App.
  const shown = failure ?? notice;

  useEffect(() => {
    let cancelled = false;
    void commands.projectHome().then((result) => {
      if (cancelled) return;
      if (result.ok) setHome(result.value);
      else setFailure({ error: result.error });
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // ── Opening ──
  // Every way of opening ends here, so SR-1005 handling exists once.
  async function open(dir: string, force = false) {
    setFailure(null);
    const result = await commands.openProject(dir, force);
    if (result.ok) onOpened(result.value);
    else setFailure(openFailure(result.error, dir));
  }

  async function chooseAndOpen() {
    const dir = await pickProjectFolder(
      t("start.openDialogTitle"),
      { files: t("dialogs.projects"), all: t("dialogs.allFiles") },
      home?.suggestedLocation,
    );
    if (dir) await open(dir);
  }

  async function trash(dir: string) {
    setFailure(null);
    const result = await commands.trashProject(dir);
    if (!result.ok) setFailure({ error: result.error });
    const refreshed = await commands.projectHome();
    if (refreshed.ok) setHome(refreshed.value);
  }

  async function forget(dir: string) {
    await commands.forgetRecent(dir);
    const result = await commands.projectHome();
    if (result.ok) setHome(result.value);
  }

  return (
    <main className="start-screen">
      <div className="start">
        <section className="start__intro">
          <span className="start__badge">{t("start.badge")}</span>
          <h1 className="start__heading">{t("start.heading")}</h1>
          <p className="start__sub">{t("start.subheading")}</p>
        </section>

        {showForm && home ? (
          <NewProjectForm
            suggestedLocation={home.suggestedLocation}
            onCreated={onOpened}
            onCancel={() => setShowForm(false)}
          />
        ) : (
          <section className="start__actions">
            <button
              type="button"
              className="start__card start__card--primary"
              onClick={() => setShowForm(true)}
              disabled={!home}
            >
              <span className="start__card-icon">
                <PlusIcon />
              </span>
              <span className="start__card-title">{t("start.newProject")}</span>
              <span className="start__card-hint">{t("start.newProjectHint")}</span>
            </button>
            <button type="button" className="start__card" onClick={() => void chooseAndOpen()}>
              <span className="start__card-icon">
                <FolderOpenIcon />
              </span>
              <span className="start__card-title">{t("start.openProject")}</span>
              <span className="start__card-hint">{t("start.openProjectHint")}</span>
            </button>
          </section>
        )}

        {shown && (
          <div className="start__failure">
            <ErrorBanner
              error={shown.error}
              onDismiss={() => (failure ? setFailure(null) : onNoticeDismiss())}
            />
            {shown.lockedDir && (
              <button
                type="button"
                className="button"
                onClick={() => void open(shown.lockedDir ?? "", true)}
              >
                {t("start.openAnyway")}
              </button>
            )}
          </div>
        )}

        <section className="start__recent">
          <h2 className="start__recent-title">{t("start.recent")}</h2>
          <RecentList
            recent={home?.recent ?? []}
            onOpen={(dir) => void open(dir)}
            isOpen={isOpen}
            onForget={(dir) => void forget(dir)}
            onTrash={(dir) => void trash(dir)}
          />
        </section>
      </div>
    </main>
  );
}
