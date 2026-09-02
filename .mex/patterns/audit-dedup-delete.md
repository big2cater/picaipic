# Audit: t_dedup.rs (dedup merge / delete_selected destructive safety)

> Status: resolved (findings verified against current code and fixed/closed)
> Scope: `src-tauri/src/t_dedup.rs` — duplicate-group keep-selection (`set_keep`) and bulk
>        delete (`delete_selected`)
> Last reviewed: 2026-07-30
> Auditor: AI (read-only audit pass)

## Summary
The original disk/DB ordering and post-selection race were real. Dedup trash now stages the file,
revalidates eligibility and deletes the DB row under one immediate transaction, then invokes the
OS trash operation only after commit. Permanent dedup deletion was already routed through the
shared batch permanent-delete command. Linked rows cascade correctly; person cover references are
repaired explicitly before face cascade.

## Findings

### D-1 [Medium] `delete_selected` trashes disk before deleting the DB row — resolved
- Location: `src-tauri/src/t_dedup.rs:1533-1546` (trash first), `:1538-1544` (DB second)
- Detail:
  ```rust
  match t_utils::trash_path(&file_path) { Ok(_) => {}, Err(e) => { eprintln!(...); continue; } }
  match AFile::delete(file_id) {
      Ok(0) => eprintln!("File not removed from DB: id=..."),   // trashed, but DB row remains
      Ok(_) => deleted_file_ids.push(file_id),
      Err(e) => eprintln!("Failed to update DB..."),            // trashed, but DB row remains
  }
  ```
  The regular `delete_file` command does the opposite: `delete_staged_from_db(staged, "file",
  || AFile::delete(file_id))?` deletes the DB row FIRST, and only calls `staged.finalize_trash()`
  (move to trash) after `deleted > 0`. `delete_selected` has the two steps reversed.
- Impact: If `AFile::delete` returns `Ok(0)` (file row already gone — e.g., deleted elsewhere in
  a race) or `Err`, the file is already in the OS trash but its `afiles` row survives. The library
  then lists a broken entry pointing at a trashed (missing) path. Trash is user-recoverable, so
  severity is bounded, but it is a real disk↔DB asymmetry the other delete path avoids.
- Verification: **True.** Dedup deletion now uses the same staged-rename contract as regular
  deletion. It restores the source on guard/DB failure or zero-row deletion, and only moves the
  restored original name to the OS trash after the DB transaction commits.

### D-2 [Low] `delete_selected` ignores permanent-delete preference — stale / closed
- Location: `src-tauri/src/t_dedup.rs:1533`
- Detail: Bulk dedup delete always calls `t_utils::trash_path` (trash). The regular delete surface
  exposes both `delete_file` (trash) and `delete_file_permanently`. Dedup has no permanent path.
- Impact: Safer default, but inconsistent if the user expects dedup to honor a permanent-delete
  setting. Low.
- Verification: **Not true at the product call site.** `Content.vue` routes dedup permanent delete
  through `batchDeleteFiles(..., true)` and calls `dedupDeleteSelected` only for trash. The backend
  command remains intentionally trash-only. Its mutating IPC wrapper now rethrows failures.

### D-3 [Low] Candidate set is committed before the deletion loop — resolved
- Location: `src-tauri/src/t_dedup.rs:1450-1513` (collect in tx), `:1503` (commit), `:1518-1546` (loop)
- Detail: The to-delete set is collected inside a transaction that is committed at ~1503, then the
  deletion loop runs afterward. Between commit and deletion, `is_keep` could change (UI re-select)
  or the row could be removed by another flow. The original candidate read transaction did not
  keep a writer lock across filesystem deletion, so the collected list could become stale.
- Impact: Negligible under a modal dedup UI. Low.
- Verification: **True.** Immediately before DB deletion, a `BEGIN IMMEDIATE` transaction now
  revalidates `is_keep=0` and, for group/implicit selection flows, `is_selected=1`. A changed
  keep/selection restores the staged file and reports it as failed instead of deleting it.

### D-4 [Verify] Do linked rows cascade-delete with the file? — verified / cover fix added
- Location: `src-tauri/src/t_dedup.rs:1538` → `AFile::delete` → `delete_with_conn` (`t_sqlite.rs:2404`)
- Detail: `delete_with_conn` removes `athumbs`, `file_hashes`, `file_phashes`, and `afiles` (and
  invalidates the embed matrix). It does NOT touch `album_files`, `collection_items`, or face/people
  links itself. Whether those dangle depends on foreign-key `ON DELETE CASCADE` for `file_id` in those
  tables — which the earlier face-cluster audit flagged as potentially missing on `album_files`.
- Impact: If not cascaded, deleting a duplicate leaves orphan rows referencing a now-deleted
  `file_id` (broken album/collection entries, stray face assignments). Verify against the schema.
- Verification: `album_files` does not exist. Actual `afile_tags`, `acollections_files`, `faces`,
  exact/similar dedup membership, thumbnails, and hash tables all cascade or are explicitly
  removed. A real fixture now verifies tags, collections, faces, and dedup membership. The audit
  missed one non-FK reference: `persons.cover_face_id`; generic `AFile` deletion now switches it
  to a remaining face (or NULL) and clears the stale thumbnail before face cascade runs.

## Verified Safe (control cases)
- `set_keep` records the keep choice transactionally. Dedup deletion now performs its final
  eligibility check under an explicit immediate writer transaction.
- `delete_selected` only targets rows with `is_keep = 0`; the chosen keep file is excluded.
- `t_utils::trash_path` moves to the OS trash (not permanent) and verifies the path is gone before
  returning — matches the project's destructive-op trust boundary.
- Dedup uses `AFile::delete_with_conn_if`; regular deletion uses `AFile::delete`. Both share the
  same transactional core that removes the `afiles` row and related cache rows.
- Orphan cleanup after delete (`cleanup_orphan_items` / `cleanup_orphan_dup_groups`,
  `:1547-1593`) runs on a fresh connection and prunes empty groups / stale items.

## Recommended Fix Priority
All findings are closed. Regression coverage includes keep/selection changes, injected DB failure
with disk restoration, linked-row cascades, and person-cover repair.
