/**
 * App — the window layout: header, current screen, footer.
 * Why: one place decides which screen is visible. Phase 0 has only the start
 * screen; Phase 3 adds the editor and switches between them here.
 */
import { useTranslation } from "react-i18next";

import { StartScreen } from "../features/projects/StartScreen";
import { AppFooter } from "./AppFooter";
import "./App.css";

export function App() {
  const { t } = useTranslation();

  return (
    <div className="app">
      <header className="app__header">
        <span className="app__rec" aria-hidden="true" />
        <span className="app__brand">{t("app.name")}</span>
        <span className="app__tagline">{t("app.tagline")}</span>
      </header>
      <StartScreen />
      <AppFooter />
    </div>
  );
}
