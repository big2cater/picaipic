---
name: change-database-schema
description: Safely evolve PicAiPic per-library SQLite schema and storage behavior.
triggers:
  - database migration
  - schema change
  - SQLite column
  - backup restore
edges:
  - target: context/architecture.md
    condition: when tracing persistence and library ownership
  - target: context/conventions.md
    condition: when applying migration and safety rules
last_updated: 2026-07-19
---

# Change the SQLite Schema

## Context
PicAiPic keeps a database per library. `t_migration.rs` owns forward schema evolution, `t_sqlite.rs` owns queries/models, and `t_storage.rs` owns paths, custom storage, WAL checkpointing, backup, and restore. Current schema is at migration **v8** (v6 Live Photo columns; v7 collections; v8 unique `afiles(folder_id, name)` after dedupe — see `fix-library-scan-selection.md`).

## Steps
1. Inspect all reads/writes of the affected table/column and any serialized frontend shape.
2. Append a new migration version in `get_migrations()`; never redefine an already shipped version.
3. Make the migration idempotent with `table_has_column`, `CREATE ... IF NOT EXISTS`, or equivalent guards.
4. Apply data backfill/index creation in an order that older library databases can survive.
5. Update Rust row mapping, queries, structs, and frontend types/consumers together.
6. Test a new database and an existing pre-migration database; include multiple libraries if storage routing is involved.
7. For storage/backup changes, preserve the migration guard, WAL checkpoint, containment checks, and failure-safe copy/update order.

## Gotchas
- Updating create-table SQL alone does not migrate existing user databases.
- Advancing `PRAGMA user_version` before all operations succeed can strand a partially migrated database.
- Column order assumptions in `SELECT *`/row mapping can silently corrupt interpretation.
- Removing or moving DB files before configuration is durably updated risks data loss.

## Verify
- [ ] New DB initializes successfully.
- [ ] Old DB migrates once and a second startup is a no-op.
- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] Relevant `cargo test --manifest-path src-tauri/Cargo.toml` coverage passes.
- [ ] Backup/restore/custom-storage behavior remains correct if touched.

## Debug
Read `PRAGMA user_version` and `PRAGMA table_info(...)`; confirm the application opened the expected library DB path. Check WAL/SHM files and migration-in-progress state for storage moves.

## Update Scaffold
- [ ] Update architecture/current state when persistence behavior changes.
- [ ] Add a decision entry for irreversible or compatibility-sensitive schema choices.
- [ ] Log migration risk or follow-up with `mex log`.
