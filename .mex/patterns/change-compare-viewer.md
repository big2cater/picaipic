---
name: change-compare-viewer
description: Multi-pane image comparison in ImageViewer (1/2/4 panes, viewport sync).
last_updated: 2026-07-19
---

# Change compare / multi-pane viewer

## When to use

- Extend 2-pane or 4-pane comparison
- Change viewport sync, active-pane navigation, or split toolbar cycle

## Touchpoints

| Area | Path |
|------|------|
| Viewer window | `src-vite/src/views/ImageViewer.vue` |
| Toolbar | `MediaViewer.vue` (`cycle-split`, `splitCount`) |
| Main → viewer IPC | `Content.vue` `request-file-at-index` / `update-img` (pane: left/right/bottomLeft/bottomRight) |
| Prefs | `configStore.imageViewer.splitCount` + legacy `isSplit` |

## Rules

- `splitCount` is source of truth: **1 | 2 | 4**. `isSplit` is computed as `splitCount > 1`.
- Toolbar cycles **1 → 2 → 4 → 1** via `cycle-split`.
- Each pane has independent file index; active pane receives nav/rating/favorite.
- Viewport sync fans out to all **visible** panes when media types match (all image or all video).
- Content must pass through arbitrary `pane` and hydrate placeholders before emitting file ids for extra panes.
- `compareMode` sessions force 2-up + sync without permanently clobbering user `splitCount` prefs on reset.

## Verify

- `pnpm --dir src-vite build`
- Manual: open viewer → cycle to 2-up and 4-up → Tab cycles active pane → prev/next only moves active → sync toggle locks zoom/pan
