# Audit: library switch / remove (current-library pointer & connection pool)

> Status: verified; LIB-1 and LIB-2 fixed, LIB-4 already serialized
> Scope: `src-tauri/src/t_cmds.rs` `switch_library`/`remove_library` (154, 148) and
>        `src-tauri/src/t_config.rs` `switch_library`/`remove_library`/`add_library`
>        (743, 698, 647); connection pool in `t_sqlite.rs` (`open_conn` 10261,
>        `clear_conn_pool` 10253, `CONN_POOL` 10150)
> Last reviewed: 2026-07-30
> Auditor: AI (read-only audit pass)

## Summary
Library add/remove/switch mutate the `current_library_id` pointer in app config. The connection
pool (`CONN_POOL: Vec<(String, Connection)>`) is keyed by **normalized DB path**, and `open_conn()`
resolves to `get_current_db_path()` (the *current* library) on **every call**, dropping any pooled
connection whose path no longer matches. This makes every DB helper (including the scan's
`AFile::delete_unseen_in_album` at `t_sqlite.rs:3283`) implicitly target whatever library is current
*at the moment of the call*. Library rebinding is now guarded server-side, so a switch or remove
cannot overlap indexing, deduplication, face indexing, or import writes.

## Findings

### LIB-2 [Medium-High] Switching/removing the current library mid-operation redirects in-flight DB writes - Fixed
- Resolution: `ActiveMediaScans` now provides an exclusive `LibraryRebindGuard`. Full scans,
  dedup scans, face indexing, and each import entry acquire operation guards. Switch/remove acquire
  the exclusive guard and fail before config changes when any writer is active; new writer work is
  also rejected until rebinding completes.

### LIB-1 [Low-Medium] `remove_library` of the current library does not call `clear_conn_pool()` - Fixed
- Resolution: removal holds the rebind guard, clears pooled connections and the embedding matrix,
  initializes the replacement current database, then refreshes scopes and folder-mtime sync.

### LIB-3 [Low, by design] `remove_library` permanently deletes DB + WAL/SHM + thumb cache (no trash)
- Location: `src-tauri/src/t_config.rs:721-737`
- Detail: Deletes `*.db`, `*.db-wal`, `*.db-shm`, and the thumbnail cache dir via `let _ =
  fs::remove_file(...)` / `fs::remove_dir_all(...)` (errors swallowed). This is the library index
  (not user photos), but it is irreversible (recoverable only by full rescan) and bypasses the
  trash/permanent distinction other destructive ops honor.
- Impact: Acceptable for "remove library" (explicit, destructive-by-intent); noted for policy
  consistency. Swallowed errors mean a failed delete is invisible.
- Fix: Optional — confirm the delete succeeded and surface an error; consider a "remove from app
  only, keep DB file" mode.

### LIB-4 [Verify] Is `remove_library` guarded against concurrent calls? - Already protected
- `t_config` already serializes config reads/writes with `CONFIG_IO_LOCK`; the rebind guard adds
  lifecycle exclusivity around switch/remove work.

## Verified Safe (control cases)
- `add_library` (`t_config.rs:647`) creates a **new** library id (no overwrite of existing) — safe.
- `remove_library` refuses to remove the last library (`:702`) and re-points `current_library_id` to
  `libraries[0]` when removing the current one (`:715-716`).
- `open_conn` (`:10261`) drops pooled connections whose path != current — prevents *accidental reuse*
  of a stale library connection on the next call (mitigates LIB-1 re-use, not LIB-2 redirect).
- `restore_databases` (`t_storage.rs:430`) refuses to replace an existing destination (always targets
  a new library id, `:521`) and writes via `write_file_atomic` (`:522`, `*.picaipic-restore.tmp` +
  rename) with `cleanup_restored_db_files` on failure — good atomic restore.
- `backup_databases` (`t_storage.rs:305`) streams libraries into a zip with `backup-info.json` — read-
  only source, safe.

## Remaining Trade-off
- LIB-3 remains intentional: removing a library permanently deletes only its local index database
  and thumbnail cache, never user media. A "forget but retain database" mode is a future UX choice.

## 2026-07-30 FE-LIB-1 re-verification
- The reported need to check frontend `index.status` / `is_face_indexing` before switch or removal is
  stale. `LibraryRebindGuard::begin_library_rebind` checks active album scans, dedup, face indexing,
  and imports in one backend mutex and rejects before `t_config` changes the current-library pointer.
- `AlbumScanGuard` and `FaceIndexGuard` are acquired by the workers themselves, so this protection
  does not depend on frontend Pinia state being current. `ManageLibraries.vue` may improve error UX,
  but adding a frontend-only safety gate is not required for correctness.
