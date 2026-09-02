# Audit: t_image.rs (in-place edit / batch / collage save)

> Status: IMG-1..IMG-4, IMG-6, IMG-7 resolved and verified; IMG-5 by design
> Scope: `src-tauri/src/t_image.rs` — `edit_image`, `batch_image_write`, `save_collage_image`,
>        `export_*` save paths
> Last reviewed: 2026-08-29
> Auditor: AI (read-only audit pass)

## Summary
The reported `edit_image` data-loss and stale-index risks were real. Edits now encode and copy
metadata into a same-directory temporary file, sync it, stage any existing destination, and only
then rename the completed output into place. A failed finalize restores the original. In-place
library edits carry `fileId`; after replacement, the backend validates the id/path pair, refreshes
metadata in one SQLite transaction, invalidates thumbnail/embedding/exact-hash/dHash state, and
restores the original file if the database refresh fails.

Correction to the original report: `prepare_metadata_backup_path` copied the whole original file,
not EXIF alone. It still did not prevent IMG-1 because encode failures removed that backup without
restoring it, and PNG overwrites did not create it at all.

## Findings

### IMG-1 [High] Resolved — failed edit no longer touches the original
- Location: `src-tauri/src/t_image.rs:1190-1198`
- Detail:
  ```rust
  let save_ok = if format == image::ImageFormat::Jpeg {
      if let Ok(file) = std::fs::File::create(path) {        // truncates dest NOW
          let mut encoder = ...JpegEncoder::new_with_quality(file, quality);
          encoder.encode_image(&img).is_ok()
      } else { false }
  } else {
      img.save_with_format(path, format).is_ok()             // also truncates/overwrites
  };
  if !save_ok { cleanup_metadata_backup(&metadata_backup_path); return false; }
  ```
  `File::create` truncates the file the instant it opens. If `encode_image` then fails (OOM, disk
  full, crash), `save_ok=false`, the function returns `false`, but the original file is already
  gone. `prepare_metadata_backup_path` (1224) only backs up **EXIF**, not pixel data, so the
  backup cannot restore the image. This is the single most destructive path in the module because
  it mutates user-owned source files directly. The danger is specific to overwrite/in-place mode
  (`source_file_path == dest_file_path`); save-as (`source != dest`) only risks the new file.
- Impact: An edit that fails mid-encode permanently destroys the user's original photo. Violates the
  project's "treat original media as user-owned source data; destructive ops must remain guarded"
  rule and is inconsistent with the atomic temp+rename used everywhere else in this file.
- Resolution: write to a UUID-namespaced same-directory edit temp that preserves the output
  extension for metadata tooling. The completed temp is synced, the old destination is staged,
  and rename failure restores it. Regression tests cover commit and rollback behavior.

### IMG-2 [Medium] Resolved — in-place edits refresh metadata and invalidate derived state
- Location: `src-tauri/src/t_image.rs:1159-1222` (no DB calls)
- Detail: After overwriting the file, the library's stored hash/size/thumbnail/phash/orientation
  stay describing the old bytes. `move_file` / `rename_file` update the DB; `edit_image` does not.
- Impact: Edited file shows a stale thumbnail, wrong size, and may dedup/search against old content.
- Resolution: ImageEditor and FileInfo quick-save pass optional `fileId`. The backend accepts it
  only when source and destination refer to the same item, verifies the current-library row path,
  rebuilds file metadata, deletes the stored thumbnail and disk cache, clears embeddings, and
  removes exact/dHash rows so they regenerate on demand/next dedup scan.

### IMG-3 [Low] Resolved — metadata failure discards only the unfinished temp
- Location: `src-tauri/src/t_image.rs:1205-1214`
- Detail: On `copy_metadata_to_output` failure with a backup present, it does
  `fs::copy(metadata_source, path)`, which replaces the entire (edited) file with the original —
  silently reverting the edit. With no backup (save-as), it `fs::remove_file(path)` and discards the
  output. The two branches behave differently and the revert is undocumented.
- Impact: Confusing implicit revert on a metadata edge case. Low.
- Resolution: metadata is copied to the edit temp before finalization. Failure removes that temp and
  returns failure while the original/destination remains unchanged; there is no implicit revert.

### IMG-4 [Low] Resolved — exact-path output-temp journals recover crash leftovers
- Location: `src-tauri/src/t_output_temp.rs`, startup setup in `src-tauri/src/main.rs`, and
  batch/collage/photo-frame writes in `src-tauri/src/t_image.rs`.
- Detail: Each export now receives a UUID-namespaced same-directory temp. Before any temp bytes are
  written, `TrackedOutputTemp` persists and syncs a journal containing its exact path and output
  kind. Normal return, error, cancellation, and Tokio task abort drop the guard and remove the temp.
  Startup enumerates only the app-data journal directory, validates the journal filename, canonical
  UUID, kind, and temp filename suffix, then removes only that exact registered path.
- Impact: A hard process exit no longer leaves newly-created batch, collage, or photo-frame temps
  indefinitely, without scanning arbitrary user-selected export roots.
- Security boundary: corrupt, mismatched, or unrecoverable journals are retained and logged; they do
  not authorize deletion. Historical fixed-name leftovers created by older builds are not swept.

### IMG-5 [Low] Batch `overwrite` mode edits library files in place (by design)
- Location: `src-tauri/src/t_image.rs:3970-3976` (dest stays in source folder), `:3932-3942`
  (remove+rename over dest)
- Detail: "overwrite" output mode writes into the source file's own folder and replaces the original.
  Atomic temp+rename prevents mid-write corruption, but a successful run still destroys the original.
  Intended behavior, but should be explicitly confirmed in the UI (non-recoverable unless trashed).
- Impact: By design; noted for completeness.

### IMG-6 [High] Resolved — export overwrite no longer deletes the destination first
- Location: `src-tauri/src/t_image.rs` — `save_collage_image`, `process_one_batch_file`,
  `export_photo_frame`
- Detail: all three published their output with:
  ```rust
  if path.exists() { let _ = fs::remove_file(path); }
  if let Err(e) = fs::rename(temp_path, path) {
      if let Err(copy_err) = fs::copy(temp_path, path) { return Err(...); }
  }
  ```
  The destination was removed **before** the replacement existed. When the rename failed
  (cross-volume or a locked target) and the copy fallback also failed (disk full, read-only media),
  the call returned an error with the user's file already deleted and the temp removed by
  `TrackedOutputTemp::drop` — neither the previous file nor the new output survived. Same class as
  IMG-1, but on the export paths rather than the in-place edit path, and the earlier "Verified Safe"
  note below was wrong because it only considered crash recovery, not a failed finalize.
- Impact: exporting a collage, a batch run, or a framed photo onto an existing file could silently
  destroy that file.
- Resolution: the three exporters share `finalize_tracked_output` → `replace_output_file`, which
  stages any pre-existing destination to a same-directory backup and restores it when both the
  rename and the copy fallback fail; the backup is removed only after the replacement is committed.
  Regression tests cover a new destination, an existing destination, and a forced finalize failure
  that must leave the original intact.

## Verified Safe (control cases)
- `save_collage_image`, `process_one_batch_file`, and `export_photo_frame`: write registered unique
  same-directory temps, then publish them through `replace_output_file` (staged backup plus restore
  on failure). RAII handles normal cleanup and the durable journal handles process-crash recovery.
  Before IMG-6 these paths removed the destination before renaming and lost data on a failed
  finalize.
- `resolve_batch_dest_path` (3983-4011): handles skip/overwrite/rename with a `reserved` set to avoid
  intra-batch name collisions. Good.
- `prepare_metadata_backup_path` (1224-1247): backs up EXIF before in-place overwrite (jpeg/webp,
  source==dest only). Good intent — but only protects metadata, not pixels (see IMG-1).

## Recommended Fix Priority
1. IMG-1 — resolved and covered by file-level rollback tests.
2. IMG-2 — resolved with transactional refresh and cache invalidation.
3. IMG-3 — resolved by moving metadata work before finalization.
4. IMG-4 — resolved with exact-path journals; arbitrary export-root sweeping remains prohibited.
5. IMG-5 — retained by design; ImageEditor overwrite already requires explicit confirmation.

## Verification (2026-08-29)
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml edit_save_tests` — 5 passed
- `cargo test --manifest-path src-tauri/Cargo.toml t_output_temp::tests` — 5 passed
- `cargo test --manifest-path src-tauri/Cargo.toml` — 183 passed / 3 ignored
- `pnpm --dir src-vite build`

### IMG-7 [Low] Resolved — `edit_image` reports why a save failed
- Location: `src-tauri/src/t_image.rs::edit_image`, `src-tauri/src/t_cmds.rs::edit_image`
- Detail: the function returned `bool` and every failure path was a bare `return false`, so the UI
  could only show a generic "save failed" toast with no diagnosable cause.
- Impact: users and support could not tell an encode failure from a full disk or a metadata-refresh
  failure.
- Resolution: it returns `Result<(), String>` and each failure names its step (output directory,
  encode, metadata preservation, temp flush, publish, indexed-metadata refresh). `FileInfo` logs the
  reason; `ImageEditor` awaits the call and keeps its single failure notification.

## Follow-ups (not fixed)
- `cleanup_edit_backup` removes the staged original with `fs::remove_file` after a committed
  overwrite export. That is the user's explicit overwrite of the exact same path, so it is retained
  as by design (same reasoning as IMG-5) rather than routed through the trash.
