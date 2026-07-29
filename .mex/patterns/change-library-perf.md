---
name: change-library-perf
description: Large-library viewport loading and similar/semantic search performance.
last_updated: 2026-07-29
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
- **Image-search ANN (2026-07-24 R2; resource fix 2026-07-28):** exact matrix scoring is the product default (`matrix=1`) because 110k queries are already responsive and rebuilding HNSW every process lifetime is too expensive. Set `PICAIPIC_EMBED_ANN=1` to opt into lazy first-search construction. Opt-in ANN uses a dedicated Rayon pool with 2 threads by default (`PICAIPIC_EMBED_ANN_BUILD_THREADS`, 1-8), then pulls ~500 candidates and exact-reranks (`matrix=2`). Failed builds are cached until generation bump.
- **Matrix warm (2026-07-26; amended 2026-07-28):** `create_db()` success path spawns `warm_embed_matrix_cache` so library open/switch preloads the exact matrix only (`ann=disabled` by default). Do not build ANN merely by opening or searching a large library unless explicitly opted in: 100k HNSW construction was too slow and saturated CPU. Full disk persistence of matrix/ANN remains deferred (versioned artifact + memory tradeoff).
- Invalidate matrix on embed write/clear and on `clear_conn_pool` (library switch / storage migrate).
- MVP: matrix includes search-folder exclusions; **file-type filter (`search_file_type != 0`) falls back to SQL blob path**.
- Cosine fallback: precompute query norm; stream LE f32 from blob (`cosine_similarity_blob`) — no per-candidate `Vec<f32>`.
- Hydrate winners with `get_files_by_ids` chunked **≤500** `IN (...)`, then restore score order.
- Viewport: 3-phase load (visible → below → above); clamp SQL chunk size to viewport-ish bounds.
- Semantic/similar results: warm thumbs for first screen only; use `fetchMissingVisibleThumbnails` on scroll — do not `getFileListThumb` the entire hit list.
- Deduplicate in-flight thumbnail work by content-request + file id. Passive scroll updates can overlap before the first IPC returns; checking only `file.thumbnail` is not sufficient.
- Give viewport thumbnail work a generation token. A scrollbar jump must stop stale phases and future batches; metadata chunk deduplication must return the existing promise so the new viewport can await it and then warm its thumbnails.
- Thumbnail cards should observe their container only when responsive rotated geometry needs live dimensions. Fixed cards and unrotated cards must not each keep a `ResizeObserver` during normal grid scrolling.
- Load plugin contributions at the content/view level, not from every virtualized thumbnail mount. Clear per-card timers on unmount.
- Lazy-mount per-file context menus for the hovered, active, or open card instead of instantiating one for every buffered virtual item. Position virtual items with contained transforms to reduce layout/paint work during large scrollbar jumps.

## Verify

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `pnpm --dir src-vite build`
- Manual: opening/searching a 100k library logs `embed_matrix ... ann=disabled`, returns via `matrix=1`, and never schedules ANN by default. With `PICAIPIC_EMBED_ANN=1`, first search schedules a 2-thread build and later searches may use `matrix=2`.

## Next checkpoint

- On the existing 110,343-vector library, set
  `PICAIPIC_EMBED_WARM_PROFILE=1`, leave ANN unset, and restart. Capture
  `embed_matrix warm ready`, `warm_profile`, and `idle_profile`; do not search
  during the five-second settle plus five-second CPU sample. No reimport.
- Default logging adds only total elapsed time and allocated matrix MiB. Detailed
  per-row SQLite/build timing and Windows process CPU/working set are opt-in.
- Baseline at 110,343 rows: 3.150s total, 1.217s SQLite, 1.912s build,
  257.5 MiB matrix, and 0.06% post-settle CPU. Disk cache is deferred.
- `COUNT(*) + try_reserve_exact` reduced 257.5 MiB to 216.8 MiB but regressed
  warm time from 3.150s to 5.846s; do not restore the double-scan design. Accepted
  one-scan borrowed-BLOB + final-shrink result: 2.876s total, 1.062s SQLite,
  1.804s build, 216.8 MiB, and 0.00% post-settle CPU.
- Matrix rows use borrowed rusqlite BLOB slices during f32 parsing; do not restore
  per-row `Vec<u8>` copies. At 110,343 x 512 that avoided about 215.5 MiB of
  cumulative temporary copying. Remeasure SQLite/build/total time and matrix MiB.
- Keep exact `matrix=1` as the default and ANN diagnostic-only until a persistent
  index can amortize construction across launches.

## Cold Import Boundary

The 2026-07-29 copied 1,000-file cold-import work is closed for metadata. Safe
JPEG marker paths reduced synchronous index work to `7.197s` and metadata to
`4.347s`, while preserving 970/970 thumbnail and embedding successes. Its
`37.443s` wall time was dominated by a variable `28.296s` embedding drain tail,
not metadata. Do not pursue smaller EXIF/XMP/TIFF parser savings as a way to
improve cold-import wall time. Any future throughput project starts by profiling
embedding preprocessing/inference and reply latency as a separate, controlled
workstream.
