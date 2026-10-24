# UI features

One folder per feature area. A feature owns its components, styles, hooks and tests.
Features may import from `shared/` and `i18n/`, but **never from another feature**: shared code moves to `shared/` first.

| Folder           | Status                 | What it does                                                                    |
| ---------------- | ---------------------- | ------------------------------------------------------------------------------- |
| `projects/`      | Phase 0 (start screen) | Start screen, new/open project, recent projects, site source picker             |
| `settings/`      | Phase 0 (language)     | Language, paths, browser and ffmpeg locations                                   |
| `canvas/`        | Phase 3                | React Flow canvas: page boards, block cards, arrows, inspector mode, live block |
| `inspector/`     | Phase 3                | Properties panel of the selected node or arrow                                  |
| `targets/`       | Phase 3                | Devices, aspect presets, resolutions, formats                                   |
| `preview/`       | Phase 3                | Real-time preview (frame stream)                                                |
| `render-queue/`  | Phase 3                | Render queue and progress                                                       |
| `cursor-studio/` | Phase 5                | Cursor packs                                                                    |
| `ai/`            | Phase 5                | Site digest, skill download, import with error report                           |

Create a folder only when its phase starts.
