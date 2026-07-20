---
name: change-crop-presets
description: Runbook for ImageEditor crop aspect ratios, print/ID photo size presets, and custom favorite ratios.
last_updated: 2026-07-18
---

# Change crop presets (Phase A)

## When to use

- Add/edit built-in print or ID photo sizes
- Change common ratio list (`1:1`, `3:2`, …)
- Fix crop aspect locking, portrait/landscape swap, or resize prefill for photo presets
- Change how user custom ratios are stored

## Touchpoints

| Area | Path |
|------|------|
| Catalog + math | `src-vite/src/common/photoSizePresets.ts` |
| Persist | `src-vite/src/stores/configStore.js` → `imageEditor.cropPresetId`, `customCropRatios` |
| Crop UI | `src-vite/src/views/ImageEditor.vue` |
| Dialogs | `PhotoSizeManageDialog.vue`, `AddCustomCropRatioDialog.vue` |
| i18n | `src-vite/src/locales/en.json`, `zh.json` under `msgbox.image_editor` |
| Product plan | `docs/guide/builtin-tools-roadmap.md` |

## Rules

- Crop UI enforces **aspect** from the preset; photo “像素要求” is target output (resize prefill + hint).
- Store user favorites in **app config** (not per-library).
- Keep built-in catalog read-only in the manage dialog; only custom ratios are deletable.
- Migrate legacy numeric `cropShape` via `migrateLegacyCropShape` when `cropPresetId` is missing.
- No required network; do not route through AI plugins.

## Performance notes (2026-07-18)

- Crop drag: snapshot `getBoundingClientRect` once per gesture; throttle moves with `requestAnimationFrame`; map crop pixels without remeasuring every frame (`updateCropFromCropBox({ refreshRects: false })`).
- Config: do not re-`normalizeCustomCropRatios` on every computed read after load/ensure.
- Save path (`t_image::get_edited_image`): heavy downscale uses Triangle; mild scale uses CatmullRom; brightness/contrast/(sat) fused into one pass when blur/hue absent.

## Verify

- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml` when touching `t_image.rs`
- Manual: open ImageEditor → crop → switch free / ratio / photo size → portrait toggle → add & delete custom ratio → drag crop handles smoothly → apply/cancel crop still works
