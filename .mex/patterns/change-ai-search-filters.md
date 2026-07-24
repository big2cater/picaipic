---
name: change-ai-search-filters
description: File-type filters and result grouping for AI / similar / filename search.
last_updated: 2026-07-24
---

# Change AI search filters & grouping

## When to use
- Add/change file-type filtering on semantic / similar search
- Change search result section headers (visual / similar / filename)
- Wire toolbar filter into temporary similar-from-file mode
- Change CLIP ranking floors, free-text template, embed source ladder, or calibrated thresholds (also touch `change-image-search-model.md` / `change-smart-tags.md` as needed)

## Key files
| Layer | Path |
|-------|------|
| Params | `t_sqlite.rs` `ImageSearchParams.search_file_type` |
| Vector SQL | `AFile::search_similar_images` (+ `build_file_type_condition`) |
| Frontend call | `Content.vue` `getImageSearchFileList` / `currentImageSearchParams` |
| Toolbar | `Content.vue` file-type `DropDownSelect` enabled in search-like views |
| Similar temp | `Content.vue` watch branch for `tempViewMode === 'similar'` |
| Group header | `GridView.vue` `sectionLabel` / `sectionHeaderEnabled` |
| i18n | `search.group_visual` / `group_similar` / `group_filename` |

## Behaviour contract
1. Mask matches library filter: `0` all, `1` image, `2` video, `4` raw, **`8` LIVE/Motion still** (`live_photo_type` IN 1,3,4; companion MOV type 2 still list-excluded). Combine with OR.
2. AI search applies filter in SQL **before** cosine scoring (embeddings candidates only).
3. Filename search continues via `QueryParams.search_file_type` (already present).
4. Toolbar type filter is enabled in search sidebar and similar temp view; sort remains disabled for AI results.
5. Result lists show one section header when not date-grouped: Visual / Similar / Filename.
6. Changing type filter re-runs active AI/filename/smart-tag/similar queries.
7. **Cosine ranking (2026-07-24 fix; image-image 2026-07-24):** slider owns the primary cut.
   - Collect candidates with junk floor `score >= 0.16`.
   - Sort desc.
   - **Text search primary cut:** `absolute_floor = max(0.16, settings_thr * 0.85)`.
   - **Similar-from-file (image→image)** uses a separate ladder — CLIP image-image scores are ~0.55–0.95, so text floors (0.14–0.24) never differentiate Low vs Very High:
     - floors: VH `0.88` / H `0.82` / M `0.74` / L `0.62` (`image_image_absolute_floor`)
     - thr_cap: VH 12 / H 24 / M 40 / L 100 (`image_image_top_k`)
     - exclude query `file_id` (self cosine≈1.0)
     - rel fallback uses `top1*0.92` (tighter than text)
   - **Relative floor** (text) `top1 * 0.85` only if abs cut empties a non-empty set (then all_fallback).
   - **Top-K thr_cap (text)** by thr: VH≥0.27→30, H≥0.23→40, M≥0.19→50, else Low→200. User `limit` is a **hard cap** for all tiers.
   - Never force `0.25` just because `search_text` is non-empty.
7b. **Settings re-run:** changing `thresholdIndex` / `limit` re-runs active similar temp view, search sidebar (text/similar), and smart-tag view (`Content.vue` watch).
8. Slider values (**histogram-calibrated 2026-07-23**, CLIP **text** scale): Very High `0.28` / High `0.24` / Medium `0.20` / Low `0.16`. **Default index = 1 (High)** in `configStore` (new installs); saved user settings win. Same UI index maps to image floors above for similar-from-file. **Smart tags use the same `thresholdIndex`** (text ladder). Re-calibrate with `scripts/calibrate_search_thresholds.py` after big library/model change. Prompts: short CLIP-style in `smartTags.ts`.
8b. **Free-text template:** short bare EN labels (`bird`, `cat`) → `a photo of a/an {label}` in `AiEngine::normalize_clip_text_query` before encode. Long phrases / CJK / already-templated left as-is. Log `preview=` is 40 chars display-only; `templated=` / `floor_mode=` / `thr_cap=` in host log.
8c. **Calibration evidence (owner logs):** bird max≈0.277 / landscape≈0.283; `>0.28`≈0–1; family with people max≈0.255; empty-concept max≈0.21. Old VH 0.30 emptied essentially everything.
9. **Embed source (2026-07-23 quality+scan pack, refined):**
   - **Scan pipeline:** thumb permit released **before** CLIP embed; folder-sync embed fire-and-forget (same ladder).
   - **Decode outside AiEngine mutex:** `t_image::load_image_for_clip_embed` then `encode_image_from_dynamic` (I/O/decode parallel-capable; ONNX still single-session locked).
   - **JPEG:** libjpeg-turbo **scaled** decode to `EMBED_SOURCE_MAX_EDGE` (1024) — not full-res `image::open`.
   - **RAW (type 3):** LibRaw preview @ 1024 (`used=raw_preview`); UI thumb last.
   - **Other:** open + longest-edge cap (`used=original_capped`).
   - **Embed semaphore = 1** (honest: one engine lock; multi-engine later if needed).
   - Shared constant: `t_common::EMBED_SOURCE_MAX_EDGE`.
10. **Tokenizer:** CLIP/default text tower enables **truncation max 77** after load (JSON had null).
11. **Diagnostics:** one `search_similar … thr=… candidates=… hit=… max=… >0.18/… top3=` line per query (console). Log `text=` is **preview only** (`chars().take(40)`); full prompt is encoded (CLIP trunc max 77 tokens). Do not treat log half-lines as encode truncation. Embed logs: `used=raw_preview|original_capped|thumbnail`.
12. **Owner check (2026-07-23):** empty library section → family hit=0 is correct. After adding people photos: family max~0.255 hit=2 @0.22, **both correct**; portrait hit=14 usable. Fixed 0.22 is OK when content exists; empty ≠ always threshold bug.

## Verify
```bash
cargo check --manifest-path src-tauri/Cargo.toml
pnpm --dir src-vite build
```
Manual: AI search → set Image only → fewer/no videos; Similar from file → same; result header label matches search mode.
After embed-path change: clear library embeds or force reindex so new originals replace old thumb embeds; compare text search to `scripts/compare_clip_vs_siglip2.py` on same album.
