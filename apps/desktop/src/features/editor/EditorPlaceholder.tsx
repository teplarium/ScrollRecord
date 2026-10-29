/**
 * EditorPlaceholder — what a project tab shows until the canvas arrives in
 * step 3.2: the project's name, folder, website and video formats.
 *
 * Also the first place that handles a moved site folder (`siteMissing`):
 * "Locate site folder…" lets Rust turn the picked folder into a portable
 * path (`portableSite`), writes it into the scenario and saves it.
 * From step 3.4 the scenario lives in the editor's store and this change
 * becomes one undoable edit.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../../shared/ipc/commands";
import { pickFolder } from "../../shared/ipc/dialogs";
import type { AppError } from "../../shared/ipc/generated/AppError";
import type { OpenedProject } from "../../shared/ipc/generated/OpenedProject";
import { ErrorBanner } from "../../shared/ui/ErrorBanner";
import { WarningIcon } from "../../shared/ui/icons";
import "./EditorPlaceholder.css";

type Props = {
  project: OpenedProject;
  /** The project's data changed (site located and saved). */
  onChange: (project: OpenedProject) => void;
};

export function EditorPlaceholder({ project, onChange }: Props) {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);
  const { flow } = project;

  async function locateSite() {
    const picked = await pickFolder(t("editor.locateTitle"));
    if (!picked) return;
    setError(null);
    const site = await commands.portableSite(project.dir, picked);
    if (!site.ok) return setError(site.error);
    const changed = { ...flow, meta: { ...flow.meta, site: site.value } };
    const saved = await commands.saveProject(project.dir, changed, project.layout);
    if (!saved.ok) return setError(saved.error);
    onChange({ ...project, flow: changed, siteMissing: false });
  }

  return (
    <main className="editor-placeholder">
      <div className="editor-placeholder__card">
        <h1 className="editor-placeholder__name">{project.manifest.name}</h1>

        <dl className="editor-placeholder__facts">
          <dt>{t("editor.folder")}</dt>
          <dd>{project.dir}</dd>
          <dt>{t("editor.site")}</dt>
          <dd>{flow.meta.site ?? t("editor.noSite")}</dd>
          <dt>{t("editor.videos")}</dt>
          <dd>{t("editor.videosValue", { count: flow.targets.length })}</dd>
          <dt>{t("editor.nodes")}</dt>
          <dd>{t("editor.nodesValue", { count: flow.nodes.length })}</dd>
        </dl>

        {project.siteMissing && (
          <div className="editor-placeholder__warning" role="alert">
            <WarningIcon />
            <span>{t("editor.siteMissing", { site: flow.meta.site ?? "" })}</span>
            <button type="button" className="button" onClick={() => void locateSite()}>
              {t("editor.locate")}
            </button>
          </div>
        )}
        {error && <ErrorBanner error={error} onDismiss={() => setError(null)} />}

        <p className="editor-placeholder__next">{t("editor.comingNext")}</p>
      </div>
    </main>
  );
}
