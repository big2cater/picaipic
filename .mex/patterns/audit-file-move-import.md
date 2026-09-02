# Audit: File Move / Copy / Import / Rename Command Layer

> Status: resolved; C-1..C-5 verified or remediated, including crash recovery for outside-library moves
> Scope: `src-tauri/src/t_cmds.rs` file commands + `src-tauri/src/t_utils.rs` transfer helpers
> Last reviewed: 2026-07-30
> Auditor: AI (read-only audit pass)

## Summary
The command layer wraps user-visible destructive file operations (move / copy / import /
rename) over `t_utils` transfer helpers that implement a staged-copy + backup + rollback
pattern. The transfer primitives are sound. The command layer now makes partial clipboard
imports visible and logs failures in post-move derived-data maintenance:
- `rename_file` and `move_file` roll back the disk on DB failure (good).
- `copy_file` is a transfer primitive; every shipped UI caller adds a successful in-library
  copy to SQLite and removes the copied path if indexing fails.
- Clipboard import returns both successful files and `failedCount`, which the UI reports.

## Findings

### C-1 [Medium] Move commits disk + DB, then silently drops cache/consistency steps - Fixed
- Location: `src-tauri/src/t_cmds.rs:1038-1072`
- Resolution: thumbnail relocation, Live Photo pairing, and album recount failures are each
  logged with file/album context. Thumbnail cache is derived and regenerates on demand; the
  move remains successful after its primary disk/DB contract commits.
- Residual: post-commit pairing/count repair is not filesystem-atomic. A repeated SQLite
  failure is now observable and can be repaired by the normal pairing/recount paths.

### C-2 [Low] Crash window in `move_file_outside_library` - Fixed
- Location: `src-tauri/src/t_cmds.rs:990-1014`
- Detail: `move_file_with_policy` physically moves the file first; `AFile::delete` runs
  after. A crash between the two leaves a DB record pointing at an already-moved file.
- Impact: Orphan DB record after a crash; normal flow is safe because delete failure rolls
  back the disk move.
- Resolution: each outside-library move first persists and syncs a UUID journal under the app
  data directory. The journal binds the original library ID, file ID, source, destination, and
  deterministic same-parent staging/backup paths. The move holds the media-operation guard,
  deletes through the original library's connection, and removes the journal only after disk,
  DB, and backup cleanup complete. Startup reconciliation runs immediately after DB creation:
  it rolls back when the source still exists, completes the original-library DB deletion when
  the source is gone and destination exists, and retains/logs ambiguous or externally changed
  states without destructive guessing. Three filesystem fault-state tests cover prepared replace,
  replace rollback, and changed-destination refusal.

### C-3 [Medium] `copy_file` never writes the DB - Verified safe in shipped flows
- `copy_file` is intentionally a filesystem transfer helper because it supports exporting to
  arbitrary destinations. `Content.vue` resolves whether the destination belongs to the
  current library; in that case it calls `addFileToDb` after copy and calls
  `removeUntrackedFile` if indexing fails. The drop-to-folder flow applies the same guard.
- No current frontend path bypasses this contract. A future caller that copies into a library
  folder must retain the explicit add-or-cleanup sequence.

### C-4 [Low] Batch clipboard import silently drops per-file failures - Fixed
- `import_clipboard` now returns `ClipboardImportResult { files, failedCount }`.
  `Content.vue` refreshes successful imports and displays a localized partial-failure warning
  whenever one or more supported clipboard files could not be imported.

### C-5 [Verify] Does `rename_file` sync the `path` column? - Verified safe
- `afiles` stores `folder_id` plus `name`; response `file_path` is derived from the joined
  `afolders.path` and `afiles.name`. Renaming updates the only stored filename fields, so no
  path-column update is needed.

## Verified Safe (control cases)
- `move_file` DB-failure branch (954-963) correctly calls `transfer.rollback_move`.
- `import_file` / `save_bytes_*` / `save_downloaded_bytes_*` all go through
  `get_unique_path`, so the `remove_file(&new_path)` orphan cleanup (1047, 1055, 1408,
  1415, 1469, 1476) cannot delete a pre-existing file.
- `move_file_with_policy` staged path (1448-1473) keeps a backup and rolls back on source
  cleanup failure; `TransferResult::finalize` (1213-1229) failing only leaves a
  `.lap-backup-*` temp file — file integrity is unaffected.
- `delete_file` (1484-1489) follows stage -> DB delete -> `finalize_trash` (trash, not
  permanent).
- `rename_file` (859-915) has symmetric disk rollback on any DB write failure.

## Verification
- `cargo test --manifest-path src-tauri/Cargo.toml t_transfer_recovery::tests` - 3 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` - 171 passed / 3 ignored.
- Rust format/check and frontend production build pass.
