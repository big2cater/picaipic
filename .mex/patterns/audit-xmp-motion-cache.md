# Audit: t_xmp.rs (Motion Photo extraction & cache)

> Status: resolved (findings verified against current code and fixed)
> Scope: `src-tauri/src/t_xmp.rs` — Motion Photo (Live Photo) extraction, JPEG packaging, cache management
> Last reviewed: 2026-07-30
> Auditor: AI (read-only audit pass)

## Scope correction (important)
The audit brief mentioned "XMP write-back atomicity". On inspection, **`t_xmp.rs` does NOT
write XMP (or any metadata) back to user originals.** It only *reads* the embedded XMP
subsegment inside JPEG/TIFF buffers to detect/parse Motion Photo containers
(`let xmp_bytes = &buf[xmp_start..xmp_end]` at `162`, `269`). All destructive file operations
here are restricted to the **motion-photo cache directory** and the system temp dir, never to
user-owned source files. The original-file privacy/destructive-safety model is therefore NOT
at risk in this module. The remaining findings are about cache-write atomicity and cache-cleanup
safety, which are lower severity (cache is reproducible).

## Findings

### X-1 [Low] `extract_motion_video_to_cache` cache-hit check trusts size only — resolved
- Location: `src-tauri/src/t_xmp.rs:531-559`
- Detail: On cache hit it returns early when `cache_path.exists() && meta.len() >= MIN_MOTION_VIDEO_SIZE`,
  with no content validation (magic bytes / embedded metadata parse). A truncated or corrupt
  `.mp4` that is still >= the minimum size would be served as-is.
- Impact: A corrupt cached motion video could be displayed/exported. Only affects cache
  (regenerable by deleting the file). Low.
- Verification: **True in the reviewed implementation.** Cache hits now require the exact
  expected video length plus a valid ISO-BMFF `ftyp` box. Invalid/truncated entries are removed
  and re-extracted; newly extracted payloads are validated before they enter the cache.

### X-2 [Low] Cache writes are atomic (verified good) — hygiene resolved
- Location: `src-tauri/src/t_xmp.rs:531-559`
- Detail: Writes to `cache_path.tmp` then `rename` to `cache_path`. A crash mid-write leaves a
  `.tmp`, not a corrupt `.mp4`. Good pattern.
- Verification: Atomic tmp-then-rename was already good. The follow-up found that concurrent
  extraction of the same cache key also shared one fixed `.tmp` path. Writes now use a UUID temp
  path, accept a valid concurrent winner, clean failed writes immediately, and remove abandoned
  `.tmp` files after one hour.

### X-3 [Low] Legacy purge scans the system temp dir each launch — resolved
- Location: `src-tauri/src/t_xmp.rs:485-558`
- Detail: Scans the OS temp directory every app start for files matching `picaipic_motion_*.mp4`
  and removes them. The prefix is app-specific (collision risk low), but it is a full temp-dir
  traversal on every launch.
- Impact: Negligible correctness risk; minor startup cost on systems with large temp dirs.
- Verification: **True, but `read_dir` was a non-recursive single-directory scan.** Startup now
  records `.legacy-temp-purge-v1` in `motion_cache` after a successful migration sweep, so normal
  launches do not repeat it. A failed sweep is retried on the next launch.

### X-4 [Low] Cleanup skips `.tmp` and can race with an in-flight extraction — resolved
- Location: `src-tauri/src/t_xmp.rs:800-837`
- Detail: `auto_cleanup_motion_cache` ignores `.tmp` files (line 815 `continue`) and, when over
  budget, removes the oldest `.mp4` entries. Two concurrent long-press extractions can race:
  one renames `x.mp4` into the cache while the other's `auto_cleanup` (triggered by exceeding
  `MOTION_CACHE_MAX_BYTES`) removes that same `x.mp4` before the first reader consumes it.
- Impact: Rare cache-miss / re-extraction; no user-data loss. Low.
- Verification: **True.** Valid cache hits now refresh mtime, and pruning protects entries used
  in the last 15 minutes. Cleanup is restricted to `picaipic_motion_*.mp4`, so export work files
  and the migration marker are not counted or deleted. Stale `.tmp` cleanup is covered by X-2.

### X-5 [Low] `clear_motion_cache_dir` uses `remove_dir_all` on cache only
- Location: `src-tauri/src/t_xmp.rs:~561-575`
- Detail: Guarded by `if cache_dir.is_dir()` before `remove_dir_all`. Cache-only, so safe even
  if misconfigured. Good guard.
- Impact: None (cache is disposable). Noted as a control case.

## Verified Safe (control cases)
- All XMP access is read-only (parse subsegment from in-memory buffers). No original-file mutation.
- `extract_motion_video_to_cache` uses tmp-then-rename (see X-2).
- `package_motion_photo_jpeg` / `read_motion_still_bytes` return in-memory `Vec<u8>`; no disk writes.
- `clear_motion_cache_dir` guards the path before `remove_dir_all`.

## Recommended Fix Priority
All actionable findings are resolved. Regression coverage includes corrupt and truncated cache
replacement, invalid payload rejection, concurrent same-key extraction, legacy purge filtering,
and recent-entry protection. X-5 remains a verified-safe control case.
