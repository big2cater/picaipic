# Pattern Index

Lookup table for PicAiPic task-specific runbooks. Read the matching pattern before implementation or diagnosis.

_Last reviewed: 2026-07-27 (S1/S6 SQLite metadata helpers, CRUD/EXIF/RAW fixtures, full Rust validation)._

| Pattern | Use when |
|---------|----------|
| [add-tauri-command.md](add-tauri-command.md) | Adding or changing frontend-to-Rust IPC, events, or cancellation |
| [change-black-hole-theme.md](change-black-hole-theme.md) | Black-hole theme, cosmos WebGL, idle PhotoVortex, chrome glass, intensity |
| [change-cyberpunk-theme.md](change-cyberpunk-theme.md) | Cyberpunk theme, night-city ambient, idle PhotoGlitchLayer, intensity gate |
| [fix-library-scan-selection.md](fix-library-scan-selection.md) | Scan/import: dup afiles, selection, Live move counts, **preview stuck N-2**, RAW thumb speed |
| [change-ai-plugin.md](change-ai-plugin.md) | Adding/changing a plugin, manifest, capability, runtime, permission, task, or package contract |
| [change-database-schema.md](change-database-schema.md) | Changing SQLite schema, migrations, database storage, backup, or restore |
| [test-sqlite-crud-fixture.md](test-sqlite-crud-fixture.md) | Testing SQLite model CRUD against a temporary per-test fixture |
| [change-live-photo.md](change-live-photo.md) | Apple Live Photo / Google Motion Photo detection, pairing, or long-press preview |
| [debug-plugin-runtime.md](debug-plugin-runtime.md) | Diagnosing plugin discovery, trust, setup, process, health, task, or output failures |
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
