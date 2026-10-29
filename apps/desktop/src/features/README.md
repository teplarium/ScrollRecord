# UI features

One folder per feature area. A feature owns its components, styles, hooks and tests.
Features may import from `shared/` and `i18n/`, but **never from another feature**: shared code moves to `shared/` first.

| Folder           | Status              | What it does                                                                 |
| ---------------- | ------------------- | ---------------------------------------------------------------------------- |
| `projects/`      | Phase 3 (step 3.1b) | Home tab: new project form, open project, recent projects                    |
| `editor/`        | Phase 3             | Project tab layout (D-38); placeholder until the canvas (3.2)                |
| `site-map/`      | Phase 3             | Left panel: pages → blocks → interactive elements, previews (D-38/D-39)      |
| `settings/`      | Phase 0 (language)  | Language, paths, browser and ffmpeg locations                                |
| `canvas/`        | Phase 3             | React Flow canvas: Start/End, block cards, Navigate, arrows (no page boards) |
| `inspector/`     | Phase 3             | Properties panel of the selected node or arrow                               |
| `targets/`       | Phase 3             | Devices, aspect presets, resolutions, formats                                |
| `preview/`       | Phase 3             | Real-time preview (frame stream)                                             |
| `render-queue/`  | Phase 3             | Render queue and progress                                                    |
| `cursor-studio/` | Phase 5             | Cursor packs                                                                 |
| `ai/`            | Phase 5             | Site digest, skill download, import with error report                        |

Create a folder only when its phase starts.
