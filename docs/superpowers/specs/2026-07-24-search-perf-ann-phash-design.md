# Design: Search Perf (rayon) + Image-Search ANN + Dedup Similar (dHash)

**Date:** 2026-07-24  
**Status:** Approved scope (scheme A); implement in three independent rounds  
**Out of scope:** Track B full-stack / SigLIP2 product UI / `app_meta` embed binding (no multi-model until owner reopens)

## Goals

| Round | Goal | Success |
|-------|------|---------|
| **R1** | Parallelize in-memory matrix cosine scoring | Same results as serial; multi-core used on large N |
| **R2** | Approximate Top-K for large libraries on the embed matrix | Auto: small N exact, large N HNSW + exact rerank; ANN fail → exact |
| **R3** | Fill Dedup **Similar** tab with visual near-duplicates | dHash groups + keep/trash parity with Exact; no AI search coupling |

## Non-goals

- Multilingual / model swap / library embed rebuild pipeline  
- Persistent ANN index on disk  
- pHash from file-menu “find similar” entry (only Dedup Similar tab)  
- Changing CLIP floors / thr ladder (except using existing hard-cap limit)

## Context (already shipped)

- Process-local `EmbedMatrix` + SQL BLOB fallback (`change-library-perf.md`)  
- Legacy multilingual text-only **disabled** (activation rejected; Settings Default-only)  
- User `limit` hard-caps all tiers  

---

## R1 — rayon parallel `score_embed_matrix`

### Design

- Add `rayon` dependency.  
- Split scoring into **serial reference** (`score_embed_matrix_serial`) and **production** path that uses rayon when `n` is large enough (e.g. `n >= 256`), else serial (avoid thread pool overhead on tiny libs).  
- Parallel pattern: `ids.par_iter().enumerate().fold(...).reduce(...)`  
  - Local state: `Vec<(i64,f32)>`, `max_score`, `band_gt[5]`  
  - Reduce: concat scores; max of max; element-wise sum of bands  
- **No shared mutable counters** across threads.  
- Ranking / floors / `image_search_top_k` unchanged.

### Tests

- Unit: serial vs parallel on synthetic matrix → same id set, same scores within float eps, same `band_gt`, same `max_score`.  
- Existing `image_search_top_k` tests still pass.

### Verify

```bash
cargo test --manifest-path src-tauri/Cargo.toml score_embed_matrix -- --nocapture
cargo check --manifest-path src-tauri/Cargo.toml
```

---

## R2 — Image-search ANN (after R1)

### Design

- Build HNSW over **L2-normalized** matrix rows (query also L2-normalized before search).  
- Distance: L2 on unit vectors ≈ cosine ranking.  
- Threshold constant `IMAGE_SEARCH_ANN_MIN_N` (default **8000**), tunable.  
- On matrix load / generation match: optionally build ANN index stored next to matrix (or separate cache keyed by same generation).  
- Query path: ANN candidates (k ≥ thr_cap soft max, with efSearch headroom) → **exact** cosine on candidates → existing absolute_floor / top_k.  
- `n < T` or ANN `Err` → exact matrix (rayon).  
- Invalidate with embed matrix generation / `clear_conn_pool`.

### Reuse

- Face clustering already uses `instant-distance`; same crate, different API shape (query Top-K vs pairwise graph).

### Verify

- Synthetic: ANN+rerank top ids match exact top ids for high-sep clusters.  
- Manual: large library search log shows `ann=1` or `matrix=1`; cancel/library switch safe.

---

## R3 — Dedup Similar (dHash)

### Design

- Algorithm: **64-bit dHash** from thumbnail or downscaled decode (prefer cheap path consistent with library perf).  
- Table `file_phashes (file_id PK, hash INTEGER/TEXT, mtime, computed_at)` via migration.  
- Scan: cancelable; skip if mtime unchanged; progress events (extend or parallel to exact scan).  
- Group: Hamming distance ≤ threshold (default **8**, settings optional later).  
- UI: `DedupPane` Similar tab uses same group list / keep / trash as Exact; **separate** storage from blake3 groups so tabs do not clobber each other.  
- i18n: replace “coming next” copy with empty/scanning states.

### Verify

- Two re-exports of same photo land in one similar group; unrelated photos do not.  
- Delete/keep path does not touch exact hash tables incorrectly.

---

## Delivery order

1. R1 (this implementation slice)  
2. R2  
3. R3  

Each round: own commit(s), MEX pattern updates, independent ship.
