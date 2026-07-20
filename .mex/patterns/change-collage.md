---
name: change-collage
description: Runbook for template/strip/free/magazine collage (拼图) UI and host export with cell-sized source decode.
last_updated: 2026-07-19
---

# Change collage / 拼图 (Phase B1–B3 + magazine cells + free drafts)

## When to use

- Add/edit equal grids, magazine freeform cells, strip layouts, or free-canvas behavior
- Change fill mode, radius, stroke, gap/margin export
- Change host composite path (`export_collage` grid / `cells` / free items / rotate)
- Wire additional entry points

## Touchpoints

| Area | Path |
|------|------|
| Templates / free helpers / magazine cells | `src-vite/src/common/collageTemplates.ts` |
| Dialog + free canvas | `src-vite/src/components/CollageDialog.vue` |
| Entry | `SelectionPanel.vue` → `Content.vue` (`openCollageDialog`) |
| IPC | `api.js` → `exportCollage` |
| Host | `t_image.rs` (`export_collage`, `export_collage_cells`, free items, rotate, cell-sized decode), `t_cmds.rs`, `main.rs` |
| i18n | top-level `collage.*` + `info_panel.collage` |
| Product plan | `docs/guide/builtin-tools-roadmap.md` § B |
| Reference assets | 光影魔术手 `Program/JigSaw/PatternJigsaw/Template/*/config.xml` (normalized cells) |

## Rules

- Local-only; never upload sources.
- **Save-as only** — do not overwrite library originals or auto-import export.
- Preview may use thumbnails; full export uses host decode (HEIC/RAW via preview path).
- **Source downscale to cell need:** for grid/cells/free, decode with `load_image_for_layout(path, max_edge)` and `downscale_image_for_fit_cells` so sources larger than the on-canvas cell are reduced before composite (same idea as print layout). Output canvas size is unchanged.
- Templates: equal + magazine freeform cells (`2`, `2v`, `3a`, `3b`, `4`, `4m`, `6`, `6m`, `9`) via normalized `cells[]` in `collageTemplates.ts`. Strips: `strip-h` / `strip-v`, max 12 cells.
- Host freeform export: `template: "cells"` + `cells: [{x,y,w,h}]` (normalized 0–1). Free canvas: `template: "free"` + `items[]`.
- Free mode: items use normalized 0–1 geometry; max 20 items; z-order draw; optional snap.
- Fill: `cover` or `contain`. Radius/stroke applied per cell/item in export space.
- Free drafts: pinia `config.collage.freeDrafts` (app-wide, max 20 via `COLLAGE_FREE_DRAFT_LIMIT`). Save/load/delete in free toolbar. Load keeps only paths present in current multi-selection (partial restore OK).
- **Never use `window.prompt` / `window.confirm`** in Tauri UI — WebView (esp. WebKit) may no-op and return null/false/undefined. Draft rename uses in-app `MessageBox` (showInput); delete uses `@tauri-apps/plugin-dialog` `ask`.
- Free export empty-guard: `exportDisabled` + `doExport` both check `freeItems.length === 0` before host invoke.
- Free rotate source budget: host `rotated_box_source_need` uses true AABB (`|w·cos|+|h·sin|`) so ~45° does not undersample (was fixed 1.15×).

## Verify

- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual: multi-select → 拼图 → magazine 3a/4m preview is non-equal grid → export
- Manual: free mode drag/resize/rotate/reorder → save draft → load → export
- Manual: large originals export without multi-second freezes (cell downscale)
