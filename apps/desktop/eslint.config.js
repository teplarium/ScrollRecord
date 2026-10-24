/**
 * ESLint config for the desktop UI.
 * Key rule: `react/jsx-no-literals` — no visible text may be typed directly in
 * JSX. Every string must come from the locale files via `t("...")`, so the UI
 * can be translated later without hunting for hard-coded text (HANDOFF R-12).
 */
import js from "@eslint/js";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "src/shared/ipc/generated"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
    plugins: { react, "react-hooks": reactHooks },
    settings: { react: { version: "detect" } },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react/jsx-no-literals": [
        "error",
        { noStrings: true, ignoreProps: true, allowedStrings: ["·", "—", "/", "→"] },
      ],
    },
  },
);
