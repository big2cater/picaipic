---
name: change-smart-tags
description: CLIP smart-tag categories, short prompts, and per-tag search threshold.
last_updated: 2026-07-24
---

# Change smart tags (CLIP zero-shot categories)

## When to use
- Add/rename/remove smart-tag categories or prompts
- Change smart-tag categories/prompts or how they call `search_similar_images` (must follow settings thr)
- Align smart tags with ranking floors / free-text template after model or calibration work

## Key files
| Layer | Path |
|-------|------|
| Categories + threshold | `src-vite/src/common/smartTags.ts` |
| Labels (i18n) | `src-vite/src/locales/en.json`, `zh.json` (`tag.smart_items`) |
| Search call | Content / smart-tag UI → `search_similar_images` with thr |
| Ranking / floors | `t_sqlite.rs` `search_similar_images` — see `change-ai-search-filters.md` |
| Free-text template | `t_ai.rs` `normalize_clip_text_query` (smart tags usually already short EN phrases) |

## Current product set (6)
| id | prompt | zh |
|----|--------|-----|
| `people` | `a photo of people` | 人物 |
| `pets` | `a photo of a dog or cat or rabbit or hamster or bird pet` | 宠物 |
| `landscape` | `a photo of a natural landscape` | 风景 |
| `architecture` | `a photo of a building` | 建筑 |
| `plants` | `a photo of a plant` | 植物 |
| `birds` | `a photo of a bird` | 鸟类 |

Removed (use free-text): family / portraits / kids / land_animals / food / sports / night / insects.

## Owner calibration notes (2026-07-24, ~103 embeds)
- Text CLIP scores sit in a narrow band: strong bird/landscape max ≈0.25–0.28; absent concept max ≈0.21.
- Abstract `human` over-fires on personal albums (High floor 0.204 → ~99/103 above floor).
- Multi-`or` people prompts dilute max (0.267→0.243) and scramble top3 — avoid.
- `a portrait of a person` misses rear-view groups (mall queues); short plural `a photo of people` recovers groups.
- Pets: list common species; still not a detector. Named people → face index, not smart tags.
- Log: host stdout `search_similar mode=text … settings_thr floor= floor_mode= above_floor= returned= max= top3=` (dev: `cargo tauri dev`).

## Behaviour contract
1. **Threshold:** smart tags call `getImageSearchFileList(prompt, …)` **without** `thresholdOverride` — same `settings.imageSearch.thresholdIndex` / UI ladder **0.28/0.24/0.20/0.16** as free-text (**text** host path). Not the image-image Find-similar floors.
2. **Prompts:** short CLIP-style English. Prefer one concept; avoid multi-`or` stacks.
3. **Categories:** keep `SMART_TAG_CATEGORIES` + matching `tag.smart_items` keys. UI uses `category.items[0]`.
4. **Do not** re-introduce a hard-coded smart-tag thr.
5. **Stale smartId:** clear if not in list (`Tag.vue` onMounted + `Content.vue` resolve).
6. **Default thr:** `configStore` `thresholdIndex = 1` (High). Existing saved settings are not auto-migrated.
7. **Thr re-run:** Content watches numeric thr/limit; smart tags re-query via `getImageSearchFileList` directly (clear stuck smart/collection `activePane` first).
8. Empty hits may be content gaps — check `search_similar` histogram first.
9. After model swap or large reindex, re-calibrate floors (`scripts/calibrate_search_thresholds.py`).

## Verify
```bash
pnpm --dir src-vite build
# Manual: smart-tag chip → hits match category; log line max/floor_mode/above_floor sensible
```

## Related
- `change-ai-search-filters.md` — ranking, floors, embed ladder
- `change-image-search-model.md` — CLIP B/32 vision + bilingual int8 text default (Track C)
