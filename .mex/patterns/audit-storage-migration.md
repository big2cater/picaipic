---
name: audit-storage-migration
description: Code-audit findings for DB storage migration, backup, and restore in t_storage.rs (pre-delete integrity, streaming restore, filename collision). Read before changing migration/backup/restore paths.
triggers:
  - storage migration audit
  - change_db_storage_dir review
  - backup/restore review
  - pre-delete integrity check
  - restore memory blow-up
edges:
  - target: patterns/change-database-schema.md
    condition: when a finding requires a schema/storage contract or migration change
  - target: patterns/change-destructive-file-ops.md
    condition: when a destructive delete needs staging/rollback (S-1)
  - target: context/conventions.md
    condition: when checking the local-first / user-media-preservation contract
last_updated: 2026-07-30
---

# Audit: storage migration / backup / restore (t_storage.rs)

Static audit of `src-tauri/src/t_storage.rs` covering `change_db_storage_dir`,
`backup_databases`, and `restore_databases`. S-1, S-2, S-3, and S-7 are
resolved; verified-safe checks and the accepted snapshot trade-off remain here
to prevent regressions.

## Findings

| ID | Severity | Location | Summary |
|----|----------|----------|---------|
| S-1 | Resolved | `migrate_db_storage_dir` | Verify copied DB SHA-256 plus `PRAGMA quick_check` before config switch; source cleanup is staged only after config save. |
| S-2 | Resolved | `backup_databases`, `restore_databases` | Database ZIP entries stream file-to-ZIP and ZIP-to-temp-file; restore never buffers all DBs. |
| S-3 | Resolved | backup metadata + restore lookup | New backups use stable library-ID ZIP paths recorded in metadata; legacy sanitized-name entries remain readable. |
| S-4 | Low (verified safe) | `restore_databases` path source | Write path comes from config, not zip entry name; zip-slip prevented. |
| S-5 | Low (verified safe) | `write_file_atomic` in restore | Refuses to overwrite existing target; restore uses new uuid + orphan rollback. |
| S-6 | Low (known trade-off) | `backup_databases` read | Snapshot read after `checkpoint_db` can still tear under concurrent writes. |
| S-7 | Resolved | backup ZIP contents | Removed unused `app-config.json`; backup metadata describes only restorable database entries. |

## S-1 — resolved: verify before source cleanup

`change_db_storage_dir` flow: copy → change config → save config → **delete source**.

```152:172:d:/ailab/PicAiPic/src-tauri/src/t_storage.rs
        fs::copy(&source_path, &target_path)
            .map_err(|e| format!("Failed to migrate database '{}': {}", library.name, e))?;
    // ...
    config.db_storage_dir = Some(target_dir_canon.to_string_lossy().into_owned());
    t_config::save_app_config(&config)?;
    // ...
        let _ = fs::remove_file(&source_path);   // error swallowed
        let _ = fs::remove_file(&wal_path);
        let _ = fs::remove_file(&shm_path);
```

Problems:

1. After `fs::copy` there is **no integrity check** (no SHA256 compare). If the target
   volume (e.g. external/USB) is written corruptly, the only good copy is lost once the
   source is removed.
2. The delete lines use `let _ =`, swallowing errors. If the remove fails (source still
   held by another connection) the function still returns `Ok(target)`, but the **good
   source lingers as an orphan** while config points at a possibly-corrupt target — the app
   opens the corrupt target and ignores the good source. This contradicts the
   "verify before destructive op, roll back on failure" contract in
   `change-destructive-file-ops.md`.

The migration now requires `wal_checkpoint` to complete, then copies to a unique
target-side temp file, compares streaming SHA-256 hashes, and runs read-only
`PRAGMA quick_check`. It stages a pre-existing target for rollback, publishes the
verified target, and saves config before staging the source DB/WAL/SHM for cleanup.
Cleanup failure is logged and leaves the known-good source in place; it cannot make
the configured target disappear.

## S-2 — resolved: stream backup and restore data

`restore_databases` walks the zip and `read_to_end`s **each** `.db` entry into a
`HashMap<String, Vec<u8>>` before writing anything:

```442:458:d:/ailab/PicAiPic/src-tauri/src/t_storage.rs
    let mut db_entries: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::new();
    for i in 0..archive.len() {
        // ...
        let mut content = Vec::new();
        entry.read_to_end(&mut content)...;
        db_entries.insert(lib_name, content);
    }
```

AGENTS.md requires protecting 10k–100k+ file libraries. A single `.db` can be hundreds of
MB; restoring several at once holds them all in memory simultaneously → OOM risk.

Backup uses `std::io::copy` from each database file to its ZIP entry. Restore reads
only the selected entry and copies it into a unique sibling temp file, syncs it,
renames it into its new library UUID path, then runs `PRAGMA quick_check`.

## S-3 — resolved: stable backup entry names

`sanitize_filename` maps `/` etc. to `_` but keeps spaces, so `"a/b"` and `"a_b"` both
become `"a_b"`. Backups are named `<sanitized>.db`; restore looks up with
`sanitize_filename(selection.library_name)`. Two distinct library names can overwrite /
mismatch each other.

New `backup-info.json` records the library UUID and `backupFile`, and each entry is
`databases/<library-uuid>.db`; restore follows that metadata instead of rebuilding a
name-derived ZIP path. Backups created before this change still use the prior sanitized
filename fallback for compatibility.

## S-4 — zip-slip already mitigated (verified safe)

Restore write path comes from `get_library_db_path_from_config(&config, &lib_id)` (config-
controlled), **not** the zip entry name. Even a `../../x.db` entry is ignored. ✅ No change.

## S-5 — restores to a new uuid, never overwrites (verified safe)

`write_file_atomic` refuses to overwrite an existing target (`path.exists()` errors out);
restore always uses `Uuid::new_v4()` new paths plus rollback that cleans orphan files. ✅

## S-6 — backup read may tear (known trade-off)

`backup_databases` `checkpoint_db`s then reads files directly. If another connection is
writing, checkpoint reduces but does not eliminate torn-read risk. Acceptable for single-
user desktop; could later use `VACUUM INTO` or the SQLite online backup API.

## S-7 — resolved: `app-config.json` omitted

Restore derives new libraries from the active configuration, so it never needed the
old `app-config.json` archive entry. New backups omit it.

## Verify

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- Storage unit tests cover verified-copy rejection, SQLite quick-check failure,
  atomic streamed restore writes, and legacy/current backup-entry resolution.
- A multi-gigabyte RSS measurement remains a manual stress check; the implementation
  has no per-entry `Vec<u8>` or archive-wide database cache.

## Update Scaffold

- [x] Record verified-copy, config-before-cleanup, and streamed backup/restore in `change-database-schema.md`.
- [x] Keep S-6 documented as the accepted concurrent-write snapshot limitation.
