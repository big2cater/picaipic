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
last_updated: 2026-07-17
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
The Rust command must be registered in `main.rs`; event listeners must return/clean up the unlisten function when component-scoped.

**Rust error pattern:** production commands return `Result<T, String>` (or another explicit error type) and add operation context. Avoid `panic!`; locks/files/native calls need explicit failure handling. Multi-column updates must not swallow intermediate errors with `let _ =` when the overall operation is reported as success (see `edit_album`).

**Disk/DB rename consistency:** for rename/move of user media, if the filesystem succeeds and the DB update fails, rollback the disk change (same pattern as `move_file` / `rename_file` / `rename_folder`) so the frontend failure path does not leave paths desynchronized.

**Database migration pattern:** inspect `PRAGMA user_version`, apply idempotent/table-column-aware migration logic, then advance the version only after successful changes. Never edit an existing migration to redefine databases already in the field.

**Plugin safety pattern:** validate normalized paths remain inside the intended store/task/output root before deletion or adoption. Preserve signature verification, trust prompts, permission gates, runtime probes/conflict checks, loopback binding, auth token, and input staging.

**Live Photo / Motion Photo pattern:** Apple Live Photos pair图片+视频 by EXIF ContentIdentifier (tag 0x0011 in `Context::Tiff`); videos are matched by ffprobe's `com.apple.quicktime.content.identifier` (try both dotted and underscored key variants). Google Motion Photos are single JPEGs with XMP `GCamera:MotionPhoto=1` and `Container:Directory` items specifying embedded video offset/length; `t_xmp.rs` parses XMP with `quick-xml` and `content_id` stores `motion:<offset>:<length>`. File-name stem fallback pairing (e.g., `IMG_1234.HEIC` + `IMG_1234.MOV`) runs when ContentIdentifier is absent. The `afiles` table's `live_photo_type` field uses: 0=none, 1=Apple image, 2=Apple video, 3=Google Motion Photo. `paired_file_id` is bilateral (both sides point to each other). Frontend preview uses a 400ms long-press timer in MediaViewer; the MOV/video layer is a controlless `<video>` overlay with `getAssetSrc()` URL conversion.

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
