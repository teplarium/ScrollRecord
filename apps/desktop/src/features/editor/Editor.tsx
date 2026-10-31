/**
 * Editor — what a project tab shows (HANDOFF D-38): site map on the left,
 * canvas in the centre, properties on the right with the device / preview /
 * render / queue row on top. No bottom toolbar.
 *
 * This file assembles the panels: features don't import each other, so the
 * screen that holds them passes data and callbacks between them (like
 * `App.tsx` does for tabs). The tab's state — scenario, undo history, shown
 * device, selection, saving — lives in its project store (`shared/store`),
 * which outlives this component when another tab is shown.
 *
 * Keys (only in this tab, not while typing in a field; `shortcuts.ts`):
 * Ctrl+Z undo, Ctrl+Shift+Z / Ctrl+Y redo, Ctrl+S save now. Delete is
 * handled by the canvas.
 *
 * "Locate site folder…" (moved site, `siteMissing`): the user picks the site's
 * start page, Rust turns its folder into a portable path (`portableSite`) and
 * it becomes an ordinary, undoable edit of `meta.site`.
 */
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../../shared/ipc/commands";
import { pickSiteFolder } from "../../shared/ipc/dialogs";
import type { AppError } from "../../shared/ipc/generated/AppError";
import type { DevicePreset } from "../../shared/ipc/generated/DevicePreset";
import type { OpenedProject } from "../../shared/ipc/generated/OpenedProject";
import { setSite } from "../../shared/store/edits";
import { projectStore, useProject } from "../../shared/store/projectStore";
import { ErrorBanner } from "../../shared/ui/ErrorBanner";
import { Canvas } from "../canvas/Canvas";
import { screenOf } from "../canvas/cardSize";
import { SiteMap } from "../site-map/SiteMap";
import { useSiteMap } from "../site-map/useSiteMap";
import { toProblems, useIssues } from "./problems";
import { PropertiesPanel } from "./PropertiesPanel";
import { editorShortcut } from "./shortcuts";
import { TopActions } from "./TopActions";
import "./Editor.css";

type Props = {
  project: OpenedProject;
};

// Device sizes never change while the app runs: ask Rust once for all tabs.
// Outside the desktop app the list stays empty and cards use a desktop shape.
let presetsRequest: Promise<DevicePreset[]> | null = null;
function loadPresets() {
  presetsRequest ??= commands.devicePresets().then((r) => (r.ok ? r.value : []));
  return presetsRequest;
}

export function Editor({ project }: Props) {
  const { t } = useTranslation();
  const store = projectStore(project);
  const { flow } = useProject(store, (s) => s.history.present);
  const targetId = useProject(store, (s) => s.targetId);
  const selectedId = useProject(store, (s) => s.selectedId);
  const siteMissing = useProject(store, (s) => s.siteMissing);
  const save = useProject(store, (s) => s.save);
  const [presets, setPresets] = useState<DevicePreset[]>([]);
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

  // ── Keyboard: undo / redo / save in this tab ──
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const shortcut = editorShortcut(event);
      if (!shortcut) return;
      event.preventDefault();
      const state = store.getState();
      if (shortcut === "undo") state.undo();
      else if (shortcut === "redo") state.redo();
      else void state.saveNow();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [store]);

  const target = flow.targets.find((tg) => tg.id === targetId) ?? flow.targets[0];
  const screen = useMemo(() => screenOf(target?.viewport, presets), [target, presets]);
  const selected = flow.nodes.find((n) => n.id === selectedId) ?? null;
  // The site map of the shown device; not read while the site folder is missing.
  const siteMap = useSiteMap(project.dir, flow, siteMissing ? null : (target?.id ?? null));

  // ── Problems marked on the canvas (FR-CNV-09) ──
  // The site map counts only when it is the shown device's and up to date.
  const issues = useIssues(flow);
  const { map, stale } = siteMap.state;
  const shownMap = map && !stale && map.target === target?.id ? map : null;
  const problems = useMemo(
    () => toProblems(issues, shownMap, flow, t),
    [issues, shownMap, flow, t],
  );

  async function locateSite() {
    const picked = await pickSiteFolder(t("siteMap.locateTitle"), {
      files: t("dialogs.webPages"),
      all: t("dialogs.allFiles"),
    });
    if (!picked) return;
    setError(null);
    const site = await commands.portableSite(project.dir, picked);
    if (!site.ok) return setError(site.error);
    store.getState().edit((doc) => setSite(doc, site.value));
    store.getState().siteLocated();
  }

  return (
    <main className="editor">
      <SiteMap
        flow={flow}
        siteMissing={siteMissing}
        onLocateSite={() => void locateSite()}
        siteMap={siteMap}
        onAdd={(source) => store.getState().requestAdd(source)}
        onShow={(id) => store.getState().focusNode(id)}
      />
      <section className="editor__canvas" aria-label={t("canvas.title")}>
        {(error ?? save.error) && (
          <div className="editor__error">
            {error && <ErrorBanner error={error} onDismiss={() => setError(null)} />}
            {save.error && (
              <ErrorBanner error={save.error} onDismiss={store.getState().dismissSaveError} />
            )}
          </div>
        )}
        <Canvas store={store} screen={screen} problems={problems} />
        <span className={`editor__save editor__save--${save.status}`} role="status">
          {t(`save.${save.status}`)}
        </span>
      </section>
      <aside className="editor__right" aria-label={t("inspector.title")}>
        <TopActions
          targets={flow.targets}
          current={target?.id ?? null}
          onDevice={(id) => store.getState().setView({ targetId: id })}
        />
        <PropertiesPanel node={selected} />
      </aside>
    </main>
  );
}
