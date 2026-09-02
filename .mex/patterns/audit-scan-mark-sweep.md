# Audit: scan mark-and-sweep (t_utils.rs index_album_worker)

> Status: verified; SCAN-4 fixed, SCAN-1 protected by schema plus defensive cleanup
> Scope: `src-tauri/src/t_utils.rs` `index_album_worker` + `src-tauri/src/t_sqlite.rs`
>        `delete_unseen_in_album` (mark-and-sweep of files no longer on disk)
> Last reviewed: 2026-07-30
> Auditor: AI (read-only audit pass)

The obsolete `traversed_count` resume-prefix counter was removed on 2026-07-30. Recovery already
starts from zero and revisits every supported file, so the counter and its increments had no
remaining progress, checkpoint, or sweep role.
The later report citing `t_utils.rs:2180/2213/2226` confused current method-closing line numbers with
the old counter locations; a full repository search finds no remaining `traversed_count` symbol.

## Summary
After a full album scan, `index_album_worker` runs mark-and-sweep: any `afiles` row whose
`last_scan_time` was not refreshed during the scan (`last_scan_time < current_scan_time`) is
deleted. This is the classic "file removed from disk → drop from index" cleanup. The **fail-closed
guards are strong**: mark-and-sweep runs ONLY when `scan_complete` (not cancelled, not traversal-
failed, album root accessible). A directory read error during `WalkDir` traversal sets
`traversal_failed = true` and `break`s the whole scan (4589 gate). So a removable/network drive
that drops mid-scan will NOT trigger deletion. The delete remains DB-only (no trash, no disk
delete), so this path never destroys user media.

## Findings

### SCAN-1 [Medium] Mark-and-sweep leaves orphan derived rows - Not reproducible on current schema; defense added
- Current schemas declare `ON DELETE CASCADE` for `athumbs`, `file_hashes`, and `file_phashes`,
  while pooled connections enable foreign-key enforcement.
- The sweep now explicitly removes those derived rows inside its transaction before deleting
  `afiles`, so normal rescans repair legacy database states too. Regression coverage validates
  this even with foreign-key enforcement disabled.

### SCAN-2 [Low, by design] Mark-and-sweep is DB-only (bypasses the trash/permanent distinction)
- Location: `src-tauri/src/t_utils.rs:4588-4595`
- Detail: Removed files are dropped from the index with no trash step. This is by design (it is an
  index, not a user delete), but it means a file merely moved/unmounted disappears from the library
  and is only recoverable by re-scanning — unlike explicit destructive ops that honor trash.
- Impact: Acceptable for an indexer; noted for consistency. A removable drive that was unmounted
  (not deleted) loses its index and needs a rescan.
- Fix: Optional — surface a "these N files are missing, rescan or remove from index?" prompt instead
  of silent drop, or document the behavior.

### SCAN-3 [Low, trade-off] A single inaccessible subdir aborts the entire scan (fail-closed)
- Location: `src-tauri/src/t_utils.rs:4259-4267`, `:4263`
- Detail: `WalkDir` yields `Err` on a directory it cannot read; the handler sets `traversal_failed =
  true` and `break`s, so the whole scan is abandoned and mark-and-sweep is skipped. Safe (no wrong
  deletion), but one flaky network subfolder forces a full re-scan of an otherwise-healthy album.
- Impact: Robustness over throughput trade-off; not a bug. Note for large network libraries.

### SCAN-4 [High] Crash-resume interaction with mark-and-sweep - Fixed
- Progress counts cannot prove that a later `WalkDir` traversal has the same prefix. Recovery now
  revisits every supported file and re-marks each surviving DB row with the current scan time
  before mark-and-sweep can run. The lightweight state cache preserves the warm-rescan path.
- If recovery explicitly skips a suspected problematic file, the completed scan suppresses its
  sweep and retains that record until a clean full scan establishes whether it is missing.

## Verified Safe (control cases) — the destructive-safety core
- Mark-and-sweep is gated by `if scan_complete` (`4590`); `scan_complete = !is_cancelled &&
  !scan_failed` (`4577`). A cancelled or failed scan never deletes.
- Any `WalkDir` traversal `Err` sets `traversal_failed = true` and `break`s the loop (`4261-4266`),
  so a mid-scan drive drop aborts before sweep.
- `scan_failed = traversal_failed || !directory_accessible(&album.path)` (`4576`) adds a final
  root-accessibility check.
- `delete_unseen_in_album` runs in a single SQLite transaction (`3284-3291`) — all-or-nothing for
  the batch, no partial-delete window.
- Scope is correctly limited to the scanned album's folders (`folder_id IN (SELECT id FROM afolders
  WHERE album_id = ?2)`), so one album's sweep cannot delete another album's rows.
- The delete touches DB rows only; it never removes files from disk (no trash, no permanent delete),
  so user media is never destroyed by this path — worst case is a lost index entry, recoverable by
  rescan.

## Remaining Trade-offs
- SCAN-2 and SCAN-3 remain intentional indexer behavior: a complete scan removes only stale
  database records, while an inaccessible subtree aborts instead of risking a false sweep.
