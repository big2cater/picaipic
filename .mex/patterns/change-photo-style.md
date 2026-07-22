---
name: change-photo-style
description: Unified adjust recipes (presets + manual + LUT) with layered CSS/host preview.
triggers:
  - photo style
  - 照片格调
  - lut library
  - filter library
  - cube apply
  - image editor presets
edges:
  - target: patterns/change-color-match.md
    condition: when style LUTs are produced from color match export
  - target: context/architecture.md
    condition: when placing photo style in the edit pipeline
last_updated: 2026-07-21
---

# Change Photo Style + LUT Library (unified presets)

## Scope
Host-built-in adjust recipes (legacy CSS looks + Panasonic-like styles) and LUT library.
ImageEditor no longer has a separate photo-style panel: recipes live under Presets + Manual.

## Model
A recipe = base params + effects + optional library LUT:
- base (CSS-capable): brightness, contrast, saturation, hue, blur, filter
- effects (host): fade, vignette, grain, highlights, shadows
- LUT: library id + intensity 0-100

Apply order on host: base then LUT then effects in t_lut apply_photo_style.

## Preview layering
| Path | When | How |
|------|------|-----|
| CSS fast | no host-only fields | browser CSS filter on the img |
| Host | highlights/shadows/fade/vignette/grain/lut non-neutral | apply_photo_style_preview maxEdge 1200 blob URL; CSS filter disabled while host preview active |

## Surfaces
| Surface | Path |
|---------|------|
| Recipe model | src-vite/src/common/photoStylePresets.ts |
| Editor UI | ImageEditor.vue presets strip + manual effects/LUT |
| LUT manager | LutLibraryDialog.vue |
| Host apply/preview | t_lut.rs, t_image.rs photoStyle on edit_image |
| Batch | photoStyle action (unchanged recipe payload) |
| Config | imageEditor.photoStyles, activePhotoStyleId, expanded custom |

## Verify
- pnpm --dir src-vite build
- Manual: CSS-only preset is instant; nostalgic/cinematic or LUT shows host spinner; save-as custom appears in presets

## UX notes (post-merge polish)
- LUT row is two-line (name, then pick/clear) so narrow sidebars do not clip Chinese labels.
- Custom presets preserve config array order; live edit does not bump sort keys.
- Save-as prepends a new custom recipe; switching customs must not reshuffle the strip.
- Keyword: stable custom order; two-line LUT row.

## Perf notes
- Host interactive previews use load_image_for_layout_cached (LRU by path/maxEdge/mtime, cap 6).
- Color-match preview accepts optional photoStyle and applies match then style (same as edit_image).
- Frontend: when both color match and host style fields are active, one combined colorMatchPreview call owns the canvas.
- lutIntensity 0 with lutId does not force host preview.
- Frontend previewMaxEdge scales with editor viewport (720-1400); debounce ~280ms.
- Histogram samples host preview URL when match/style bake is active (blur-only CSS stack); shows a short hint.
- Host PREVIEW_JPEG_CACHE (LRU ~10) keys style/match params + file mtime fingerprint to skip re-apply/re-encode.
- Frontend short-circuits identical host preview fingerprints in-session; invalidated on file change / reset / clear match.
- Histogram crop maps into host-preview pixel space via long-edge maxEdge scale.

## Geometry-aware host preview
- Interactive previews accept optional geometry (flip/rotate/crop + fullWidth/fullHeight).
- Host order matches save: orient decode → flip/rotate/crop → downscale → color match? → photo style → JPEG.
- Frontend suppresses CSS flip/rotate when showing host raster; crop-baked host uses crop box display size.
- Compare view: left keeps full base geometry; right uses host bake when present.
- Crop precision: when crop is set, decode budget scales so the crop long edge ≈ preview maxEdge (up to 8192).
- Compare view: both panes share a crop-aligned window; before offsets full base by -crop, after uses host-baked crop raster when present.

## Geometry-aware host preview
- Optional geometry on photo-style / color-match preview (flip/rotate/crop + fullWidth/Height).
- Decode budget scales when crop is set so crop long edge approx maxEdge (cap 8192).
- Keyword: PreviewGeometry; decode cap 8192.
