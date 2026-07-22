---
name: change-color-match
description: Traditional global Lab color match (追色) plus single-image style .cube LUT export in the host ImageEditor and batch.
triggers:
  - color match
  - 追色
  - traditional color transfer
  - Lab match
  - cube lut
  - export lut
  - style lut
edges:
  - target: context/conventions.md
    condition: when changing IPC or UI patterns
  - target: patterns/add-tauri-command.md
    condition: when adding or renaming host commands
  - target: context/architecture.md
    condition: when placing color match in the media/edit pipeline
last_updated: 2026-07-22
---

# Change Traditional Color Match (追色) + Style LUT

## Scope
Host-built-in **global Lab statistics match** and **single-image style `.cube` export**.

- No segmentation / SAM / FaceParser
- No OpenCV on host
- Not the AI plugin runtime
- Not cancelled SA-LUT **G7** `export-lut`

Reference algorithm base: Photo Color Match `color_transfer.py` soft-global path only.

## Two different features

| Feature | Inputs | Output | Purpose |
|---------|--------|--------|---------|
| **Color match apply** | current/target image + reference image + params | graded pixels (preview JPEG / saved image / batch file) | Transfer reference look onto target |
| **Style LUT export** | **one** image (reference if chosen, else current) | 33³ `.cube` | Bake that image Lab look into a reusable LUT |

Do **not** re-merge these into a dual-image match-map LUT unless product explicitly changes. Owner intent (2026-07-21): LUT = single-image style bake.

## Surfaces
| Surface | Path |
|---------|------|
| Algorithm | `src-tauri/src/t_color_match.rs` |
| Edit / batch / export wiring | `src-tauri/src/t_image.rs` |
| IPC | `color_match_preview`, `export_color_match_lut`, `edit_image` (`colorMatch`), batch `colorMatch` |
| API | `colorMatchPreview`, `exportColorMatchLut` in `src-vite/src/common/api.js` |
| Editor UI | `ImageEditor.vue` adjust tab -> 追色 panel |
| Batch | `batchProcess.ts`, `BatchProcessDialog.vue` |
| Product plan | `docs/guide/builtin-tools-roadmap.md` Phase E |
| Progress | `docs/guide/picaipic-progress.md`, `docs/guide/目前的开发情况.md` |

## Apply pipeline
1. Load target + reference (`load_image_for_layout` / edit loaders)
2. Optional auto WB on target
3. Soft global Lab median + 16-84% width match (L weak, chroma stronger)
4. Blend with intensity x 0.65
5. Highlight / shadow protect + tone preservation
6. Editor save order: geometry -> **color match** -> CSS-style brightness/contrast/...

### Perf / memory (2026-07-22)
- **Stats only on downsampled images** (`STATS_MAX_EDGE = 1024`) for both target and reference. Full-res median/percentile sorts on 50MP+ were multi-second and multi-GB.
- **Single full-res f32 buffer**: one in-place pixel pass (WB → Lab grade blend → protect → tone). Do **not** reintroduce parallel `original` / `global_grade` / `result` planes.
- Channel quantiles: one sort per channel → (median, p16, p84); no triple full sorts.
- Lab a/b clamp is **full 0..255** OpenCV 8-bit Lab (not 72..186); saturated references must keep chroma (`saturated_reference_keeps_chroma` test).
- Gray-world WB scales are estimated from the downsampled target sample, then applied full-res.

### Params
- UI percents 0-100 -> host 0-1: `intensity`, `tonePreservation`, `highlightProtection`, `shadowProtection`
- `autoWb` bool
- Batch requires `referenceFilePath`

### Entry points
- **Editor:** 编辑图片 -> **调色** -> **追色** (not multi-select panel)
- **Batch:** 多选 -> 批处理 -> action `colorMatch`

## Style LUT export
- Command: `export_color_match_lut`
- Params: `sourceFilePath`, `destFilePath`, optional `lutSize` (default **33**)
- Size **must be 17–65 inclusive**; out-of-range returns `Err` (no silent clamp)
- Implementation: `build_style_cube_from_image` / `write_style_cube_from_image`
- Neutral sRGB lattice graded toward the **style image** Lab stats (soft blend 0.65)
- Adobe/Resolve `.cube`: `LUT_3D_SIZE`, R G B 0..1, B outer / G mid / R inner
- Editor: export button uses reference path if set, else current file
- Filename default: `{stem}_style_33.cube`

## Rules
- Local-only; never upload media
- Preview uses downscaled host JPEG; do not overwrite crop/save dimensions from preview loads
- Keep host traditional LUT separate from SA-LUT plugin / G7
- No `window.prompt` / `window.confirm` for save path — use `@tauri-apps/plugin-dialog` `save`

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml color_match`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `pnpm --dir src-vite build`
- Manual:
  1. Editor -> 调色 -> 追色 -> pick reference -> preview -> save
  2. Export 33 style `.cube` from reference (or current image without reference)
  3. Batch `colorMatch` with shared reference
  4. High-res / high-sat reference: no multi-minute hang; chroma retained
