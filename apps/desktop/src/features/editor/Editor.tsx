/**
 * Editor — what a project tab shows (HANDOFF D-38): site map on the left,
 * canvas in the centre, properties on the right with the device / preview /
 * render / queue row on top. No bottom toolbar.
 *
 * This file assembles the panels: features don't import each other, so the
 * screen that holds them passes data and callbacks between them (like
 * `App.tsx` does for tabs). View state that is never saved — the shown device
 * and the selected shape — lives here until the per-tab stores of step 3.4.
 *
 * "Locate site folder…" (moved site, `siteMissing`): the user picks the site's
 * start page, Rust turns its folder into a portable path (`portableSite`), it
 * goes into the scenario and is saved. From step 3.4 this becomes one undoable edit in the store.
 */
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../../shared/ipc/commands";
import { pickSiteFolder } from "../../shared/ipc/dialogs";
import type { AppError } from "../../shared/ipc/generated/AppError";
import type { DevicePreset } from "../../shared/ipc/generated/DevicePreset";
import type { OpenedProject } from "../../shared/ipc/generated/OpenedProject";
import { ErrorBanner } from "../../shared/ui/ErrorBanner";
import { Canvas } from "../canvas/Canvas";
import { screenOf } from "../canvas/cardSize";
import { SiteMap } from "../site-map/SiteMap";
import { useSiteMap } from "../site-map/useSiteMap";
import { PropertiesPanel } from "./PropertiesPanel";
import { TopActions } from "./TopActions";
import "./Editor.css";

type Props = {
  project: OpenedProject;
  /** The project's data changed (site located and saved). */
  onChange: (project: OpenedProject) => void;
};

// Device sizes never change while the app runs: ask Rust once for all tabs.
// Outside the desktop app the list stays empty and cards use a desktop shape.
let presetsRequest: Promise<DevicePreset[]> | null = null;
function loadPresets() {
  presetsRequest ??= commands.devicePresets().then((r) => (r.ok ? r.value : []));
  return presetsRequest;
}

export function Editor({ project, onChange }: Props) {
  const { t } = useTranslation();
  const { flow } = project;
  const [presets, setPresets] = useState<DevicePreset[]>([]);
  const [targetId, setTargetId] = useState<string | null>(flow.targets[0]?.id ?? null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    let cancelled = false;
    void loadPresets().then((list) => {
      if (!cancelled) setPresets(list);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const target = flow.targets.find((tg) => tg.id === targetId) ?? flow.targets[0];
  const screen = useMemo(() => screenOf(target?.viewport, presets), [target, presets]);
  const selected = flow.nodes.find((n) => n.id === selectedId) ?? null;
  // The site map of the shown device; not read while the site folder is missing.
  const siteMap = useSiteMap(project.dir, flow, project.siteMissing ? null : (target?.id ?? null));

  async function locateSite() {
    const picked = await pickSiteFolder(t("siteMap.locateTitle"), {
      files: t("dialogs.webPages"),
      all: t("dialogs.allFiles"),
    });
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
    <main className="editor">
      <SiteMap
        flow={flow}
        siteMissing={project.siteMissing}
        onLocateSite={() => void locateSite()}
        siteMap={siteMap}
      />
      <section className="editor__canvas" aria-label={t("canvas.title")}>
        {error && (
          <div className="editor__error">
            <ErrorBanner error={error} onDismiss={() => setError(null)} />
          </div>
        )}
        <Canvas flow={flow} layout={project.layout} screen={screen} onSelect={setSelectedId} />
      </section>
      <aside className="editor__right" aria-label={t("inspector.title")}>
        <TopActions targets={flow.targets} current={target?.id ?? null} onDevice={setTargetId} />
        <PropertiesPanel node={selected} />
      </aside>
    </main>
  );
}
