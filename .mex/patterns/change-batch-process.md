---
name: change-batch-process
description: Runbook for the built-in batch wizard and host batch_process_images pipeline.
last_updated: 2026-07-20
---

# Change batch processing / 批处理 (Phase C1–C2)

## When to use

- Add/edit batch actions (resize, crop, rotate, adjust, filter, …)
- Change wizard steps, output naming, overwrite policy
- Change host queue, progress, or cancel behavior
- Wire new entry points

## Touchpoints

| Area | Path |
|------|------|
| Action model / templates | `src-vite/src/common/batchProcess.ts` |
| Wizard UI | `src-vite/src/components/BatchProcessDialog.vue` |
| Entry | `SelectionPanel.vue` → `Content.vue` (`openBatchDialog`) |
| IPC | `api.js` → `batchProcessImages`, `cancelBatchProcess` |
| Host | `t_image.rs` (`batch_process_images`, action apply), `t_cmds.rs`, `main.rs` |
| Config | `configStore.batchProcess.templates` |
| Crop presets | reuses `photoSizePresets.ts` |
| i18n | top-level `batch.*`, `info_panel.batch` |
| Product plan | `docs/guide/builtin-tools-roadmap.md` § C |

## Rules

- Local-only offline processing; explicit file list (never whole-library by default).
- Default output is **save-as** to a chosen folder; overwrite originals requires confirmation via `ask` from `@tauri-apps/plugin-dialog` (never `window.confirm` — Tauri WebView no-op can skip the guard and overwrite silently).
- Template name input: in-app `MessageBox` with `showInput` (never `window.prompt`). Template delete also uses `ask`.
- Actions are ordered and free-composable (“一键动作” = saved action chain templates).
- Progress via `batch-process-progress` events; cancel is cooperative between files.
- **Host concurrency:** `batch_process_images` plans destinations serially (`resolve_batch_dest_path` + reserved set), then runs decode/process/write on a bounded `JoinSet` (`batch_worker_limit` ≈ 70% cores, clamp 2–8). Do not re-resolve dest inside workers (race on rename). Cancel: stop spawn + `abort_all` + drain joins.
- Reuse Phase A photo/ratio presets for crop actions; custom ratios pass `ratio_w`/`ratio_h`.
- C2: `border`/`expand` are pure geometry+fill; `watermark` needs a local image path; `text` loads a system TTF/TTC via `ab_glyph` (no bundled font asset).
- **Optional library import (G2 MVP):** host returns `outputPaths` for successful writes. Wizard checkbox `batchProcess.importToLibrary` (default off). `Content.onBatchDone`: **saveAs** → sequential `importFile` into current album; **overwrite** → `updateFileInfo` only (never re-copy). No album → toast `batch.import_need_album`.
- Hue slider range is **[-180, 180]**; brightness/contrast remain **[-100, 100]**. Do not fold `hue` into the brightness/contrast min branch (dead-code trap).

## Verify

- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual: multi-select → 批处理 → chain resize+border+text+watermark → save template → output folder → start → cancel mid-run
- Manual: large batch (dozens+) should progress faster than pure serial; mid-run cancel still stops further work
- Manual: saveAs + import checked → outputs appear in current album; overwrite + import → metadata refresh only, no duplicate
