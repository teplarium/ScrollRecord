/**
 * StartScreen — first screen of the app: create / open a project, recent list.
 * Phase 0: the buttons call real IPC commands that answer SR-0002
 * ("planned for phase 3"). That proves the whole chain works end to end:
 * click → Rust command → typed AppError → translated banner.
 * Phase 3 replaces the command bodies; this component barely changes.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { commands, type IpcResult } from "../../shared/ipc/commands";
import type { AppError } from "../../shared/ipc/generated/AppError";
import { ErrorBanner } from "../../shared/ui/ErrorBanner";
import { FolderOpenIcon, PlusIcon } from "../../shared/ui/icons";
import "./StartScreen.css";

export function StartScreen() {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);

  // Runs a command and shows its error, if any. Success handling arrives in Phase 3.
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
          onClick={() => void run(commands.createProject)}
        >
          <span className="start__card-icon">
            <PlusIcon />
          </span>
          <span className="start__card-title">{t("start.newProject")}</span>
          <span className="start__card-hint">{t("start.newProjectHint")}</span>
        </button>
        <button
          type="button"
          className="start__card"
          onClick={() => void run(commands.openProject)}
        >
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
