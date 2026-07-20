---
name: change-ai-search-filters
description: File-type filters and result grouping for AI / similar / filename search.
edges:
  - target: change-library-perf.md
    condition: when changing vector search SQL or hydrate batching
  - target: change-smart-albums.md
    condition: when smart-tag search reuses ImageSearchParams
  - target: ../ROUTER.md
    condition: after shipping search UX filters
last_updated: 2026-07-20
---

# Change AI search filters & grouping

## When to use
- Add/change file-type filtering on semantic / similar search
- Change search result section headers (visual / similar / filename)
- Wire toolbar filter into temporary similar-from-file mode

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
1. Mask matches library filter: `0` all, `1` image, `2` video, `4` raw (combine with OR).
2. AI search applies filter in SQL **before** cosine scoring (embeddings candidates only).
3. Filename search continues via `QueryParams.search_file_type` (already present).
4. Toolbar type filter is enabled in search sidebar and similar temp view; sort remains disabled for AI results.
5. Result lists show one section header when not date-grouped: Visual / Similar / Filename.
6. Changing type filter re-runs active AI/filename/smart-tag/similar queries.

## Verify
```bash
cargo check --manifest-path src-tauri/Cargo.toml
pnpm --dir src-vite build
```
Manual: AI search → set Image only → fewer/no videos; Similar from file → same; result header label matches search mode.
