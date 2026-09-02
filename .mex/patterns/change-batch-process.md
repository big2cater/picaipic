---
name: change-batch-process
description: Runbook for the built-in batch wizard and host batch_process_images pipeline.
last_updated: 2026-07-30
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
- **Atomic write + crash cleanup (2026-07-30):** each worker uses `t_output_temp::TrackedOutputTemp` to persist a synced exact-path journal before writing a UUID-namespaced same-directory sidecar, then renames it to the final destination. Guard drop covers success/error/cancel/task abort; startup recovery covers process exit. Never replace this with an export-root wildcard sweep. Progress `current` is clamped with `(completed + in_flight).min(total)`.
- Reuse Phase A photo/ratio presets for crop actions; custom ratios pass `ratio_w`/`ratio_h`.
- C2: `border`/`expand` are pure geometry+fill; `watermark` needs a local image path; `text` loads a system TTF/TTC via `ab_glyph` (no bundled font asset).
- **Optional library import (G2 MVP):** host returns `outputPaths` for successful writes. Wizard checkbox `batchProcess.importToLibrary` (default off). `Content.onBatchDone`: **saveAs** → sequential `importFile` into current album; **overwrite** → `updateFileInfo` only (never re-copy). No album → toast `batch.import_need_album`.
- Hue slider range is **[-180, 180]**; brightness/contrast remain **[-100, 100]**. Do not fold `hue` into the brightness/contrast min branch (dead-code trap).
- Color match in batch still runs full-res grade for output quality, but stats are downsampled in `t_color_match` (see `patterns/change-color-match.md`).
- **Watermark / capture-time (2026-07-22):** watermark source images and EXIF capture-time labels are cached per worker; `read_capture_time_label` uses ISO-safe `format_frame_date_time` (same as photo frame).

## Verify

- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual: multi-select → 批处理 → chain resize+border+text+watermark → save template → output folder → start → cancel mid-run
- Manual: large batch (dozens+) should progress faster than pure serial; mid-run cancel still stops further work
- `cargo test --manifest-path src-tauri/Cargo.toml t_output_temp::tests`
- Manual: cancel mid-run leaves no `.picaipic-batch-<uuid>.tmp` and no half-written new outputs; overwrite does not wipe originals mid-encode
- Manual: saveAs + import checked → outputs appear in current album; overwrite + import → metadata refresh only, no duplicate

## Capture-time stamp (watermark / text)
- Text / image watermark actions accept `includeCaptureTime` + `captureTimeFormat` (`datetime` | `date` | `time`).
- Host reads EXIF DateTimeOriginal (fallback Digitized / DateTime) per source file and stamps with system font.
- Optional text field acts as prefix before the time when both are set.
