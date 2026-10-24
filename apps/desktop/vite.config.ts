/// <reference types="vitest/config" />
/**
 * Vite config for the desktop UI (also used by Vitest).
 * Why these settings: Tauri expects the dev server on a fixed port (1420,
 * see src-tauri/tauri.conf.json → build.devUrl) and must not see Rust files
 * change, otherwise every `cargo` rebuild would reload the UI.
 */
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "es2022",
    sourcemap: true,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
