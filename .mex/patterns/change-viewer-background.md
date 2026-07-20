---
name: change-viewer-background
description: Image/quick viewer canvas background modes and cycle shortcut.
edges:
  - target: change-compare-viewer.md
    condition: when multi-pane viewer chrome interacts with canvas styling
  - target: ../ROUTER.md
    condition: after shipping viewer chrome behavior
last_updated: 2026-07-20
---

# Change viewer background

## When to use
- Add/change viewer canvas backgrounds (theme/black/white/gray/checker)
- Change cycle shortcut or toolbar control
- Adjust where background is painted (standalone viewer vs quick view)

## Key files
| Layer | Path |
|-------|------|
| State | `configStore.mediaViewer.backgroundMode` (0–4) |
| Helpers | `utils.ts` `normalize/cycle/getViewerBackgroundClass` |
| Style | `assets/app.css` `viewer-bg-checker` |
| Render | `MediaViewer.vue` media area class + palette toolbar button |
| Shortcut | `shortcuts.ts` `view.cycleBackground` (B) |
| Handlers | `ImageViewer.vue`, `Content.vue` (quick view / filmstrip) |
| Settings | `Settings.vue` Viewer tab select + shortcuts list |
| i18n | `settings.image_view.background*`, `image_viewer.toolbar.cycle_background` |

## Behaviour contract
1. Modes: `0` theme, `1` black, `2` white, `3` gray, `4` checker.
2. Default `0` (theme) — no visual change for existing users.
3. Background applies to the **media canvas only**, not global app chrome.
4. Shortcut **B** cycles modes in standalone viewer and in-app preview when a media viewer is active.
5. Toolbar palette button also cycles; selected when mode ≠ theme.
6. Setting is app-wide (Pinia persist) and synced across windows.

## Verify
```bash
pnpm --dir src-vite build
```
Manual: open image → press B → background cycles; Settings → Viewer → change select; reopen viewer keeps mode.
