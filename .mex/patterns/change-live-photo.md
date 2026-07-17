---
name: change-live-photo
description: Support Apple Live Photo and Google Motion Photo detection, pairing, and long-press preview.
triggers:
  - live photo
  - motion photo
  - LivePhoto
  - content identifier
  - paired video
  - long press preview
edges:
  - target: context/architecture.md
    condition: when tracing the media/AI pipeline or Tauri command flow
  - target: context/decisions.md
    condition: when revisiting the Live Photo support decision or alternatives
  - target: context/conventions.md
    condition: when following coding, IPC, or verify conventions
  - target: context/stack.md
    condition: when checking quick-xml, EXIF crate, or FFprobe dependency details
last_updated: 2026-07-17
---

# Change Live Photo / Motion Photo Features

## Context

PicAiPic supports two hybrid still+motion photo formats:

- **Apple Live Photo** — two separate files: an image (HEIC/JPEG) and a companion video (MOV). They are paired by matching EXIF ContentIdentifier (tag 0x0011, `Context::Tiff`) on the image with the MOV's `com.apple.quicktime.content.identifier` metadata (read via ffprobe).
- **Google Motion Photo** — a single JPEG file with a video segment (MP4) appended after the image data. XMP metadata (`GCamera:MotionPhoto=1` and `Container:Directory`) records the video offset and length. `t_xmp.rs` parses XMP using `quick-xml`.

The `afiles` table (schema v6) has three columns: `content_id` (UUID, `motion:<offset>:<length>`, or `heifitem:`/`heifseq:`), `paired_file_id` (bilateral link), and `live_photo_type` (0=none, 1=Apple image, 2=Apple video, 3=Google Motion Photo, 4=HEIC-internal video).

## Key Code Locations

| Concern | File | Symbol/Location |
|--------|------|-----------------|
| DB migration v6 | `t_migration.rs` | `check_and_migrate` v6 branch |
| AFile struct + SQL | `t_sqlite.rs` | struct ~line 826, `insert`/`update`/`from_row`/`build_base_query` |
| Pair scan | `t_sqlite.rs` | `AFile::pair_live_photos()` |
| Image EXIF ContentIdentifier | `t_sqlite.rs` | `AFile::new` image branch |
| Video ffprobe content identifier | `t_video.rs` | `VideoMetadata.content_id`, `get_video_metadata_async` |
| XMP / Motion Photo detection | `t_xmp.rs` | `detect_motion_photo`, `parse_motion_content_id`, `extract_motion_video_to_cache`, `motion_cache_dir`, `init_motion_cache`, `clear_motion_cache_dir` |
| Export orchestration | `t_live_photo.rs` | `export_live_photo` (modes: still / video / pair / to_motion / to_pair / set_keyframe) |
| Motion packaging | `t_xmp.rs` | `package_motion_photo_jpeg`, `read_motion_still_bytes` |
| FFmpeg helpers | `t_video.rs` | `remux_or_transcode_to_mp4`, `remux_with_content_id`, `extract_keyframe_jpeg` |
| HEIC internal video | `t_heif.rs` | `detect_heic_embedded_video`, `extract_heic_embedded_video_to_cache` (items → sequences/ffmpeg) |
| Tauri commands | `t_cmds.rs` | `get_paired_video`, `extract_motion_video`, `rebuild_live_photo_pairs`, `export_live_photo` |
| Command registration | `main.rs` | `invoke_handler` (append to `generate_handler!`) |
| Frontend API | `src-vite/src/common/api.js` | `getPairedVideo`, `extractMotionVideo`, `rebuildLivePhotoPairs`, `exportLivePhoto` |
| Frontend types | `src-vite/src/common/types.ts` | `PairedVideoInfo`, `ExportLivePhotoOptions`, `ExportLivePhotoResult` |
| MediaViewer preview | `src-vite/src/components/MediaViewer.vue` | long-press handlers + `<video>` overlay + LIVE badge |
| Thumbnail badge | `src-vite/src/components/Thumbnail.vue` | `isLivePhoto` computed + LIVE badge |
| FileInfo label | `src-vite/src/components/FileInfo.vue` | `livePhotoLabel` computed |
| Export UI | `LivePhotoExportDialog.vue`, `fileMenu.ts`, `Content.vue` | right-click Export Live Photo → still/video/pair |

## Steps

1. **Schema change** → add a new migration version in `t_migration.rs` (follow patterns/change-database-schema.md). Update AFile struct, insert/update/select SQL, `from_row` indices.
2. **Backend detection**:
   - Image side: read `Tag(Context::Tiff, 0x0011)` with kamadak-exif; fallback to `scrape_ascii_from_tag(data, 0x0011)`.
   - Video side: add `content_id` to `VideoMetadata`; extract from ffprobe `format.tags` and stream-level `tags` via `first_exist` (try both dotted `com.apple.quicktime.content.identifier` and underscored variant).
   - Motion Photo: use `t_xmp::detect_motion_photo(file_path)` in the image branch; encode offset as `motion:<offset>:<length>` in `content_id`, set `live_photo_type=3`.
3. **Pairing** → `AFile::pair_live_photos(album_id)` runs in `index_album_worker` after mark-and-sweep cleanup. Two strategies: (1) match by `content_id` UUID (Apple), (2) file-name stem fallback (same folder, same stem, different extension).
4. **Tauri commands** → register in `t_cmds.rs` and `main.rs` `invoke_handler`.
5. **Frontend** → add API wrappers in `api.js`, types in `types.ts`. In MediaViewer: watch `file.id`, call `getPairedVideo`, set up 400ms long-press timer, play paired/Motion-extracted video on a controlless `<video>` overlay, stop on pointer up/leave. Add LIVE badge to Thumbnail and FileInfo.

## Gotchas

- ffprobe may preserve or normalize dotted Apple metadata keys; always check both `com.apple.quicktime.content.identifier` and `com_apple_quicktime_content_identifier` variants.
- `AFile::get_file_info` uses `build_base_query()` which populates `file_path` via folder+name join — do not construct paths manually in commands.
- The `file_path` field on `AFile` is output-only (populated by `from_row`, set to `None` in `AFile::new`).
- File-name stem pairing uses `Path::file_stem()` which strips only the last extension; `IMG_1234.tar.gz` would stem to `IMG_1234.tar`, but media files rarely have double extensions.
- Motion Photo extracts go to `app_cache_dir()/motion_cache/` with source-keyed names (`picaipic_motion_{fnv(path)}_{size}_{mtime}_{offset}_{length}.mp4`). Cache hits skip re-read; writes use `*.tmp` then rename. Soft cap 2GB → prune to ~1.4GB (oldest first). Startup purges legacy OS-temp `picaipic_motion_*.mp4`. `clear_video_cache` also clears `motion_cache`.
- Always parse `motion:<offset>:<length>` via `t_xmp::parse_motion_content_id` — do not reimplement in `t_cmds` / `t_live_photo`.
- MediaViewer must use optional chaining on `props.file` in toolbar class and Live Photo playback; long-press timers can fire after the file is cleared.
- The long-press overlay sits at `z-60` — higher than the Image component but below the toolbar; verify it doesn't block toolbar interactions.
- `scrape_ascii_from_tag` reads from the 128KB file header pre-read buffer, which may not contain ContentIdentifier for all files (especially HEIC where metadata is deeper). The kamadak-exif `get_field` path should be the primary reader.

## Verify

- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] `pnpm --dir src-vite build`
- [ ] Scan an album with Apple Live Photo (HEIC+MOV) → confirm `live_photo_type=1` on image, `=2` on MOV, `paired_file_id` set on both.
- [ ] Scan an album with Google Motion Photo JPEG → confirm `live_photo_type=3`, `content_id` starts with `motion:`.
- [ ] Scan HEIC with internal video (Win/Linux + libheif) → `live_photo_type=4`; long-press extracts to motion_cache.
- [ ] Long-press an Apple Live Photo image in MediaViewer → video plays, release stops.
- [ ] Long-press a Motion Photo in MediaViewer → embedded video extracts and plays, release stops.
- [ ] Thumbnail shows LIVE badge; FileInfo shows "Live Photo" / "Motion Photo" label.
- [ ] Export still/video/pair (and conversion modes if used) does not modify library originals.
- [ ] Existing non-Live-Photo files scan/preview normally (no regression).

## Debug

- `PRAGMA user_version` should be 6 after migration. Check `PRAGMA table_info(afiles)` for `content_id`, `paired_file_id`, `live_photo_type` columns.
- Run `ffprobe -v quiet -show_format -show_streams -of json <mov_file>` to confirm the ContentIdentifier key name.
- Inspect XMP: search for `<x:xmpmeta` in the JPEG file with a hex editor or `grep`.
- If pairing fails, check `SELECT id, name, content_id, live_photo_type, paired_file_id FROM afiles WHERE content_id IS NOT NULL`.
- Motion Photo extraction: verify `t_xmp::extract_motion_video_to_cache` writes a valid MP4 under `app_cache_dir()/motion_cache/` and reuses it on second extract.

## Update Scaffold

- [ ] Update `context/architecture.md` if pipeline boundaries change.
- [ ] Update `context/stack.md` if Media/EXIF/XMP libraries change.
- [ ] Update `context/decisions.md` if the Live Photo support decision is revisited.
- [ ] Log new risks or follow-ups with `mex log`.