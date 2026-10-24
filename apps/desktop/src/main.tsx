/**
 * Entry point of the desktop UI.
 * Why: mounts React once and loads i18n *before* the first render, so no
 * screen ever flashes raw translation keys.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./i18n";
import "./styles/global.css";
import { App } from "./app/App";

const rootElement = document.getElementById("root");

if (rootElement) {
  createRoot(rootElement).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
