---
name: change-library-perf
description: Large-library viewport loading and similar/semantic search performance.
last_updated: 2026-07-19
---

# Change large-library performance

## When to use

- Slow similar-image / AI text search ranking
- Grid scroll freezes or bulk thumbnail storms on large hit lists
- Porting Lap v0.3.0 search/viewport optimizations

## Touchpoints

| Area | Path |
|------|------|
| Similar search | `t_sqlite.rs` `search_similar_images`, `cosine_similarity_blob`, `get_files_by_ids` |
| Viewport load | `Content.vue` `fetchDataRange`, `handleVisibleRangeUpdate`, `fetchMissingVisibleThumbnails` |
| Virtual scroll | `VirtualScroll.vue`, `GridView.vue` |

## Rules

- Rank on **id + embeds BLOB only**; never hydrate full `AFile` rows until after sort+limit.
- Cosine: precompute query norm; stream LE f32 from blob (`cosine_similarity_blob`) — no per-candidate `Vec<f32>`.
- Hydrate winners with `get_files_by_ids` chunked **≤500** `IN (...)`, then restore score order.
- Viewport: 3-phase load (visible → below → above); clamp SQL chunk size to viewport-ish bounds.
- Semantic/similar results: warm thumbs for first screen only; use `fetchMissingVisibleThumbnails` on scroll — do not `getFileListThumb` the entire hit list.

## Verify

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `pnpm --dir src-vite build`
- Manual: AI search / similar image on a large library — results return faster; scrolling loads thumbs progressively
