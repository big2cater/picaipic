---
name: router
description: PicAiPic session bootstrap, current state, and context routing hub.
edges:
  - target: context/architecture.md
    condition: when understanding application flow or changing a major subsystem
  - target: context/stack.md
    condition: when working with dependencies, versions, or build tooling
  - target: context/conventions.md
    condition: when writing or reviewing Rust, Vue, IPC, database, or UI code
  - target: context/decisions.md
    condition: when a non-obvious design choice or historical rationale matters
  - target: context/setup.md
    condition: when running, validating, packaging, or troubleshooting the development environment
  - target: context/plugin-runtime.md
    condition: when touching AI plugins, manifests, runtimes, permissions, tasks, packaging, or sandboxing
  - target: patterns/INDEX.md
    condition: before any implementation or diagnosis task
last_updated: 2026-07-23
---



# Session Bootstrap

Read root `AGENTS.md`, then this file, then the routed context and matching pattern before changing code.

## Current Project State

**Working:**
- Tauri 2/Rust desktop host with Vue 3 frontend for Windows and Linux.
- **v1.1.0** app/docs versions aligned; tag `v1.1.0` has a **private draft** multi-arch release (Linux deb/AppImage + Windows x64/arm64 MSI + updater latest JSON on the Release assets). Not published; keep private until the owner decides.
- Release CI publishes installers to **GitHub Release assets** (not Actions artifact storage) after quota failures; PR builds use best-effort artifact upload — see `patterns/release-build.md`.
- Folder-first multi-library browsing, SQLite metadata (schema v6+; collections v7, unique afiles v8), indexing/recovery, thumbnails, timeline/folder/location/camera/lens/tag/favorite/rating/face filters, deduplication, image editing, and broad image/RAW/video support.
- Rename/move disk↔DB consistency: `rename_file` / `rename_folder` roll back disk on DB failure (aligned with `move_file`); `edit_album` propagates name-column errors; dedup `get_files_by_sizes` reuses precomputed suspicious sizes via chunked `IN` binds.
- Local AI search and face processing use bundled ONNX models; FFmpeg is bundled as a sidecar for video workflows.
- AI plugin host: discovery, signed package install/trust, permissions, install profiles, shared/private/external Python runtimes, lifecycle, async tasks, output adopt/discard, runtime-conflict detection, two sample plugins.
- Plugin security A+B+C: bearer token, Ed25519 package signing/trust, default input-file staging; Windows deny-ACL opt-in only.
- Sandbox **Phase 0–2 done**: cross-platform staging, fail-closed, diagnostics, `plugin_writable_roots`, same-volume hardlink→copy. Phase 3–4 (network OS / Landlock) **not** implemented — `docs/ai-plugin-sandbox-roadmap.md`.
- Live Photo / Motion Photo: detect/pair/preview/export; HEIC-internal video; keyframe overwrite (JPEG); album rescan; user guide `docs/guide/live-photo.md`.
- Confirmed shared→plugin-private runtime switch + managed model open/validate/import (Settings).
- Merged to main (2026-07-18): Live Photo polish (#1), sandbox Phase 0–2 + runtime/model UX (#2).
- Built-in **Phase A crop presets** (2026-07-18): ImageEditor ratios + photo-size catalog + custom favorites — `photoSizePresets.ts`, `patterns/change-crop-presets.md`.
- Built-in **Phase B collage** (refined 2026-07-19): multi-select 拼图 — equal + magazine freeform cells (ids 2, 2v, 3a, 3b, 4, 4m, 6, 6m, 9; NeoImaging-style), strip, free canvas + free drafts; host `export_collage` with `template: "cells"` + cell-sized source downscale — `patterns/change-collage.md`.
- Built-in **Phase C1–C2 batch** (2026-07-18): multi-select 批处理 wizard — composable actions including border/expand/watermark/text (+ optional EXIF capture-time stamp); templates; progress/cancel; host `batch_process_images` — `patterns/change-batch-process.md`.
- Dialog safety (2026-07-19): collage draft save/delete + batch template save/delete/overwrite use `MessageBox` / plugin-dialog `ask` only — no `window.prompt`/`window.confirm` (WebView no-op risk). Free collage rotate source headroom uses true AABB.
- Batch process **parallel workers** (2026-07-19): serial dest planning + JoinSet concurrency (2–8); GridView VirtualScroll buffer 4→8. SearchBox submits on Enter only (no per-key SQLite). Face index **CPU parallel** (2026-07-19): 2–4 worker engines + batched SQLite writes; GPU EP still future.
- **Smart Albums / 智能相册** (2026-07-19): rule SQL + sidebar list/editor + Content smart source — `patterns/change-smart-albums.md`. Inserted at absolute sidebar index 1 (`SIDEBAR.SMART`); later panels shifted — always use `SIDEBAR` constants.
- **Collections / 集合** (2026-07-19): SQLite membership + left-sidebar tray + drag-add + Content `collection` query source — `patterns/change-collections.md`.
- Large-library **search perf** (2026-07-19): `cosine_similarity_blob` + chunked `get_files_by_ids`; semantic search defers bulk thumbs to viewport — `patterns/change-library-perf.md`.
- **Library polish Phase 5** (2026-07-19): Library sidebar All/Favorites/On this day quick entries + Content view-adaptive date grouping (`effectiveDateGrouping` → GridView) — `patterns/change-library-shortcuts.md`.
- **Lap 0.3 port (core + UX, 2026-07-19)**: program order perf → 4-pane → collections → smart albums → library polish **done**; bugfix wave (`patterns/fix-library-scan-selection.md`); then UX pack:
  - **AI PNG/JPEG prompt → empty comments**: scan A1111/NovelAI/InvokeAI/ComfyUI PNG text + JPEG UserComment/COM; default on; no full-library backfill — `t_ai_prompt.rs`, `patterns/change-ai-prompt-import.md`
  - **Thumbnail media badges**: format/ISO/shutter/aperture/focal/exposure overlays (default off) — `settings.grid.mediaBadges`, `patterns/change-media-badges.md`
  - **Viewer background modes**: theme/black/white/gray/checker + shortcut **B** — `mediaViewer.backgroundMode`, `patterns/change-viewer-background.md`
  - **AI search file-type + groups**: `ImageSearchParams.search_file_type`; Visual/Similar/Filename section headers — `patterns/change-ai-search-filters.md`
- Built-in **photo print layout / 冲印排版** (refined 2026-07-19): fill-the-paper packing + A4 presets + custom paper form + optional library import; **export** = full plan DPI; **print** = print-sized sheet (long edge ~1800px) + blob URL + `window.print` (fast dialog open); host downscales sources to cell need; print-cache purge + 24h stale temp cleanup — `printLayout.ts`, `PrintLayoutDialog.vue`, `export_print_layout` — `patterns/change-print-layout.md`.
- Dialog open freeze fixed (2026-07-18/19): never mutate pinia `printLayout` inside `computed`.
- **Batch optional library import / G2** (2026-07-20): host `BatchProcessResult.outputPaths`; wizard `batchProcess.importToLibrary`; saveAs → sequential `importFile`; overwrite → `updateFileInfo` only — `patterns/change-batch-process.md`.
- **Signing multi-key + local revoke / G6** (2026-07-20): registry `keys[]` + `revokedKeys`; `trust_publisher` adds keys; `revoke_publisher_key` / `list_revoked_keys`; install fails closed on revoked keys — `docs/ai-plugin-security-hardening.md` Q3 closed.
- **Sandbox Phase 3–5** (2026-07-20): **Phase 3** opt-in Windows netsh outbound + policy env. **Phase 4** opt-in Linux Landlock (ABI probe + path rules + pre_exec; soft-fail). **Phase 5** env hygiene opt-in. Default confinement remains Phase 0–2 — `docs/ai-plugin-sandbox-roadmap.md`, `t_sandbox.rs`.

- **G10 FileInfo Live hover** (2026-07-20): info-panel preview hover/long-press plays Live/Motion; i18n labels — `FileInfo.vue`, `patterns/change-live-photo.md`.
- **G11 print magazine pack** (2026-07-20): free-rect `magazine` strategy + auto scoring — `printLayout.ts`.
- **G12 export-only DPI** (2026-07-20): DPI under Export options; Export DPI copy — `PrintLayoutDialog.vue`.
- **Photo style / 照片格调 + LUT library** (2026-07-21): presets+manual merge; geometry-aware host preview (flip/rotate/crop); decode/JPEG caches; combined color-match+style; crop-aligned compare in editor — `photoStylePresets.ts`, `t_lut.rs`, `t_image.rs`, `patterns/change-photo-style.md`.
- **Traditional color match / 追色 + style LUT** (2026-07-20/21; **perf pack 2026-07-22**): host global Lab stats match (no segmentation); ImageEditor **调色→追色** + batch `colorMatch`; preview `color_match_preview`; **single-image style 33³ `.cube`** via `export_color_match_lut` (reference preferred, else current; not dual-image match map; not G7 SA-LUT). Stats downsample both images to 1024 max-edge; single-pass full-res grade (no multi-plane 50MP buffers); Lab a/b full 0–255; LUT size 17–65 errors instead of silent clamp — `t_color_match.rs`, `patterns/change-color-match.md`.
- **Batch cancel atomic write (2026-07-22):** `{dest}.picaipic-batch.tmp` then rename; cancel cleans temps only; progress `current` clamped to `total` — `t_image.rs`, `patterns/change-batch-process.md`.
- **Audit follow-ups (2026-07-22):** SQLite pool path normalize + max 8 idle; `update_column` allow-lists; face cancel progress→100% + safer model Err; plugin setup log ring 2000 lines — `t_sqlite.rs`, `t_face.rs`, `t_plugin.rs`.
- **Collage/cluster/watermark audit pack (2026-07-22):** collage atomic temp+rename; JPEG turbo scale-on-decode in `load_image_for_layout`; strip max 48; cluster linear top-k + cancel `Err`; watermark ISO datetime + batch watermark/time caches — `t_image.rs`, `t_cluster.rs`, `t_face.rs`.
- **Multi-image compare library entry** (2026-07-21): context menu “Compare with next / selected” → `forceSplit` 2-up viewer — `Content.vue`, `fileMenu.ts`, `patterns/change-compare-viewer.md`.
- **ImageViewer Edit toolbar** (2026-07-21): built-in Edit button; `image.toolbar` plugin buttons no longer rendered (plugins remain on context menu) — `MediaViewer.vue`, `ImageViewer.vue`.
- **Batch capture-time watermark** (2026-07-21): text/image watermark optional EXIF time stamp — `batchProcess.ts`, `BatchProcessDialog.vue`, `t_image.rs`, `patterns/change-batch-process.md`.
- **Photo frame / 相框 G-Frame-1+G2** (2026-07-22; **bug pack + presets same day**): classic white/black + float/sink blur+shadow; host `photo_frame_preview` / `export_photo_frame`; dialog + optional library import. Custom presets in `config.photoFrame.presets`; **frame** default logo = `resources/branding/default-frame-logo.png` (`logo-pic.png` wordmark). **App chrome icons** = neural-cat from **`favicon1.ico`** → `src-tauri/icons/*` (not frame logo). Package: `build-exe.bat` passes `-Clean`; `package_windows.ps1` regenerates icons then `cargo clean -p PicAiPic`. — `photoFrameTemplates.ts`, `PhotoFrameDialog.vue`, `t_image.rs`, `patterns/change-photo-frame.md`, `scripts/regenerate_app_icons.ps1`.
- **Face cluster ANN pack (2026-07-22):** P0 logs + exact/blocked/ANN adaptive + `face.clusterMode`; `instant-distance` HNSW; P3 deferred — `docs/guide/face-cluster-ann-plan.md`, `patterns/change-face-index.md`.
- **LIVE filter bit 8 + AI threshold honor (2026-07-22):** toolbar LIVE; search floors recalibrated to CLIP text→image band **0.30/0.26/0.22/0.18** (was 0.40/0.34/0.28/0.22 — Medium+ emptied text search); no force 0.25 on text search.
- **Smart tags nature/built set (2026-07-22):** CLIP categories `birds` / `plants` / `insects` / `architecture` + short English prompts; smart-tag floor **0.22** (was 0.28/0.3 — too strict, emptied most tags) via `getSmartTagThreshold`; zh/en labels — `smartTags.ts`, `patterns/change-smart-tags.md`.
- **Image-search model tracks (2026-07-23):** **A stop-bleed shipped** (CLIP floors `0.30/0.26/0.22/0.18`, smart-tag `0.22`, histogram logs). **B0 next (design approved, not coded):** default bundled **CLIP B/16 int8** replaces B/32; `app_meta` hard-cut rebuild; multilingual text-only rejected; design `docs/superpowers/specs/2026-07-23-clip-b16-default-bump-design.md`. **Track B deferred:** multilingual SigLIP sideload (CN+EN). Pattern: `patterns/change-image-search-model.md`.
- **Settings hydrate gate + mediaBadges equal-noop (2026-07-22):** `patterns/settings-cross-window-sync.md`; import/updateFileInfo rethrow.
- **G13 system print UX** (2026-07-20): hint that printer/tray is system dialog only; no host `print_file`.
- **Correctness fixes (2026-07-20):**
  - Face clustering is **incremental**: preserves existing person names/assignments; only unassigned faces join clusters or create new `Person N` — `t_cluster.rs`, `Face::get_all_for_clustering` includes `person_id`.
  - Face detector naming/IO aligned to **SCRFD det_500m** with output/anchor mismatch fail-closed — `t_face.rs`, `t_common.rs`.
  - DB storage move/reset clears conn pool after migrate — `t_cmds.rs` (`change_db_storage_dir` / `reset_db_storage_dir`).
  - Collage host rejects source count > cell count (no silent drop) — `t_image.rs`.
  - Batch dialog: hue min **-180**, removed `&& false` dead disable — `BatchProcessDialog.vue`.
  - Dedup skips rows without `file.id` instead of `unwrap` panic — `t_dedup.rs`.
  - **Calendar empty list (root cause):** after Smart Albums was inserted at sidebar index 1, `Content.vue` still treated calendar as index `3` (now Search). Calendar is absolute index `4`. Fixed via `SIDEBAR` constants in `constants.ts` and rewired Content/Home routing. Also: local-day SQL filter, month/day number UI on dots, numeric QueryParams coercion.

**Cancelled / deferred (owner 2026-07-20):**
- **G1** collage-as-batch-action (C3 insert) — cancelled.
- **G7** SA-LUT `export-lut` — cancelled / not doing.
- **G8** face index GPU EP — cancelled / not doing.
- **G9** optional whole-library empty-comment AI-prompt backfill — not doing (scan-time empty-only remains).

**Not yet built / future work:**
- **Large-library face clustering** — plan `docs/guide/face-cluster-ann-plan.md`; **P0–P2 + HNSW ANN done** (`instant-distance`, blocked fallback, `face.clusterMode`). **P3 disk/incremental ANN deferred** (low ROI; measure first; prefer embedding cache over graph serde if needed). Ops: `patterns/change-face-index.md`.
- Sandbox deeper enforcement: Linux netns / real WFP; seccomp; Landlock×ROCm matrix; optional cache ref-range (Phase 3–5 flags exist, default off).
- Remote signing CRL / dual-sign key-transition artifacts; recurring release-exe plugin regression after host changes.
- Broader HEIC sequence sample coverage; broader automated coverage outside plugin-host + current Rust unit tests.
- Publish v1.1.0 draft release (owner decision; repo remains private for now).
- Commit remaining G10–G13 + correctness pack + calendar SIDEBAR fix + color match / photo style / photo frame / face-cluster ANN / LIVE filter / settings sync / favicon1 app icons if not yet on `main` tip.
- In-app updater UX polish deferred until public Release (endpoint already configured).

**Known issues / active risks:**
- `api.js` still maps many **query** IPC failures to `null`/`false`/`[]` (empty-vs-error debt). Mutating paths rethrow for rating/favorite/rotate/batch metadata and for `importFile` / `importUrl` / `updateFileInfo`. Settings cross-window: hydrate-gated emits + equal-noop object setters (`patterns/settings-cross-window-sync.md`).
- Adding a new left-sidebar button **shifts absolute indices** — update `SIDEBAR` in `constants.ts` and every `updateContent` / shortcut branch; never hard-code calendar as `3`.
- Packaged-plugin behavior must be checked in the release executable; dev-mode success alone does not prove installer/resource/runtime correctness.
- GitHub Actions **artifact storage quota** can fail uploads even when builds succeed; prefer Release assets for installers; PR upload is best-effort.
- Release Rust builds can fail at local MSVC/CRT link time in native deps (ONNX/LibRaw) even when `cargo check` passes.
- AI plugin compatibility enforces min/max PicAiPic versions and plugin API major.
- Protocol thumbnail/preview resolve against library id in the URL; preserve isolation.
- pnpm is the sole JS package manager; host/frontend/docs versions aligned at **1.1.0**.
- Historical internal identifiers may still use `Lap`; user-visible paths corrected.
- ffprobe ContentIdentifier key name may vary; `first_exist` checks dotted and underscored variants.

## Routing Table

| Task type | Load |
|-----------|------|
| Understand application flow or subsystem boundaries | `context/architecture.md` |
| Work with libraries, versions, native dependencies, or CI | `context/stack.md` |
| Write/review Rust, Vue, IPC, database, or UI code | `context/conventions.md` |
| Make or revisit a design choice | `context/decisions.md` |
| Set up, run, verify, or package | `context/setup.md` |
| Change AI plugin host, manifest, runtime, task, trust, or sandbox | `context/plugin-runtime.md` |
| Change Live Photo / Motion Photo detection, pairing, or preview | `patterns/change-live-photo.md` |
| Change face indexing performance, clustering, or scan DB batching | `patterns/change-face-index.md` |
| Plan large-library face clustering (ANN / blocked KNN, O(n²) removal) | `docs/guide/face-cluster-ann-plan.md` then `patterns/change-face-index.md` |
| Change calendar dots, day/month selection, or empty calendar content | `patterns/change-calendar.md` |
| Change AI PNG prompt import into comments | `patterns/change-ai-prompt-import.md` |
| Change thumbnail media-info badges | `patterns/change-media-badges.md` |
| Main↔settings Pinia emit/listen, hydrate gate, object equal-noop | `patterns/settings-cross-window-sync.md` |
| Regenerate Windows app icons from favicon1.ico | `scripts/regenerate_app_icons.ps1` then `build-exe.bat` / `package_windows.ps1 -Clean` |
| Change image viewer background modes | `patterns/change-viewer-background.md` |
| Change AI search filters or result grouping | `patterns/change-ai-search-filters.md` |
| Add/tune CLIP smart tags (prompts, per-tag threshold) | `patterns/change-smart-tags.md` |
| Image-search model (B0 CLIP B/16 default bump, later SigLIP sideload) | `patterns/change-image-search-model.md` + `docs/superpowers/specs/2026-07-23-clip-b16-default-bump-design.md` |
| Change traditional color match / 追色 / host style `.cube` | `patterns/change-color-match.md` |
| Plan or implement built-in crop presets, collage, batch, print layout, color match / style LUT, photo style, or photo frame / 相框 | `docs/guide/builtin-tools-roadmap.md` then `patterns/change-crop-presets.md` / `patterns/change-collage.md` / `patterns/change-batch-process.md` / `patterns/change-print-layout.md` / `patterns/change-color-match.md` / `patterns/change-photo-style.md` / `patterns/change-photo-frame.md` |
| Change EXIF photo frame / 相框 (classic, float/sink blur, logo) | `patterns/change-photo-frame.md` + roadmap Phase G |
| Build/release installers or plugin packages | `patterns/release-build.md` |
| Perform any recurring task | `patterns/INDEX.md` |

## Behavioural Contract

1. **CONTEXT** — Read the routed context and matching pattern; use current code/docs as truth if memory conflicts.
2. **BUILD** — Keep changes focused and preserve the non-negotiables in `AGENTS.md`.
3. **VERIFY** — Run every applicable item in `context/conventions.md`; plugin changes also use `scripts/check_plugin_host.ps1`.
4. **DEBUG** — Use the matching debug pattern, reproduce at the narrowest boundary, then rerun verification.
5. **GROW** — Update current state/context/patterns, bump `last_updated`, and record material decisions, risks, or todos with `mex log`.
