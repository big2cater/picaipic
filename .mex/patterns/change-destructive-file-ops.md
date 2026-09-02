---
name: change-destructive-file-ops
description: Keep user media and SQLite rows consistent across move, trash, and permanent-delete failures.
last_updated: 2026-08-29
---

# Change destructive file operations

## When to use

- Changing move/replace rollback behavior
- Changing outside-library move journaling or startup reconciliation
- Changing file or folder trash/permanent-delete commands
- Adding bulk destructive operations or their frontend result handling

## Safety contract

- User media bytes take priority over metadata consistency when an OS and SQLite operation cannot be made truly atomic.
- Outside-library moves persist a UUID journal before touching disk. Bind it to the original library ID and use that library's DB connection during completion/recovery.
- Recovery rolls back only when the source still exists and the destination is provably unchanged; it completes DB deletion only when the source is absent and destination exists. Ambiguous journals remain for diagnosis.
- Journal-named staging and replacement backups are deterministic siblings of the destination. The durable app-data journal is removed last.
- A Replace move must restore both the moved source and the previous destination when the DB update fails.
- Trash and permanent delete use a same-directory staged rename before the DB deletion.
- Commands must verify that a file ID resolves to the supplied path, or that a folder path exists in the current library DB, before staging anything.
- If the DB deletion fails, restore the staged path to its original name.
- A successful DB call that deletes zero rows is also a failure and must restore the staged path.
- Permanent delete removes the staged path only after the DB commit. If cleanup fails, restore the original path and report failure.
- Trash restores the original name after the DB commit, then calls the system trash API so the recycle-bin entry retains its user-facing name and location. If trash fails, the original remains and a later library scan can recover its DB row.
- Batch targets reject duplicate IDs/paths, require the DB deleted-row count to match the staged target count, and return only IDs whose physical cleanup completed. Validation, staging, and finalization failures increment `failed_count` and remain visible in the frontend.
- Dedup trash follows the same staging contract and revalidates keep/selection state under an immediate DB transaction before deleting the row.
- Mutating frontend IPC wrappers must rethrow backend errors so the UI does not remove an item after a failed coordinated operation.
- A command that mutates disk **before** its SQLite write must treat `Ok(0)` as a failure and roll the disk change back. Zero affected rows means the record is absent from the current library, so committing would move the media while the DB keeps pointing at the old path (and, for Replace, would already have destroyed the overwritten target). `move_file` was the last command missing this check.
- Rename helpers must keep only the final component of a caller-supplied new name. `PathBuf::push` and `PathBuf::set_file_name` replace the entire path for absolute or drive-prefixed values and `..` escapes the parent directory, which would move a library item somewhere the DB no longer describes. Use `sibling_path_with_name` in `t_utils.rs`.
- A superseded destination is **user media**, not a scratch artifact: `TransferResult::finalize` and `t_transfer_recovery::cleanup_completed` move it to the system trash rather than `remove_file`, so a Replace is recoverable and consistent with `delete_file`. A trash failure keeps the staged backup in place and only logs; it must never fall back to deleting it.
- Staged deletes are journaled. `stage_delete` registers the exact staged path with `t_output_temp::TrackedOutputTemp::create_staged_delete` **before** renaming; the guard restores the file on early drop and startup recovery restores it after a crash. Recovery always restores rather than deletes: losing the DB delete only means the file is re-indexed on the next scan, while deleting it would destroy media the user never confirmed. Recovery refuses to clobber an existing original path.
- Never let a temp-file cleanup helper delete a path that holds user media. `TrackedOutputTemp` now carries a `DropAction`: scratch output (batch/collage/photo-frame) is removed, staged deletes are restored.

## Touchpoints

| Area | Path |
|------|------|
| Disk staging and transfer rollback | `src-tauri/src/t_utils.rs` |
| Outside-move journal and startup reconciliation | `src-tauri/src/t_transfer_recovery.rs` |
| Tauri commands and batch result | `src-tauri/src/t_cmds.rs` |
| SQLite delete transactions | `src-tauri/src/t_sqlite.rs` |
| IPC error propagation | `src-vite/src/common/api.js` |
| UI removal and partial failure | `src-vite/src/components/Content.vue` |

## Verify

- `cargo test --manifest-path src-tauri/Cargo.toml transfer_tests -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml destructive_delete_tests -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml t_transfer_recovery::tests -- --nocapture`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `pnpm --dir src-vite build`
- Confirm Replace rollback restores both byte sequences in its regression test.
- Confirm a Replace sends the superseded file to the system trash, and keeps it in place (logging only) when the trash call fails.
- Confirm a crash between staging a delete and its DB delete restores the file on the next start: `cargo test --manifest-path src-tauri/Cargo.toml t_output_temp::tests -- --nocapture`.
- Confirm a DB failure restores every staged delete path.
- Confirm mismatched file ID/path pairs and non-library folder paths are rejected before disk staging.
