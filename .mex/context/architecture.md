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
last_updated: 2026-07-20
---

# Architecture

## System Overview

User interaction starts in Vue views/components under `src-vite/src/`. Pinia stores hold UI, library, configuration, and plugin state. Frontend operations call wrappers in `src-vite/src/common/api.js`, which use Tauri `invoke`; long-running work reports through Tauri events consumed with `listen`. `src-tauri/src/main.rs` registers the Rust commands and owns shared managers for video, AI, face indexing, cancellation, deduplication, and plugin runtimes. Rust modules perform filesystem/media work and read or update the current library's SQLite database. Original media remains in user-selected folders; PicAiPic stores metadata, thumbnails, models, plugin data, and caches separately. AI plugin capabilities cross another boundary: the host validates manifest/trust/permissions/runtime, stages inputs, starts a loopback HTTP plugin process, tracks tasks, and adopts approved outputs back into the library workflow.

**Scan-time metadata enrichment (2026-07-19):** optional AI prompt import (`t_ai_prompt`) may fill empty `comments` from PNG/JPEG generation metadata when the import flag is on. **Search:** semantic/similar queries share the library file-type bitmask via `ImageSearchParams.search_file_type`. **Viewer chrome:** canvas background mode and thumbnail media badges are Pinia-driven presentation only (no DB schema).

## Key Components

- **Vue shell (`src-vite/src`)** — routes Home/ImageViewer/ImageEditor/Settings, reusable components, Pinia stores, i18n (`en`/`zh`), and Tailwind/daisyUI presentation; depends on Tauri IPC rather than direct OS/database access.
- **IPC facade (`src-vite/src/common/api.js`)** — centralized frontend wrappers for commands and event listeners; depends on command names and payload shapes registered by Rust.
- **Tauri host (`src-tauri/src/main.rs`, `t_cmds.rs`)** — application lifecycle, command registration, managed state, window/menu behavior, and orchestration of indexing, files, metadata, search, faces, and deduplication.
- **Persistence (`t_sqlite.rs`, `t_migration.rs`, `t_storage.rs`, `t_config.rs`)** — per-library SQLite databases, schema migrations, configuration, custom database storage, WAL checkpointing, backup, and restore.
- **Media protocols (`t_protocol.rs`)** — `thumb://` and `preview://` URLs include a library id and must open that specific library database so in-flight WebView requests stay isolated across library switches.
- **Media/AI pipeline (`t_image.rs`, `t_video.rs`, `t_libraw.rs`, `t_heif.rs`, `t_jpeg.rs`, `t_jxl.rs`, `t_ai.rs`, `t_face.rs`, `t_ai_prompt.rs`)** — decoding, metadata, thumbnails, video compatibility, ONNX inference, embeddings, face detection/clustering, and optional scan-time AI prompt → empty comments import.
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
- No macOS release target in the current product scope, despite some harmless conditional Rust branches remaining.
- No general-purpose unrestricted plugin execution contract: plugins are permissioned, signed in release builds, authenticated, and mediated by the host.
