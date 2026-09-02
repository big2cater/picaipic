---
name: architecture
description: PicAiPic application flow and major subsystem boundaries.
triggers:
  - architecture
  - system design
  - data flow
  - Tauri command
  - database flow
edges:
  - target: context/stack.md
    condition: when implementation libraries or versions matter
  - target: context/conventions.md
    condition: when extending a component or IPC boundary
  - target: context/plugin-runtime.md
    condition: when the flow crosses into an AI plugin process
  - target: patterns/add-tauri-command.md
    condition: when adding or changing a frontend-to-Rust operation
  - target: patterns/change-live-photo.md
    condition: when working on Live Photo / Motion Photo detection, pairing, or preview
  - target: patterns/change-color-match.md
    condition: when changing traditional color match / 追色 or host style .cube export
  - target: patterns/change-photo-style.md
    condition: when changing photo styles / 照片格调 or LUT library
  - target: patterns/change-photo-frame.md
    condition: when changing EXIF photo frame (classic bar, float/sink blur, logo)
  - target: patterns/change-batch-process.md
    condition: when changing batch process concurrency, cancel, or atomic writes
last_updated: 2026-07-30
---

# Architecture

## System Overview

User interaction starts in Vue views/components under `src-vite/src/`. Pinia stores hold UI, library, configuration, and plugin state. Frontend operations call wrappers in `src-vite/src/common/api.js`, which use Tauri `invoke`; long-running work reports through Tauri events consumed with `listen`. `src-tauri/src/main.rs` registers the Rust commands and owns shared managers for video, AI, face indexing, cancellation, deduplication, and plugin runtimes. Rust modules perform filesystem/media work and read or update the current library's SQLite database. Original media remains in user-selected folders; PicAiPic stores metadata, thumbnails, models, plugin data, and caches separately. Clipboard imports return both their successful indexed records and a failed-item count, so partial import loss is visible in the UI. AI plugin capabilities cross another boundary: the host validates manifest/trust/permissions/runtime, stages inputs, starts a loopback HTTP plugin process, tracks tasks, and adopts approved outputs back into the library workflow.

**Scan-time metadata enrichment (2026-07-19):** optional AI prompt import (`t_ai_prompt`) may fill empty `comments` from PNG/JPEG generation metadata when the import flag is on. A completed scan marks every surviving indexed file before DB-only mark-and-sweep; cancellation, traversal failure, root inaccessibility, or an explicitly skipped recovery file suppresses the sweep. Library switching/removal takes an exclusive backend rebind guard, while scanning, deduplication, face indexing, imports, and outside-library moves hold operation guards; current-library DB resolution cannot change beneath an active writer. Outside-library moves additionally persist an original-library-bound journal before disk mutation and reconcile it after database initialization on the next launch. **Search:** semantic/similar queries share the library file-type bitmask via `ImageSearchParams.search_file_type`. **Viewer chrome:** canvas background mode and thumbnail media badges are Pinia-driven presentation only (no DB schema).

**Photo style + LUT library (2026-07-21, UI merge):** host `t_lut` manages user `.cube` library and applies recipe pipeline (base → LUT → effects) for ImageEditor save/batch. Editor UX: unified recipes under **Presets** + **Manual** with layered CSS/host preview (geometry-aware, cached). ImageViewer toolbar exposes built-in Edit (not plugin toolbar icons). Batch text/watermark can stamp EXIF capture time. Traditional color match remains `t_color_match`. Both are local; Photon AI recolor is cloud API and is not used here.

**Photo frame / 相框 (2026-07-22, G-Frame-1+G2):** multi-select creative tool draws EXIF info frames (photix-inspired). Classic solid bars; float/sink layouts use cover-blur canvas + soft drop shadow (sink biases photo up, bar in lower blur zone). Optional local logo (path + position). Host reads EXIF/LibRaw summary, previews JPEG bytes, exports multi-file save-as with cancel/progress. Does not mutate originals; optional import copies outputs into the open album.

## Key Components

- **Vue shell (`src-vite/src`)** — routes Home/ImageViewer/ImageEditor/Settings, reusable components, Pinia stores, i18n (`en`/`zh`), and Tailwind/daisyUI presentation; depends on Tauri IPC rather than direct OS/database access.
- **IPC facade (`src-vite/src/common/api.js`)** — centralized frontend wrappers for commands and event listeners; depends on command names and payload shapes registered by Rust.
- **Tauri host (`src-tauri/src/main.rs`, `t_cmds.rs`)** — application lifecycle, command registration, managed state, window/menu behavior, and orchestration of indexing, files, metadata, search, faces, and deduplication.
- **Persistence (`t_sqlite.rs`, `t_migration.rs`, `t_storage.rs`, `t_config.rs`)** — per-library SQLite databases, schema migrations, configuration, custom database storage, WAL checkpointing, backup, and restore.
- **Face persistence (2026-07-30):** face scan results write in transactional batches and failures terminate the task visibly. Face clustering computes outside the DB write lock, then applies new-person creation plus unassigned face links in one immediate transaction; a conflict or error rolls back the assignment plan.
- **Storage safety (2026-07-30):** custom database moves require a completed WAL checkpoint and use a target-side verified copy (SHA-256 plus read-only SQLite `quick_check`) before the atomic config switch; old DB/WAL/SHM cleanup is staged only afterward. Backup and restore stream databases, and new ZIP metadata maps stable library IDs to entry paths so display-name sanitization cannot collide. Backup snapshots remain checkpoint-based rather than SQLite online backups.
- **Media protocols (`t_protocol.rs`)** — `thumb://` and `preview://` URLs include a library id and must open that specific library database so in-flight WebView requests stay isolated across library switches.
- **Media/AI pipeline (`t_image.rs`, `t_output_temp.rs`, `t_color_match.rs`, `t_lut.rs`, `t_video.rs`, `t_libraw.rs`, `t_heif.rs`, `t_jpeg.rs`, `t_jxl.rs`, `t_ai.rs`, `t_face.rs`, `t_ai_prompt.rs`)** — decoding, metadata, thumbnails, video compatibility, EXIF photo frame (`apply_photo_frame`: classic bar + float/sink blur + optional logo), traditional global Lab color match (stats on ≤1024 max-edge; single-pass full-res grade) + single-image style 33³ `.cube` export, photo-style/LUT library (`t_lut`), journaled exact-path recovery for batch/collage/photo-frame output temps, ONNX inference, embeddings, face detection/clustering, and optional scan-time AI prompt → empty comments import.
- **Live Photo / Motion Photo (`t_xmp.rs`, `t_live_photo.rs`, `t_heif.rs`, pair logic in `t_sqlite.rs`)** — Apple Live Photo (HEIC+MOV via ContentIdentifier UUID), Google Motion Photo (JPEG+embedded MP4 via XMP), and HEIC-internal video (`live_photo_type=4` via libheif items/sequences + ffmpeg fallback); long-press preview and still/video/pair/conversion export; extracts land in `app_cache_dir()/motion_cache/`; see `patterns/change-live-photo.md`.
- **Search / library views (`t_sqlite` query + `ImageSearchParams`, Content/GridView)** — folder/timeline filters, semantic/similar search with shared file-type bitmask, collections/smart albums; grid section headers for search result kinds.
- **Plugin host (`t_plugin.rs`, `t_sandbox.rs`, frontend plugin store/settings)** — signed package lifecycle, trust/permissions, runtime profiles, loopback HTTP tasks, logs, outputs, model bindings, and input staging; see `context/plugin-runtime.md`.

## External Dependencies

- **User filesystem** — source of truth for media; asset protocol scopes are restored for selected libraries, and file operations must respect trash/permanent-delete semantics.
- **SQLite** — local metadata database per library via bundled `rusqlite`; schema version is tracked with `PRAGMA user_version`.
- **Bundled native/media stack** — LibRaw, libheif, libjpeg-turbo, jxl-oxide, FFmpeg/FFprobe, EXIF libraries, and Rust `image` support broad formats and thumbnails.
- **Local AI assets** — ONNX Runtime plus CLIP and InsightFace model files downloaded into `src-tauri/resources/models` and bundled for release.
- **GitHub Releases** — distributes application updates and binary/model sidecars; updater artifacts are signed.
- **Aptabase** — optional telemetry, compiled in only when `APTABASE_KEY` is provided.

## What Does NOT Exist Here

- No required cloud account, server database, or forced upload pipeline; the product is local-first.
- No web-service backend for the main application; privileged work runs in the local Rust host.
- No Android, iOS, or macOS application target. Windows/Linux builds may retain harmless conditional source branches or upstream dependency metadata, but dedicated mobile/macOS bundle assets and native bridges are not maintained.
- No general-purpose unrestricted plugin execution contract: plugins are permissioned, signed in release builds, authenticated, and mediated by the host.
