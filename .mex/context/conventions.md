---
name: conventions
description: PicAiPic coding, organization, IPC, persistence, UI, and verification conventions.
triggers:
  - convention
  - naming
  - style
  - review
  - verify
edges:
  - target: context/architecture.md
    condition: when a convention depends on subsystem boundaries
  - target: context/plugin-runtime.md
    condition: when reviewing plugin host or plugin package changes
  - target: patterns/add-tauri-command.md
    condition: when extending frontend/backend IPC
  - target: patterns/change-database-schema.md
    condition: when persistent schema or storage behavior changes
  - target: patterns/change-live-photo.md
    condition: when working on Live Photo / Motion Photo features
  - target: patterns/change-photo-frame.md
    condition: when changing EXIF photo frame / 相框
last_updated: 2026-07-30
---


# Conventions

## Naming

- Rust backend modules use `t_*.rs`; Tauri commands use snake_case and the same string is used by frontend `invoke` wrappers.
- Vue components and views use PascalCase filenames; shared frontend helpers/stores use camelCase filenames such as `pluginRuntime.ts` and `libraryStore.js`.
- Pinia stores are defined as `useXStore`; component state/functions follow existing camelCase JavaScript style.
- Plugin identifiers, capability identifiers, profile identifiers, and runtime identifiers are stable kebab-case contract values; changing them is a migration/compatibility change.
- User-facing text belongs in both `src-vite/src/locales/en.json` and `zh.json`; avoid adding hard-coded UI strings when an i18n key is appropriate.

## Structure

- Vue views/components orchestrate UI; OS, database, indexing, and destructive file work stays in Rust behind `src-vite/src/common/api.js`.
- Left-sidebar panel routing uses absolute indices from `Home.vue` `buttons` order. Prefer `SIDEBAR` in `src-vite/src/common/constants.ts` (`LIBRARY=0` … `CALENDAR=4` … `MAP=9`). Inserting a panel (e.g. Smart Albums at 1) shifts later indices — update constants and Content/Home together.
- Register every new Tauri command in `src-tauri/src/main.rs` and add/update its frontend wrapper in `api.js`; long operations expose progress/cancel behavior through events and managed state.
- Put SQLite schema evolution in ordered migrations in `t_migration.rs`; keep runtime queries in `t_sqlite.rs` and database location/backup/restore behavior in `t_storage.rs`.
- Keep reusable state in Pinia stores and reusable UI in components; use Vue `<script setup>` and existing Tailwind/daisyUI patterns.
- AI plugin host contract changes span Rust host, frontend store/settings, manifest/schema docs, sample plugins, packaging scripts, and regression checks; audit all consumers.
- Keep release resources under `src-tauri/resources`; use download/package scripts instead of committing generated models, sidecars, environments, or caches.

## Patterns

**IPC wrapper pattern:**
```js
export async function operationName(value) {
  try {
    return await invoke('operation_name', { value })
  } catch (error) {
    console.error('operationName error:', error)
    throw error // preserve actionable failures for mutating operations
  }
}
```
The Rust command must be registered in `main.rs`; event listeners must return/clean up the unlisten function when component-scoped. Mutating metadata (`setFileRating` / favorite / rotate / batch metadata) must rethrow; do not map failures to `null` when the UI optimistically updates. Query helpers may still return `null` for empty-vs-error ambiguity (known debt). Prefer rethrow on **mutating** IPC (`importFile` / `updateFileInfo` / rating / favorite / rotate / batch metadata).

**Settings cross-window pattern:** main and settings webviews each own Pinia and listen for `settings-*-changed`. Settings.vue must gate emits with a mount hydrate flag (`emitSettings` / `settingsHydrating`) so opening Settings does not fan out every current value. Object settings need equal-noop setters (see `setGridMediaBadges`, `patterns/settings-cross-window-sync.md`, `patterns/change-media-badges.md`).

**Sidebar routing pattern:** `Content.vue` `updateContent` switches on `config.main.sidebarIndex` using `SIDEBAR.*`. Calendar date selection only updates `libConfig.calendar`; Content must be on `SIDEBAR.CALENDAR` or the grid will not load that range. See `patterns/change-calendar.md`.

**Rust error pattern:** production commands return `Result<T, String>` (or another explicit error type) and add operation context. Avoid `panic!`; locks/files/native calls need explicit failure handling. Multi-column updates must not swallow intermediate errors with `let _ =` when the overall operation is reported as success (see `edit_album`).

**Disk/DB rename consistency:** for rename/move of user media, if the filesystem succeeds and the DB update fails, rollback the disk change (same pattern as `move_file` / `rename_file` / `rename_folder`) so the frontend failure path does not leave paths desynchronized.

**Outside-library move recovery:** persist and sync an app-data journal before moving bytes. The journal must name the original library and deterministic destination-side staging/backup paths. Normal completion removes it last; startup reconciliation uses the original library connection, rolls back provably unchanged disk states, completes provable moves, and retains ambiguity.

**Destructive operation consistency:** trash and permanent deletion must validate the current-library file ID/path or folder record before staging the path with a same-directory rename. Restore the path on DB failure or zero-row deletion. Permanent deletion removes the staged path after commit; trash restores the original name before calling the system trash API. Batch deletion rejects duplicate targets, verifies the DB deleted-row count, and lists only paths whose final cleanup completed. Dedup trash additionally revalidates keep/selection eligibility in the same immediate transaction as DB deletion. Keep mutating delete IPC errors observable in the frontend; see `patterns/change-destructive-file-ops.md`.

**In-place image edit consistency:** encode pixels and preserved metadata into a same-directory temp before touching the destination. Stage an existing destination so finalize/DB-refresh failure can restore it. Pass and validate the current-library `fileId` for overwrite saves, then transactionally refresh metadata and invalidate thumbnail, embedding, exact-hash, and perceptual-hash state. See `patterns/audit-image-edit-save.md`.

**Arbitrary-root output temp recovery:** batch, collage, photo-frame, and future user-selected exports must register and sync their exact UUID-namespaced temp path in app data before writing it. Own the temp with an RAII guard for success/error/cancel/task-abort cleanup, and recover only strictly validated journal entries at startup. Never enumerate arbitrary user export roots or delete by a broad `*.picaipic-*.tmp` pattern. See `t_output_temp.rs` and `patterns/audit-image-edit-save.md`.

**Long media work and SQLite:** never hold a write transaction while reading media, decoding images, hashing, running O(n²) comparisons, or sorting large result sets. Compute outside the transaction, flush bounded prepared-statement batches, and keep visible group replacement atomic. Dedup and full album indexing use the shared `ActiveMediaScans` RAII gate; see `patterns/change-dedup-scan.md`.

**Database migration pattern:** inspect `PRAGMA user_version`, apply idempotent/table-column-aware migration logic, then advance the version only after successful changes. Never edit an existing migration to redefine databases already in the field.

**Plugin safety pattern:** validate normalized paths remain inside the intended store/task/output root before deletion or adoption. Preserve signature verification, trust prompts, permission gates, runtime probes/conflict checks, loopback binding, auth token, and input staging.

**Live Photo / Motion Photo pattern:** Apple Live Photos pair图片+视频 by EXIF ContentIdentifier (tag 0x0011 in `Context::Tiff`); videos are matched by ffprobe's `com.apple.quicktime.content.identifier` (try both dotted and underscored key variants). Google Motion Photos are single JPEGs with XMP `GCamera:MotionPhoto=1` and `Container:Directory` items specifying embedded video offset/length; `t_xmp.rs` parses XMP with `quick-xml` and `content_id` stores `motion:<offset>:<length>`. File-name stem fallback pairing (e.g., `IMG_1234.HEIC` + `IMG_1234.MOV`) runs when ContentIdentifier is absent. The `afiles` table's `live_photo_type` field uses: 0=none, 1=Apple image, 2=Apple video, 3=Google Motion Photo. `paired_file_id` is bilateral (both sides point to each other). Frontend preview uses a 400ms long-press timer in MediaViewer; the MOV/video layer is a controlless `<video>` overlay with `getAssetSrc()` URL conversion.

**AI prompt → comments pattern:** Prefer `t_ai_prompt` for PNG/JPEG generation metadata. Only fill **empty** `comments`; preserve non-empty user notes on re-scan. `AFile::update` does not write `comments`—use `update_column` when filling after change rescan. Gate with the import AtomicBool + Pinia `importAiPromptsToComments`. Bound file reads (PNG ≤4 MiB ancillary, JPEG marker walk ≤2 MiB). See `patterns/change-ai-prompt-import.md`.

**Search file-type mask pattern:** Library, filename, and AI/similar search share the same bitmask (0 all, 1 image, 2 video, 4 raw). Pass as `searchFileType` / `search_file_type`; apply in SQL (`build_file_type_condition`) before ranking when possible. See `patterns/change-ai-search-filters.md`.

**Viewer presentation pattern:** Thumbnail media badges and viewer canvas background are Pinia presentation flags only—no schema. Prefer helpers in `utils.ts` and existing settings event sync across main/settings windows. See `patterns/change-media-badges.md` and `patterns/change-viewer-background.md`.

**Cyberpunk scope pattern:** `setTheme` owns the global `html.is-cyberpunk` class and `app.css` owns route-wide neon daisyUI variables. Keep the city canvas and idle WebGL photo glitch scoped to Home/GridView; ImageViewer, ImageEditor, Settings, and auxiliary routes inherit chrome only. Animation callbacks must check active/render prerequisites before scheduling their next frame. See `patterns/change-cyberpunk-theme.md`.

**Photo frame / 相框 pattern:** Multi-select creative dialog (not ImageEditor tab). Host reads EXIF/LibRaw, composites classic bar or float/sink blur+shadow, optional local logo. IPC camelCase options via `photoFramePreview` / `exportPhotoFrame`; progress event `photo-frame-progress`. Save-as only; optional import copies outputs into the open album. See `patterns/change-photo-frame.md` and roadmap Phase G.

## Verify Checklist

- [ ] Frontend production build passes: `pnpm --dir src-vite build`.
- [ ] Rust formatting passes: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [ ] Rust compilation passes: `cargo check --manifest-path src-tauri/Cargo.toml`.
- [ ] Relevant Rust tests pass: `cargo test --manifest-path src-tauri/Cargo.toml` or a narrower documented target.
- [ ] New/changed IPC command names, payload keys, events, and cancellation behavior match across Rust and frontend.
- [ ] Database/media operations preserve originals, path boundaries, migrations, and large-library performance.
- [ ] UI text is present in both English and Chinese and follows existing component/store patterns.
- [ ] Plugin-related changes pass `scripts/check_plugin_host.ps1`; include fast stress for task/lifecycle/protocol changes.
- [ ] Packaging/resource/updater changes are verified with the relevant package preflight or CI-equivalent command.
