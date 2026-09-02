# Live Photo / Motion Photo

PicAiPic can detect hybrid still+motion captures, preview them with a long-press gesture, export or convert them, and repair metadata on already-indexed albums without a full re-scan.

Your **original library files are not modified** by normal export/convert flows. The only deliberate exception is an **optional, confirmed** “replace original still” action for JPEG keyframes (see below).

## Supported formats

| Type | How it appears on disk | `live_photo_type` | What PicAiPic does |
|------|------------------------|-------------------|--------------------|
| **Apple Live Photo** | Still image (HEIC/JPEG) + companion video (usually MOV) | `1` image, `2` video | Pair by ContentIdentifier UUID; same-folder same-name stem as fallback |
| **Google Motion Photo** | Single JPEG with MP4 bytes after the image, described in XMP | `3` | Detect offset/length; extract video to app cache for preview/export |
| **HEIC-internal video** | Motion stored inside one HEIC container (item or sequence track) | `4` | Detect with libheif; extract via item data or ffmpeg demux (Windows/Linux) |

Paired Apple MOV files stay **visible as normal videos** in the library. They are linked to the still via `paired_file_id` but are not hidden.

## Preview (long-press)

1. Open a still in the main viewer (MediaViewer).
2. Long-press (about **400ms**) on the image to play motion.
3. Release to stop and return to the static image.

Requirements:

- The file must be scanned with Live Photo metadata populated.
- Apple Live Photos need a successful pair (UUID or stem) so the companion MOV can be found.
- Motion Photo / HEIC-internal video is extracted on demand into  
  `app_cache_dir()/motion_cache/` (shared cleanup with **Clear video cache**).

UI hints:

- Grid **LIVE** badge on thumbnails when `live_photo_type > 0`.
- File info panel type label: **Live Photo**, **Motion Photo**, or **HEIC Live**.

## Export and convert

Open export from:

- Right-click menu → **Export Live Photo…**
- File info panel → download icon next to the Live Photo type label

### Modes

| Mode | Output | Typical use |
|------|--------|-------------|
| **Still only** | Image file | Save a static frame without motion |
| **Video only** | MOV/MP4 | Save the motion clip alone |
| **Still + video pair** | Folder with matching stems | Share as separate files |
| **Convert to Motion Photo** | Single JPEG (Apple pair → embedded MP4) | Pack into one Motion Photo-style file |
| **Convert to still + video** | JPEG + video from a Motion Photo | Split a Motion Photo |
| **Export keyframe still** | JPEG from the motion timeline | Pick a moment as a new still |

Options:

- **Replace existing files at the destination** — destination conflict policy for export paths.
- **Replace original still in library** (keyframe mode only, JPEG stills only) — see next section.

Conversion notes:

- Apple ↔ Motion conversion aims for “re-openable in PicAiPic and common players,” not full fidelity with iOS/Google Photos proprietary rules.
- HEIC stills used as sources may be re-encoded to JPEG when packaging Motion Photos.

## Replace original still with a keyframe

Available when:

- Mode is **Export keyframe still**, and  
- The library still is a **JPEG** (Apple Live JPEG or Motion Photo).

Not available for **HEIC** stills / HEIC-internal type `4` (would require rewriting a HEIC container). Export a **new** JPEG keyframe instead.

Safety:

1. Checkbox **Replace original still in library**.
2. System **warning confirmation** dialog.
3. Staged write next to the original; on failure the previous still is restored.
4. For Motion Photos, the embedded video trailer is **kept** and the file is re-packaged.

After success, PicAiPic refreshes the current file’s thumbnail/preview.

## Repair Live Photo metadata (no full re-index)

If older albums were scanned before Live Photo support (or HEIC-internal detection), use:

**Album list → right-click album → Repair Live Photo metadata**

This runs `rescan_live_photo_metadata`:

- Re-checks candidates with `live_photo_type` **0** or **4**
- Detects Motion Photo XMP, HEIC-internal video, and ContentIdentifier where cheap
- Rebuilds pairs (`pair_live_photos`)

It does **not** re-thumbnail or re-embed the whole album. Use a normal album scan when files are new or missing entirely.

## Cache and cleanup

- Motion / HEIC extracts: `{app cache}/motion_cache/`
- Reuse is source-keyed; old system-temp `picaipic_motion_*.mp4` files are purged on startup
- Soft size cap with oldest-first prune
- Cleared when you clear the video cache from the app

## Limitations

- **HEIC sequence tracks** that ffmpeg cannot demux may fail extract (no frame-by-frame re-encode path yet).
- **ffprobe** must resolve Apple content identifiers (dotted and underscored keys are both tried).
- **macOS** is not a release target; libheif HEIC-internal paths are Windows/Linux-oriented.
- Interoperability of converted Motion Photos with every phone OEM gallery is not guaranteed.

## For developers

- Schema: library DB `PRAGMA user_version` ≥ 6; columns `content_id`, `paired_file_id`, `live_photo_type`
