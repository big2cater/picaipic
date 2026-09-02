# Pattern Index

Lookup table for PicAiPic task-specific runbooks. Read the matching pattern before implementation or diagnosis.

_Last reviewed: 2026-08-29 (export-overwrite staged backup, move/rename consistency guards, Replace→trash, staged-delete crash journal, fail-closed plugin auth, copy-first input staging, and storage/backup temp cleanup documented; audit fixes retained)._

| Pattern | Use when |
|---------|----------|
| [add-tauri-command.md](add-tauri-command.md) | Adding or changing frontend-to-Rust IPC, events, or cancellation |
| [change-comfy-integration.md](change-comfy-integration.md) | ComfyUI workflow import/UI→API conversion, run/batch/cancel pipeline, result import, or format misdetection |
| [change-destructive-file-ops.md](change-destructive-file-ops.md) | Move/replace rollback, trash, permanent delete, or batch delete consistency |
| [change-dedup-scan.md](change-dedup-scan.md) | Exact/similar dedup hashing, group rebuilds, cancellation, or SQLite lock contention |
| [change-black-hole-theme.md](change-black-hole-theme.md) | Black-hole theme, native maximize gate, GPU-limited PhotoVortex, CSS spiral fallback, intensity |
| [change-cyberpunk-theme.md](change-cyberpunk-theme.md) | Cyberpunk theme, native maximize gate, GPU-limited PhotoGlitchLayer, CSS glitch fallback, intensity |
| [fix-library-scan-selection.md](fix-library-scan-selection.md) | Scan/import: dup afiles, selection, Live move counts, **preview stuck N-2**, RAW thumb speed |
| [profile-library-scan.md](profile-library-scan.md) | Establishing real scan/metadata/thumbnail/embedding performance baselines |
| [change-ai-plugin.md](change-ai-plugin.md) | Adding/changing a plugin, manifest, capability, runtime, permission, task, or package contract |
| [change-database-schema.md](change-database-schema.md) | Changing SQLite schema, migrations, database storage, backup, or restore |
| [test-sqlite-crud-fixture.md](test-sqlite-crud-fixture.md) | Testing SQLite model CRUD against a temporary per-test fixture |
| [change-live-photo.md](change-live-photo.md) | Apple Live Photo / Google Motion Photo detection, pairing, or long-press preview |
| [debug-plugin-runtime.md](debug-plugin-runtime.md) | Diagnosing plugin discovery, trust, setup, process, health, task, or output failures |
| [audit-plugin-trust-boundary.md](audit-plugin-trust-boundary.md) | Plugin trust boundary: P-1 missing declared file fixed/tested; P-2 network lookup fail-closed; P-3 host-managed package snapshot + single archive through extraction |
| [audit-storage-migration.md](audit-storage-migration.md) | Storage migration/backup/restore audit: verified copy before config switch, streaming restore, UUID backup entries, and residual snapshot trade-off |
| [audit-face-cluster-tx.md](audit-face-cluster-tx.md) | Face batch-write and cluster assignment transaction audit; full embedding RSS remains a 100k-face measurement follow-up |
| [audit-file-move-import.md](audit-file-move-import.md) | File move/copy/import/rename audit in t_cmds.rs / t_utils.rs (post-move maintenance diagnostics, library-copy add-or-cleanup, partial clipboard-import reporting, outside-library crash-window follow-up) |
| [audit-xmp-motion-cache.md](audit-xmp-motion-cache.md) | t_xmp.rs scope correction: no XMP write-back; cache validation, concurrent atomic writes, one-time legacy purge, and active-entry cleanup safety (X-1..X-5 resolved/verified) |
| [audit-dedup-delete.md](audit-dedup-delete.md) | Dedup staged trash, immediate keep/selection revalidation, permanent-delete routing, linked-row cascades, and person-cover repair (D-1..D-4 resolved/verified) |
| [audit-image-edit-save.md](audit-image-edit-save.md) | t_image.rs edit/batch/collage/photo-frame save: IMG-1..IMG-4 + IMG-6 export-overwrite staged backup + IMG-7 `Result`-returning `edit_image` resolved, with metadata/cache refresh and exact-path crash-temp recovery; IMG-5 overwrite by design |
| [audit-scan-mark-sweep.md](audit-scan-mark-sweep.md) | Scan mark-and-sweep: fail-closed traversal guards, explicit derived-row cleanup, recovery full re-mark before sweep, and skipped-file sweep suppression |
| [audit-library-switch-remove.md](audit-library-switch-remove.md) | Library switch/remove: exclusive rebind guard across scans/dedup/faces/imports, pooled-connection and embedding-cache reset, config mutation serialization |
| [audit-plugin-install-rollback.md](audit-plugin-install-rollback.md) | t_plugin.rs install/uninstall rollback: PLUG-1..5 fixed with staged same-parent directories, registry commit/restore, extraction budget, bounded deletion retry, and hidden transaction discovery exclusion |
| [audit-summary.md](audit-summary.md) | Current closure matrix for destructive/consistency audits, intentional contract reclassifications, and bounded remaining follow-ups |
| [audit-frontend-destructive.md](audit-frontend-destructive.md) | Frontend destructive/consistency re-verification: plugin install concurrency and drag-drop partial failure fixed; library/edit risks closed by backend guards; explicit copy+index contract retained |
| [audit-cyberpunk-theme.md](audit-cyberpunk-theme.md) | Cyberpunk theme: CP-1 global neon chrome and CP-2 guarded glitch rAF resolved; CP-3 force-dark and CP-4 Home-only backdrop confirmed by design; FX lifecycle verified safe |
| [audit-frontend-perf.md](audit-frontend-perf.md) | Frontend perf re-verification: PE-1 bounded off-screen thumbnail retention and PE-2 shared plugin contributions fixed; PE-3 retained as low-impact; PE-4 stale because the current cache is a 96 MiB in-memory LRU |
| [release-build.md](release-build.md) | Building/validating application installers, updater artifacts, or plugin packages |
| [change-crop-presets.md](change-crop-presets.md) | Changing ImageEditor crop ratios, photo-size catalog, or custom favorite ratios |
| [change-collage.md](change-collage.md) | Changing template/magazine collage UI, strips, free canvas, or `export_collage` (incl. cells + cell-sized decode) |
| [change-batch-process.md](change-batch-process.md) | Changing batch wizard, action palette, templates, or `batch_process_images` |
| [change-photo-style.md](change-photo-style.md) | Unified adjust recipes in ImageEditor presets/manual + LUT library |
| [change-color-match.md](change-color-match.md) | Traditional global Lab color match (追色) + single-image style 33³ `.cube` (`color_match_preview`, `export_color_match_lut`) |
| [change-print-layout.md](change-print-layout.md) | Changing 冲印排版 packing, A4, export vs print-sized print, temp cache, or cell-sized decode |
| [change-photo-frame.md](change-photo-frame.md) | EXIF photo frame (classic bar, float/sink blur+shadow, logo; preview/export) |
| [../../docs/guide/builtin-tools-roadmap.md](../../docs/guide/builtin-tools-roadmap.md) | Built-in tools roadmap: A–G (crop/collage/batch/print + color match + photo style/LUT + photo frame) |
| [change-face-index.md](change-face-index.md) | Changing face detection/embedding index workers, batch DB writes, clustering, or scan progress |
| [../../docs/guide/face-cluster-ann-plan.md](../../docs/guide/face-cluster-ann-plan.md) | Large-library face clustering: remove O(n²) all-pairs via ANN / blocked KNN (product plan) |
| [change-calendar.md](change-calendar.md) | Calendar sidebar dots, day/month selection, on-this-day, or Content date-range empty list |
| [change-library-perf.md](change-library-perf.md) | Large-library viewport loading or similar/semantic search performance |
| [change-compare-viewer.md](change-compare-viewer.md) | Multi-pane image comparison (2/4-up) or viewport sync |
| [change-collections.md](change-collections.md) | Collections tray, membership DB, or collection Content source |
| [change-smart-albums.md](change-smart-albums.md) | Smart Albums rule engine, editor, or smart Content source |
| [change-library-shortcuts.md](change-library-shortcuts.md) | Library panel All/Favorites/Today shortcuts or view-adaptive date grouping |
| [change-ai-prompt-import.md](change-ai-prompt-import.md) | AI PNG/JPEG prompt import into empty comments during scan |
| [change-media-badges.md](change-media-badges.md) | Configurable thumbnail media-info badges (format/ISO/shutter/…) |
| [settings-cross-window-sync.md](settings-cross-window-sync.md) | Main↔settings Pinia emit/listen hydrate gate and object equal-noop setters |
| [change-viewer-background.md](change-viewer-background.md) | Image/quick viewer canvas background modes + B shortcut |
| [change-ai-search-filters.md](change-ai-search-filters.md) | AI/similar/filename search file-type filter, result headers, text vs image-image ranking floors, free-text template, embed ladder |
| [change-smart-tags.md](change-smart-tags.md) | CLIP smart-tag categories, short prompts, smart-tag threshold |
| [change-image-search-model.md](change-image-search-model.md) | CLIP B/32 vision + bilingual int8 text default (Track C); SigLIP Track B probe; B0 abandoned |
| [../../docs/guide/altclip-phase0-probe.md](../../docs/guide/altclip-phase0-probe.md) | Track C bilingual text Phase 0 + product C (no reindex) |
| [../../docs/guide/siglip2-phase0-probe.md](../../docs/guide/siglip2-phase0-probe.md) | SigLIP2 ONNX Phase 0 probe results (no product vision default change) |
