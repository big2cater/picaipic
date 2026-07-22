---
name: change-photo-frame
description: Runbook for EXIF photo frame / info bar (classic bar, float/sink blur+shadow, optional logo).
last_updated: 2026-07-22
---

# Change photo frame / 相框

## When to use

- Change EXIF info-bar layout, fields, templates, or palettes
- Change blur float/sink canvas, soft shadow, or logo placement
- Change host EXIF summary reading (JPEG EXIF, little_exif, LibRaw)
- Change preview JPEG or multi-file save-as export / cancel / progress
- Wire new entry points or import-to-library after export
- Add G3+ (top/side magazine, batch action, logo library) — start here, do not fork a second pipeline lightly

## Touchpoints

| Area | Path |
|------|------|
| Templates / options | `src-vite/src/common/photoFrameTemplates.ts` |
| Dialog | `src-vite/src/components/PhotoFrameDialog.vue` |
| Entry | multi-select `SelectionPanel` → `Content.openPhotoFrameDialog` |
| Import after export | `Content.onPhotoFrameDone` → sequential `importFile` + album refresh |
| IPC | `api.photoFramePreview`, `exportPhotoFrame`, `cancelPhotoFrameExport` |
| Host | `t_image::{photo_frame_preview, export_photo_frame, cancel_photo_frame_export, apply_photo_frame, read_frame_exif_summary}` |
| Layout helpers | `frame_layout_kind`, `make_cover_blur_bg`, `make_soft_shadow`, `load_frame_logo`, `place_frame_logo`, `draw_frame_info_bar`, `fit_frame_text` |
| Register | `t_cmds.rs`, `main.rs` |
| Progress event | `photo-frame-progress` |
| i18n | `photo_frame.*`, `info_panel.photo_frame` |

## Product rules

- Local-only; no cloud EXIF or remote render.
- **Save-as only** — never overwrite originals by default.
- Layout templates: `classic-white`, `classic-black`, **`float-blur`**, **`sink-blur`**.
- **Custom presets (2026-07-22):** full option snapshots in Pinia `config.photoFrame.presets` (same pattern as batch templates). Dialog: save / apply / delete via MessageBox + `ask` (no `window.prompt`/`confirm`).
- Field toggles: brand, model, lens, focal, aperture, shutter, ISO, datetime.
- Style: bar height, outer margin / canvas pad, colors (classic); blur sigma, shadow blur/offset/opacity (blur layouts).
- **Default logo:** `showLogo` defaults true; empty `logoPath` → host resolves bundled `resources/branding/default-frame-logo.png` (from repo `logo-pic.png` / black wordmark). User can pick another file or clear back to default.
- **Do not reuse frame logo as the Windows app icon. Regenerate app icons with `scripts/regenerate_app_icons.ps1` (from `favicon1.ico`), then **clean rebuild** so `icon.ico` is re-linked into the EXE (`build.rs` now has `rerun-if-changed` on icon assets).** App chrome icons are the neural-cat mark from `docs/public/icon.png` → `src-tauri/icons/*` (+ `src-vite/src/assets/images/icon.png`). Regenerating frame branding must not overwrite `src-tauri/icons/icon.ico` / `icon.png`.
- Logo positions: **`bar-center` (default)**, top-left, top-right. Legacy bar-left/bar-right map to center.
- **Preview perf:** `photo_frame_preview` uses `load_image_for_layout_cached`, EXIF summary cache, logo resize cache, and system-font cache; UI maxEdge ~1000.
- Optional import into the open album (copy outputs; unique names).
- App Windows / taskbar / title-bar icons use the **neural-cat** mark from repo-root `favicon1.ico` → `src-tauri/icons/*` + `src-vite/src/assets/images/icon.png` + `docs/public/icon.png`. Tauri `bundle.icon` points at `icons/icon.png` + `icons/icon.ico`. **Frame default logo** is separate: `resources/branding/default-frame-logo.png` (black wordmark from `logo-pic.png`) — never overwrite app icons with the frame logo.

## Host layout

### Classic bar
1. Load image (`load_image_for_layout` preview / export decode capped at 8192).
2. `read_frame_exif_summary` — **one head open** (512KB): kamadak from bytes + little_exif `new_from_vec` on same buffer; RAW uses LibRaw only; TIFF edge may still open LibRaw.
3. Solid canvas + optional margin + bottom bar + four corner strings (`ab_glyph` + system font).

### Float / sink blur
1. Cover-blur background (`make_cover_blur_bg`: downsample → blur → upsample) + slight dim.
2. Soft drop shadow under the photo (`make_soft_shadow`).
3. **Float:** photo vertically centered-ish above bar; shadow offset mild.
4. **Sink:** photo biased to top; larger bottom pad; bar sits in the lower blur zone (not glued to photo); translucent darken strip under text for readability.
5. Optional logo overlay after bar text.

### Info bar text fit (2026-07-22)
- Left/right columns each get half of `(out_w - 2*pad_x - min_center_gap)`.
- `fit_frame_text`: shrink font down to ~55% desired, then ellipsis-truncate.
- `min_size = (size * 0.55).max(1.0).min(size)` — never `clamp(lo, hi)` with lo>hi.
- Never let right-aligned text start negative or collide with left column.
- Layout ids: exact `sink-blur` / `float-blur` only (no substring `blur`).
- Style sliders UI min/max match host clamps (`barRatio` 0.05–0.22, etc.).
- Export decode: `load_image_for_layout(src, 8192)` — no full-res then downscale peak.

### Datetime
- `format_frame_date_time` accepts EXIF `YYYY:MM:DD HH:MM:SS` and ISO `YYYY-MM-DDTHH:MM:SS[.fff][Z|±HH:MM]` → `YYYY-MM-DD HH:MM:SS`.

### Export path (2026-07-22)
- Preview: JPEG bytes, long edge clamp.
- Export: serial `resolve_batch_dest_path` + `JoinSet`.
- **Workers:** `photo_frame_export_worker_limit()` = `min(batch_worker_limit(), 2)` (full-res canvas memory).
- **Source scale:** long edge > 8192 downscaled before `apply_photo_frame`.
- **Atomic write:** `{dest}.picaipic-batch.tmp` then rename; cancel/error only removes temp (`remove_batch_temp`).
- Single `to_rgba8()` in `apply_photo_frame` for photo + blur source.

## Options (camelCase IPC)

`templateId`, field bools, `barRatio`, `marginRatio` (pad on blur), `backgroundColor`, `textColor`, `secondaryTextColor`, `blurSigma`, `shadowBlur`, `shadowOffsetRatio`, `shadowOpacity`, `showLogo`, `logoPath`, `logoScale`, `logoPosition`.

## Explicit non-goals (current)

- Full photix Canvas processor port
- Built-in brand logo asset pack / logo library UI (user picks a local file only)
- Batch action `photoFrame` in the batch wizard (G3)
- Top/side magazine layouts (still follow-up)
- ImageEditor embedded tab

## Verify

- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml photo_frame -- --nocapture`
- Manual: multi-select → 相框; classic white/black; float-blur / sink-blur
- Manual: tune fields/style/logo → **保存当前为相框** → re-open dialog → **应用** preset without re-tuning
- Manual: logo empty + showLogo → default branding logo appears; pick custom; clear back to default
- Manual: long brand+model + full EXIF right column on narrow bar — no overlap, ellipsis if needed
- Manual: blur/shadow/pad sliders update preview; logo pick + positions
- Manual: image with full EXIF vs none (empty corners, no crash)
- Manual: multi export jpg/png; cancel mid-run leaves no `*.picaipic-batch.tmp` / half files; originals untouched; optional import
- Manual: cancel mid-export
- Packaging: Windows installer/taskbar icon reflects new `src-tauri/icons/icon.ico`

## Related

- Roadmap Phase G: `docs/guide/builtin-tools-roadmap.md`
- Batch dest/name helpers: `change-batch-process.md`
- Print/collage dialog entry pattern: `change-print-layout.md`, `change-collage.md`
