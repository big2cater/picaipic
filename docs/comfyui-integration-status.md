# PicAiPic ComfyUI Integration - Current Status

Date: 2026-09-02

## Overview

ComfyUI integration connects a user-managed ComfyUI server (desktop build, 一键包, or a
remote box) to the library: import a workflow, run it against selected photos, and adopt the
results back into the album. PicAiPic only talks HTTP to whatever address the user points it
at; it never hosts the server process.

This document tracks what exists, what is broken, and what is planned.

## 2026-09-02 Import, conversion, run, batch, cancel

### Import

- **Pick a `.json` file directly**: the import dialog previously accepted pasted text only.
  Reads through the browser File API rather than Tauri `plugin-fs`, because `fs:scope` only
  covers `$HOME/**`, `$DOWNLOAD/**`, `$DESKTOP/**` and friends — a workflow kept on another
  drive would be rejected by the plugin.
- **UI-format conversion**: a pasted/selected UI-format graph (the canvas format with
  `nodes` / `links` / positional `widgets_values`) is detected and offered as
  "convert and import", producing the API format that `/prompt` accepts.
- **Loader detection relaxed**: was `class_type === 'LoadImage'`, now matches on shape
  (`inputs.image` is a string). Custom loaders such as `LoadImageAutoMP` work.

### Run

- **Pipeline**: upload image → overwrite the loader's `image` field → submit → poll →
  download → `importFile` back into the album.
- **Entry points**: right-click a single photo and multi-select. **These are two separate
  menus** (see Pitfalls) and both were wired.
- **Batch**: strictly serial, one image at a time.
- **Cancel**: new `comfy_cancel_run` records the prompt id and calls ComfyUI `/interrupt`;
  the polling loop observes the flag and returns `cancelled`. Images finished before the
  cancel are **kept**.
- **Cooldown**: configurable pause between images (default 2s, `config.comfy.cooldownSecs`)
  so ComfyUI can release VRAM before the next run claims it.

### Server maintenance

- **Unload models**: Settings button calling ComfyUI `POST /free` with `unload_models` and
  `free_memory`.

## Results handling

- **Readable import names (2026-09-02):** outputs no longer land in the album as
  `picaipic_<uuid>.png`. Each result is imported as `comfy_<workflow name>_<n>.<ext>`
  (e.g. `comfy_放大 2x_3.png`). The stem is derived from the workflow name and sanitized on
  both sides: the frontend strips illegal filename characters, and the backend
  `sanitize_import_name` (`t_utils.rs`) is the authority — it keeps only the final path
  component (so a name can never escape the album folder), replaces illegal characters,
  and tames Windows reserved names (`CON`, `NUL`, `COM1`…). Collisions get the usual
  `(1)` suffix via `get_unique_path`. The staging temp file keeps the `picaipic_` prefix,
  so `delete_temp_file`/`cleanup_stale_temp_files` still work.
- **Auto-locate after run (2026-09-02):** when a run finishes, the first imported result
  is scrolled into view and selected. `Content.vue` reuses the dedup row-resolution
  pattern (`resolveFileIndexInCurrentQuery`: search the loaded list, else
  `get_query_file_position` on the current query, prefetch the surrounding chunk, then set
  `selectedItemIndex` — GridView scrolls to it). It waits for the content reload
  (`updateContent` starts the list reload without awaiting it) and only jumps while the
  user is still on the same album. The same `revealImportedFile` helper is also called by
  the other new-file producers (batch saveAs, photo frame, print layout, plugin results
  from both the grid dialog and the viewer), so every tool that lands a new image in the
  library brings it into view. (ImageEditor "save as new" already scrolled+selected via
  `insertIndexedFileIntoList`; collage and live-photo export never create library files.)

## Architecture

### Backend commands (`src-tauri/src/t_comfy.rs`)

| Command | Purpose |
|---|---|
| `comfy_test_connection` | reachability probe, reports version/device |
| `comfy_object_info` | fetches `/object_info`, **distilled to widget ordering** — the full response is megabytes, reduced to a few KB before crossing IPC |
| `comfy_upload_image` | multipart upload into ComfyUI's input dir, returns `{name, subfolder, image_type}` |
| `comfy_run_workflow` | submits `/prompt` and polls `/history` until done (blocking, 30 min cap). Takes an optional `prompt_id` so the caller can cancel before it returns |
| `comfy_cancel_run` | records the cancel flag and calls `/interrupt` |
| `comfy_free_memory` | `POST /free` |
| `comfy_download_output` | downloads one output image to a given path |

### Key files

| File | Role |
|---|---|
| `common/comfyConvert.js` | UI → API conversion |
| `components/ComfyWorkflowDialog.vue` | import, validation, conversion entry |
| `components/ComfyRunDialog.vue` | run, batch, cancel, cooldown |
| `common/fileMenu.ts` | **single-selection** context menu definition |
| `components/Content.vue` | multi-selection menu + action dispatch |

## Current problems

### 1. UI-format misdetection (fixed, verified 2026-09-02)

An official ComfyUI `Save (API Format)` export was misclassified as UI format and run
through the converter a second time, shifting every widget value.

Symptoms: ComfyUI answers `node_errors` such as `cfg: 959948902156062.0`, `tile_size: 8`,
`scale_method: "8"` — array elements landing on the wrong named inputs.

Cause: detection keyed only on the presence of a `nodes` or `links` array. Several ComfyUI
versions keep a `nodes` key in the API export, so it matched.

Fix, in two parts:
- `isUiWorkflow` now requires a node to carry `widgets_values` (unique to the UI format; the
  API format never has it).
- The import validator now runs every accepted document through `normalizeApiWorkflow`
  (`common/comfyConvert.js`): an API export that keeps a `nodes` array — whose entries carry
  `class_type`/`inputs` but no `widgets_values` — is rebuilt into the clean keyed form. This
  closes both failure modes of the fix alone: the file was no longer corrupted by a second
  conversion, but it would have been rejected as `not_api_format` (the stray `nodes` entry
  looked malformed), and if it had passed, the raw `nodes` key would have been sent to
  `/prompt` and rejected there. Stray `nodes`/`links` keys never reach the server.

Verified with shape fixtures: plain keyed API format, API format + stray `nodes` (rebuilds
to the identical keyed form), real UI graph (still offered conversion), UI graph with no
`widgets_values` anywhere (rejected as non-API, never corrupted), and garbage input.

Diagnostic value: the workflow runs fine inside ComfyUI, so the file itself is good — the
corruption happened on the PicAiPic side.

### 2. Import appends instead of replacing (fixed 2026-09-02)

Saving a workflow with a name that already exists creates a second entry. The picker
defaults to the first, so re-importing a corrected file can still run the stale data.

Fix: `saveComfyWorkflow` (Settings.vue) now upserts by name — the existing entry keeps its
id and gets the new workflow, so the picker's first-entry default still points at the
corrected data. The import dialog also shows a "saving will overwrite" hint when the typed
name matches a saved workflow.

## Plan

1. ✅ **Verify the UI-format detection fix** — done: detection requires `widgets_values`, and
   API exports that keep a `nodes` key are rebuilt by `normalizeApiWorkflow` instead of being
   rejected or corrupted. Still worth one end-to-end pass against the real server with the
   original offending file.
2. ✅ **Make import replace by name** — done: saving with an existing name replaces in place
   (id kept), with an overwrite hint in the dialog.
3. **(Optional) post-conversion value validation** — check each value against the
   `min`/`max`/enum from `/object_info` and report the offending node and field immediately,
   instead of surfacing a wall of ComfyUI JSON. Note this only **exposes** misalignment
   faster; it cannot fix the misalignment itself.

## Known limitations

- **Conversion is not lossless for custom nodes.** It relies on widget ordering from
  `/object_info`. If a node pack registers widgets on the frontend only (the server does not
  know them), every value shifts by one. Workflows using such nodes should be imported via
  ComfyUI's own `Save (API Format)` and skip the converter entirely.
- **Cancel cannot abort a step already on the GPU.** `comfy_run_workflow` blocks and the
  cancel is observed at the poll interval (700 ms), so worst-case latency is ~0.7 s.
- **`comfy_run_workflow` blocks up to 30 minutes** (`RUN_TIMEOUT`).
- **Results import into the currently open album.** Outside an album (smart album, search
  results) the action refuses up front.
- **Only image outputs are collected.** `collect_output_images` reads the `images` array of
  each node output; animated/video outputs (e.g. `SaveAnimatedWEBP`, `VHS_VideoCombine`)
  appear under a different key and are not downloaded. Animated-webp workflows are not yet
  supported end to end.

## 2026-09-02 review sweep

- **Download intermediate now survives cleanup rules (fixed).** `comfy_download_output`
  staged bytes as `.{name}.picaipic-comfy-<uuid>.tmp` — a leading dot put it outside
  `is_allowed_temp_basename`, so an interrupted download leaked permanently (the stale-temp
  sweep never saw it). The intermediate is now `picaipic-comfy-<uuid>.tmp` (sweepable) and
  is also removed when the final rename fails.
- **`/prompt` non-2xx answers now carry the status.** A rejected submission (400 with an
  error body) previously surfaced as "invalid /prompt response" if the body was not JSON;
  it now reports `HTTP <status>` plus the response text, and `node_errors` handling is
  unchanged for the 200-with-errors case.
- **Editor save flush bug (fixed elsewhere, verified here).** Every `edit_image` save was
  failing on Windows at the temp-file flush step (read-only handle + `FlushFileBuffers`
  rejects it with access denied) — see `.mex/ROUTER.md` 2026-09-02 notes. Save-as-new and
  overwrite now pass hermetic pipeline tests, and PNG/WebP sources converting to JPEG also
  pass (metadata copy is tolerant of EXIF-less sources).

## Pitfalls

- **Temp files must use the `picaipic` prefix.** `delete_temp_file` accepts only
  `print_layout_`, `picaipic_`, `picaipic-` (`t_utils.rs:1882`), and
  `cleanup_stale_temp_files` sweeps only those. A bespoke prefix leaks every output image
  into the temp directory permanently.
- **There are two context menus.** Single selection goes through `common/fileMenu.ts`;
  multi-selection through `Content.vue`'s `selectionMenuItems`, which only opens when
  `selectMode` is true. A new menu entry must be added in both places.
- **A `v-if`-mounted dialog still needs its `show` prop.** Mounting with `v-if="x.show"`
  while the child declares `show` as required and its root is `v-if="show"` means a missing
  prop renders nothing at all. Vue logs `Missing required prop` — check the console before
  theorising.
- **Dev command is `cargo tauri dev`**, not `pnpm dev`. The repo root has no `package.json`;
  the frontend lives in `src-vite/`. See `AGENTS.md`.

## Verification

```powershell
pnpm --dir src-vite build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
