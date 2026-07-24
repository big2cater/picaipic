---
name: change-smart-albums
description: Rule-based Smart Albums (智能相册) — LibraryState definitions + server-side SQL evaluation.
last_updated: 2026-07-24
---

# Change smart albums / 智能相册

## When to use

- Add/edit smart album rule fields, operators, or evaluation SQL
- Change SmartAlbumList/Edit UI or Content smart query source

## Touchpoints

| Area | Path |
|------|------|
| Rule SQL | `src-tauri/src/t_sqlite.rs` (`SmartRule`, `build_smart_rule_condition`, `build_smart_query_parts`, `get_smart_query_*`) |
| IPC | `src-tauri/src/t_cmds.rs` + `main.rs`; frontend `getSmartQueryCountAndSum` / `getSmartQueryFiles` in `api.js` |
| Persistence | LibraryState smart album JSON + sidebar selection |
| UI | `SmartAlbumList.vue`, `SmartAlbumEdit.vue`, Home sidebar entry |
| Content | `Content.vue` smart source via `getSmartFileList` |
| i18n | `src-vite/src/locales/en.json`, `zh.json` (`album.smart_edit.*`) |

## Rules

- Definitions are **JSON in LibraryState** (not SQLite tables). Evaluation is **server-side SQL** only.
- Require ≥1 rule; match mode `all` (AND) or `any` (OR). Max 20 rules in editor.
- Always AND search-exclusion + exclude Live companion videos.
- Core fields: favorite, rating, name, file_type, extension, dates, size, orientation, tag, person, has_gps, camera, lens.
- Size operators must match UI: `gt` / `gte` / `lt` / `lte` / `is` / `is_not` / `empty` / `not_empty`.
- Size values are always **MB** (fractional OK); backend multiplies by `1e6` to bytes. Do **not** treat large inputs as raw bytes.
- Person/camera/lens use pickers (`getPersons` / `getCameraInfo` / `getLensInfo`). Camera/lens value format remains `Make||Model`.
- Smart query failures must toast `album.smart_edit.query_error` — never silently show an empty album.
- Opening a smart album updates cached count (and first-file cover when available).
- Smart album `sort: { type, order }` is editable in SmartAlbumEdit (same indices as toolbar sort options). Persisted per album; Content uses album sort when opening.
- Date `before` / `after` / `between` compare **local calendar days** via SQLite `strftime(..., 'unixepoch', 'localtime')`, matching calendar range filters. Frontend date inputs use local midnight + local Y-M-D display (not UTC `toISOString`).

## Verify

- Frontend: `pnpm --dir src-vite build`
- Rust: `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual:
  - Size `is_not` / `empty` / `not_empty` no longer empty the album via unsupported-op error
  - Create album (favorite is true AND rating ≥ 4) → open → list matches
  - Person / camera / lens pickers populate from library data
  - Invalid rule shows toast, not a silent empty list
  - Sort type/order in editor is applied when opening the album
  - Date before/after boundary day matches local calendar day
  - Edit/delete; switch library isolation

## UX notes (2026-07-24)

- Opening **SIDEBAR.SMART** with no album selected must call `showEmptyContent` (set `contentReady`) — never leave the grid spinning.
- Stale `smartAlbum.id` (missing from list) clears selection and falls through; do not hang on loading.
- Date rule default op is `in_last` (relative amount/unit); before/after use local date input.
- Empty tag/person/camera/lens pickers show guidance instead of a dead disabled select.
