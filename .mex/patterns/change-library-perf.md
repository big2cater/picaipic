---
name: change-library-perf
description: Large-library viewport loading and similar/semantic search performance.
last_updated: 2026-07-24
---


# Change large-library performance

## When to use

- Slow similar-image / AI text search ranking
- Grid scroll freezes or bulk thumbnail storms on large hit lists
- Porting Lap v0.3.0 search/viewport optimizations

## Touchpoints

| Area | Path |
|------|------|
| Similar search | `t_sqlite.rs` `search_similar_images`, `cosine_similarity_blob`, embed matrix cache, `get_files_by_ids` |
| Viewport load | `Content.vue` `fetchDataRange`, `handleVisibleRangeUpdate`, `fetchMissingVisibleThumbnails` |
| Virtual scroll | `VirtualScroll.vue`, `GridView.vue` |

## Rules

- Rank on **id + embeds only**; never hydrate full `AFile` rows until after sort+limit.
- **Embed matrix cache (2026-07-24):** process-local row-major f32 matrix + per-row L2 norms, keyed by normalized DB path + generation. First search may load all embeds; subsequent searches score in RAM (`matrix=1` in host log). Exact cosine only (ANN is R2). Soft skip if data would exceed ~512 MiB.
- **Rayon scoring (2026-07-24 R1):** `score_embed_matrix` uses serial for N < 256; otherwise `par_iter` fold/reduce of local (scores, max, band_gt) — no shared counters. Unit test: serial ≡ parallel.
- **Image-search ANN (2026-07-24 R2 + polish):** when N ≥ `IMAGE_SEARCH_ANN_MIN_N` (8000), **background** HNSW build on L2-normalized rows (`instant-distance`); first queries use exact matrix until `embed_ann ready` log. Query pulls ~500 candidates then exact cosine (`matrix=2`). Failed builds cached until generation bump (no rebuild thrash). Small N / fail / building → exact (`matrix=1`).
- Invalidate matrix on embed write/clear and on `clear_conn_pool` (library switch / storage migrate).
- MVP: matrix includes search-folder exclusions; **file-type filter (`search_file_type != 0`) falls back to SQL blob path**.
- Cosine fallback: precompute query norm; stream LE f32 from blob (`cosine_similarity_blob`) — no per-candidate `Vec<f32>`.
- Hydrate winners with `get_files_by_ids` chunked **≤500** `IN (...)`, then restore score order.
- Viewport: 3-phase load (visible → below → above); clamp SQL chunk size to viewport-ish bounds.
- Semantic/similar results: warm thumbs for first screen only; use `fetchMissingVisibleThumbnails` on scroll — do not `getFileListThumb` the entire hit list.

## Verify

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `pnpm --dir src-vite build`
- Manual: AI search / similar image on a large library — second query faster (`matrix=1`); scrolling loads thumbs progressively
