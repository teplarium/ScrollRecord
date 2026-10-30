/**
 * App — the window layout: project tabs, the shown screen, footer.
 * Why: one place decides which screen is visible — the start screen (Home
 * tab) or a project tab (HANDOFF D-38). Features never import each other,
 * so this file wires them together with callbacks.
 * At startup the projects that were open last time come back as tabs (D-39).
 */
import { useEffect, useReducer, useState } from "react";

import { Editor } from "../features/editor/Editor";
import { openFailure, StartScreen, type OpenFailure } from "../features/projects/StartScreen";
import { commands } from "../shared/ipc/commands";
import type { OpenedProject } from "../shared/ipc/generated/OpenedProject";
import { AppFooter } from "./AppFooter";
import { TabBar } from "./TabBar";
import { activeProject, initialTabs, tabsReducer } from "./tabs";
import "./App.css";

export function App() {
  const [tabs, dispatch] = useReducer(tabsReducer, initialTabs);
  // A tab that could not be restored at startup, shown on the start screen
  // (with "Open anyway" if another window holds it).
  const [restoreFailure, setRestoreFailure] = useState<OpenFailure | null>(null);
  // Changing `homeKey` re-mounts the start screen (fresh recent list);
  // `formOnHome` makes it open with the new-project form ("+" button).
  const [homeKey, setHomeKey] = useState(0);
  const [formOnHome, setFormOnHome] = useState(false);

  // ── Restore the tabs of the last session ──
  // Opened one by one so the tab order stays the same. Home stays shown
  // only if nothing could be restored.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const home = await commands.projectHome();
      if (!home.ok || cancelled) return;
      let first = true;
      for (const dir of home.value.openTabs) {
        const opened = await commands.openProject(dir);
        if (cancelled) return;
        if (opened.ok) {
          dispatch({ type: "opened", project: opened.value, activate: first });
          first = false;
        } else {
          setRestoreFailure(openFailure(opened.error, dir));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  function showHome(withForm: boolean) {
    setFormOnHome(withForm);
    setHomeKey((k) => k + 1);
    dispatch({ type: "activate", dir: null });
  }

  function opened(project: OpenedProject) {
    setRestoreFailure(null);
    dispatch({ type: "opened", project, activate: true });
  }

  async function close(dir: string) {
    // Closing never fails in Rust (a stale lock is only logged).
    await commands.closeProject(dir);
    dispatch({ type: "closed", dir });
  }

  const shown = activeProject(tabs);

  return (
    <div className="app">
      <TabBar
        projects={tabs.projects}
        active={tabs.active}
        onHome={() => showHome(false)}
        onSelect={(dir) => dispatch({ type: "activate", dir })}
        onClose={(dir) => void close(dir)}
        onNew={() => showHome(true)}
      />
      {shown ? (
        <Editor
          key={shown.dir}
          project={shown}
          onChange={(project) => dispatch({ type: "updated", project })}
        />
      ) : (
        <StartScreen
          key={homeKey}
          startWithForm={formOnHome}
          notice={restoreFailure}
          onNoticeDismiss={() => setRestoreFailure(null)}
          onOpened={opened}
        />
      )}
      <AppFooter />
    </div>
  );
}
