---
name: audit-summary
description: Consolidated current status of PicAiPic destructive and consistency audits.
last_updated: 2026-07-30
---

# Audit Summary - Destructive / Consistency Paths

> Status: re-verified 2026-07-30. All actionable correctness findings from the
> 2026-07-29 audit are resolved, verified safe, or reclassified as intentional contracts.

## Scope

This summary covers plugin trust/install/uninstall, database storage migration and restore,
face indexing/clustering writes, file move/copy/import/rename, Motion Photo cache handling,
dedup deletion, image edit/save, scan mark-and-sweep, library switch/remove, and the matching
frontend destructive paths.

The original static audit found several real high/medium-severity defects. They were fixed on
2026-07-29/30. The matching `audit-*.md` files are the detailed evidence and regression notes;
this file records the current state rather than preserving a stale fix queue.

## Current Status

| Area | Original findings | Current result | Runbook |
|------|-------------------|----------------|---------|
| Image edit/save | IMG-1..IMG-4, FE-IMG-2 | Resolved: same-directory staged replacement, original rollback, transactional metadata refresh, derived-cache invalidation, observable IPC failure, and exact-path crash-temp recovery | [audit-image-edit-save.md](audit-image-edit-save.md), [audit-frontend-destructive.md](audit-frontend-destructive.md) |
| Library switch/remove | LIB-1, LIB-2, FE-LIB-1/2 | Resolved: exclusive backend rebinding blocks overlap with scans, dedup, face indexing, and imports; pool/matrix reset on remove/switch | [audit-library-switch-remove.md](audit-library-switch-remove.md) |
| Plugin install/uninstall | PLUG-1..PLUG-5, FE-PLUG-1 | Resolved: staged swap/rollback, durable registry replacement, unpack ceilings, bounded Windows deletion retry, frontend install guard | [audit-plugin-install-rollback.md](audit-plugin-install-rollback.md), [audit-frontend-destructive.md](audit-frontend-destructive.md) |
| Plugin trust boundary | P-1..P-3 | Resolved: missing declared files rejected, network lookup fails closed, host-owned package snapshot closes source swap/TOCTOU | [audit-plugin-trust-boundary.md](audit-plugin-trust-boundary.md) |
| Dedup deletion | D-1..D-4 | Resolved/verified: staged trash, eligibility revalidation in immediate transaction, permanent-delete routing, cascades and person-cover repair | [audit-dedup-delete.md](audit-dedup-delete.md) |
| Scan mark-and-sweep | SCAN-1, SCAN-4 | Resolved/verified: defensive derived-row cleanup, full recovery re-mark, skipped-file sweep suppression, fail-closed traversal | [audit-scan-mark-sweep.md](audit-scan-mark-sweep.md) |
| File move/copy/import | C-1..C-5, FE-C-3, FE-IMPORT-1 | Resolved/verified: outside moves use a durable crash journal and startup reconciliation; copy retains explicit add-or-cleanup semantics; partial imports are visible | [audit-file-move-import.md](audit-file-move-import.md), [audit-frontend-destructive.md](audit-frontend-destructive.md) |
| Storage migration/backup | S-1..S-7 | Resolved/verified except accepted snapshot trade-off: checkpoint, verified temp copy, SHA-256, `quick_check`, streaming ZIP restore, stable ID entries | [audit-storage-migration.md](audit-storage-migration.md) |
| Face writes/clustering | F-A, F-C, T-1/T-2 | Resolved/verified: batch errors surface, assignments commit atomically, pooled connections have busy timeout | [audit-face-cluster-tx.md](audit-face-cluster-tx.md) |
| Motion Photo cache | X-1..X-5 | Resolved/verified: exact-length and `ftyp` validation, unique atomic temps, active-entry protection, bounded namespaced cleanup | [audit-xmp-motion-cache.md](audit-xmp-motion-cache.md) |
| Frontend destructive paths | FE-D-UI, FE-MOVE, FE-STORAGE | Verified safe: no optimistic destructive removal, truthy-only move updates, symmetric storage guards and visible failures | [audit-frontend-destructive.md](audit-frontend-destructive.md) |

## Important Reclassifications

- `copy_file` not inserting an `afiles` row is intentional. It also exports outside the library;
  current library-copy callers explicitly index the result and remove the copy if indexing fails.
- Frontend scan cancellation timing is no longer a library-isolation correctness boundary. The
  Rust `LibraryRebindGuard` rejects rebinding while conflicting work is active.
- Batch overwrite and library-index removal are explicitly destructive choices. Existing UI
  confirmation and the trash/permanent-delete distinction must remain intact.
- Mark-and-sweep removes stale index rows, not user media. An inaccessible traversal suppresses
  the sweep instead of treating unseen files as deleted.

## Remaining Follow-ups

These are real limitations, but none is an active normal-flow data-loss bug suitable for an
unscoped patch:

| ID | Status | Follow-up trigger |
|----|--------|-------------------|
| F-D | Measurement required | Face clustering loads all embeddings before graph construction. Measure RSS on a representative 100k+ face library before choosing mmap/chunking; avoid speculative complexity without the profile. |
| S-6 | Accepted trade-off | Checkpoint-based backup is a consistent practical snapshot for normal local use, but cannot make concurrent external writers impossible. Backup already fails if checkpoint completion cannot be established. |

## Non-Regressions

- Never replace staged/rollback destructive operations with direct overwrite/delete ordering.
- Never sweep arbitrary export roots for temp-like names. Register and sync the exact path before
  creating an output temp, validate the journal at startup, and retain ambiguous entries.
- Keep `src-vite/src/common/api.js` mutating IPC wrappers observable; do not convert errors to
  `null`, `false`, or empty collections where the UI could report success.
- Keep plugin signature, publisher trust, permissions, bearer auth, package snapshot, path
  containment, unpack limits, and transactional registry behavior together.
- Keep library rebinding exclusive with long-running media writers and preserve per-library DB
  pools, WAL checkpointing, embedding cache invalidation, and storage migration serialization.
- Keep large-library changes backed by representative performance or memory measurements.

## Verification Baseline

The closure runbooks record the exact focused tests. The consolidated 2026-07-30 baseline reached
174 passed / 3 ignored in the Rust suite, plugin-host regression passed, and the frontend
production build passed. Re-run the relevant focused tests after touching one area, plus:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
pnpm --dir src-vite build
powershell -ExecutionPolicy Bypass -File .\scripts\check_plugin_host.ps1
```
