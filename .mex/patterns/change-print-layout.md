---
name: change-print-layout
description: Runbook for 冲印排版 (fill packing, A4, export vs print-sized print, temp cache, cell-sized decode).
last_updated: 2026-07-20
---

# Change print layout / 冲印排版

## When to use

- Add paper sizes or packing templates
- Change auto-pack math (gaps, mixed bands, utilization, fill-the-paper scaling)
- Change export sheet rendering, **print-sized** print path, guides, or temp cache
- Wire import-to-library after export/print
- Wire new entry points

## Touchpoints

| Area | Path |
|------|------|
| Catalog + pack math | `src-vite/src/common/printLayout.ts` |
| Photo cell sizes | reuses `photoSizePresets.ts` |
| Dialog | `src-vite/src/components/PrintLayoutDialog.vue` |
| Custom paper form | `src-vite/src/components/AddCustomPaperDialog.vue` |
| Entry | multi-select `SelectionPanel` → `Content.openPrintLayoutDialog` |
| Import after export/print | `Content.onPrintLayoutDone` → `importFile` + album refresh |
| IPC | `api.exportPrintLayout`, `tempFilePath`, `deleteTempFile`, `cleanupStaleTempFiles` |
| Host | `t_image::export_print_layout` (cell-sized source downscale + parallel unique decode), `t_utils::temp_file_path` / `delete_temp_file` / `cleanup_stale_temp_files` |
| Config | `configStore.printLayout` (`customPapers`, `customLayouts`, dpi, background, guides, `importToLibrary`) |
| i18n | `print_layout.*`, `info_panel.print_layout` |

## Packing rules

- Local-only; export is save-as sheet image (does not mutate originals).
- Paper/photo sizes are physical inches; **export** raster = inch × DPI for the sheet.
- **Fill packing (光影-style):** cells scale to nearly fill the paper (preserve photo aspect).
- Built-in papers: 3R–8R + A4/A6. Layouts include 1R/2R/ID/passport/wallet mixes + **A4** packs.
- Mixed layouts: `h-bands` / `v-bands` / `magazine` (free-rect) / `auto` (scores H/V/magazine). Slot `count = 0` = max-fit in remaining band / free space.

## Export vs print (critical)

| Path | Canvas | Source decode | Why |
|------|--------|---------------|-----|
| **导出** | Full plan DPI (`paperPxW/H`) | Downscale each source to **largest cell on that full sheet** | Archive / lab quality |
| **打印** | Same paper **aspect**, long edge capped (~**1800px**, `PRINT_MAX_EDGE`) | Same host path: downscale to **cells on the print-sized sheet** | OS print dialog does not need lab-DPI bitmap; waiting on full DPI blocked `window.print` |

- Print flow: print-sized composite → temp JPEG → prefer **blob URL** (avoid asset-protocol re-read) → hidden `.print-only` img → `window.print()`.
- Do **not** reintroduce host `print_file` for layout.
- UI DPI = **export** density only, not OS printer DPI. Lives under collapsible **Export options**; label is Export DPI.
- Print uses system dialog for printer/tray/copies — no in-app device picker and no host `print_file`.

## Temp / print cache cleanup

- Print cache: fingerprint of layout + files + print geometry; revoke blob URL; delete temp file on change/close.
- Only delete under system temp with prefixes `print_layout_*` / `picaipic_*`.
- Shallow fingerprint (`fileAssignmentKey`) — avoid `deep: true` watch thrashing that invalidates cache constantly.
- Background warm ~200ms after layout settles (`schedulePrintPrerender`).
- Cancel/unmount: clear temps + DOM; 24h stale purge on open.

## Freeze / perf gotchas

- Never mutate pinia `printLayout` from `computed` (`ensureConfigOnce` only at setup / explicit actions).
- Host: `load_image_for_layout` + cover/fit downscale — source pixels larger than cell size on the **target canvas** are wasted.
- Do not wait on full export-DPI composite before opening the print dialog.

## Verify

- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual: 冲印排版 opens without freeze; 8×1R fills paper
- Manual: **打印** opens system dialog quickly (print-sized path); second print hits cache
- Manual: **导出** still full DPI; change layout purges print cache/temp
- Manual: close dialog; no pile of `print_layout_*` in temp
