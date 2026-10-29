/**
 * StartScreen — first screen of the app: create / open a project, recent list.
 * Step 3.1a: the project commands exist in Rust (`commands.createProject`
 * etc.), but they need a folder, a name and a site, which the new-project
 * form and the folder picker provide in step 3.1b. Until then the buttons
 * show SR-0002 ("planned") through the same error banner as before.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { IpcResult } from "../../shared/ipc/commands";
import { appError } from "../../shared/ipc/errors";
import type { AppError } from "../../shared/ipc/generated/AppError";
import { ErrorBanner } from "../../shared/ui/ErrorBanner";
import { FolderOpenIcon, PlusIcon } from "../../shared/ui/icons";
import "./StartScreen.css";

/** Placeholder until the step 3.1b form calls the real commands. */
function formPending(): Promise<IpcResult<null>> {
  return Promise.resolve({
    ok: false,
    error: appError("SR-0002", "project form arrives in step 3.1b", {
      feature: "project form",
      phase: "3",
    }),
  });
}

export function StartScreen() {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);

  // Runs an action and shows its error, if any.
  async function run(action: () => Promise<IpcResult<null>>) {
    setError(null);
    const result = await action();
    if (!result.ok) setError(result.error);
  }

  return (
    <main className="start">
      <section className="start__intro">
        <span className="start__badge">{t("start.badge")}</span>
        <h1 className="start__heading">{t("start.heading")}</h1>
        <p className="start__sub">{t("start.subheading")}</p>
      </section>

      <section className="start__actions">
        <button
          type="button"
          className="start__card start__card--primary"
          onClick={() => void run(formPending)}
        >
          <span className="start__card-icon">
            <PlusIcon />
          </span>
          <span className="start__card-title">{t("start.newProject")}</span>
          <span className="start__card-hint">{t("start.newProjectHint")}</span>
        </button>
        <button type="button" className="start__card" onClick={() => void run(formPending)}>
          <span className="start__card-icon">
            <FolderOpenIcon />
          </span>
          <span className="start__card-title">{t("start.openProject")}</span>
          <span className="start__card-hint">{t("start.openProjectHint")}</span>
        </button>
      </section>

      {error && <ErrorBanner error={error} onDismiss={() => setError(null)} />}

      <section className="start__recent">
        <h2 className="start__recent-title">{t("start.recent")}</h2>
        <p className="start__recent-empty">{t("start.recentEmpty")}</p>
      </section>
    </main>
  );
}
