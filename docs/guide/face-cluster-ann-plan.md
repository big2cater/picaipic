---
name: face-cluster-ann-plan
description: Product-level plan to eliminate O(n²) face clustering for large libraries (ANN / blocked KNN).
last_updated: 2026-07-22
status: in_progress
---

# Large-library face clustering — product plan (ANN / blocked KNN)

## Problem

Current clustering (`t_cluster.rs`) builds a similarity graph by **all-pairs** cosine distance among face embeddings, then Chinese Whispers, with Top-K edges (`K_NEIGHBORS = 80`).

- Complexity: **O(n² · d)** distance work (+ Top-K maintenance).
- Memory: edges pruned to ~**n · K** (good), but **time** dominates.
- Target libraries: **10k–100k+ files** → faces can be tens/hundreds of thousands → full pairs can take **tens of minutes to hours** on desktop CPUs.

**Already shipped (2026-07-22 audit pack, still O(n²)):**

- Linear `insert_top_k` (no full sort per insert).
- Cancel returns `Err("cancelled…")` so UI is not “success with 0 persons”.
- Incremental seed/freeze of existing `person_id` (unchanged product rule).

This plan is the **next product step**: remove all-pairs as the default path for large `n`.

## Goals

1. **Interactive desktop UX:** full re-cluster of ~50k faces finishes in **minutes**, not hours, on a mid-range Windows laptop (CPU-only OK).
2. **Preserve product semantics:**
   - Existing person names / assignments survive re-index (**frozen seeds**).
   - Only unassigned faces move between people / new clusters.
   - Same-file faces never form an edge (siblings in one photo).
3. **Local-first:** no cloud embeddings or remote ANN service.
4. **Graceful degradation:** small libraries keep exact Top-K path; large libraries use approximate path with documented quality trade-off.
5. **Cancellable** at graph-build and assign; UI always shows cancelled vs failed vs success.

## Non-goals (this program)

- GPU face **detection** EP (G8 cancelled).
- Changing embedding model (still MobileFaceNet 512-d unless a separate project decides otherwise).
- Real-time “type-to-search person” ANN (can reuse index later).
- Perfect recall of every true neighbor at 100k+ (we optimize for usable people grouping).

## Success metrics

| Metric | Small library (n ≤ 5k faces) | Large (n ≈ 50k faces) |
|--------|------------------------------|------------------------|
| Graph build wall time | ≤ few seconds (exact OK) | **≤ 3–5 min** target (CPU) |
| Peak RAM (extra beyond embeddings) | modest | **≤ ~1–2 GB** graph working set |
| Quality | match current exact path | ≥ ~95% of faces that exact path would assign land in same person **or** a user-acceptable merge/split rate on a labeled sample set |
| Cancel | cooperative, no false “done” | same |

Quality gate: maintain a **private sample set** (owner-curated albums with known people counts), run exact vs ANN side-by-side, track merge/split deltas—not public cloud eval.

## Architecture options (decision record)

### Option A — Pure Rust ANN in-process (recommended default)

**Idea:** Build an ANN index over normalized f32 embeddings (HNSW or IVF-flat), query top-K per face (or mutual Top-K), feed the same Chinese Whispers pipeline.

| Pros | Cons |
|------|------|
| No Python/sidecar | New crate + validation work |
| Fits Tauri host lifecycle | HNSW memory grows with n |
| Same process cancel/progress | Tuning (ef, M) needed |

**Candidate crates (evaluate at implement time, pin versions in Cargo.lock):**

- HNSW-style: e.g. `usearch` (if Windows MSVC friendly) / pure-Rust HNSW crates audited for license + build.
- Fallback: **blocked exact KNN** (Option B) if ANN crate fails Windows/link.

### Option B — Blocked / tile exact KNN (no new ANN dependency)

**Idea:** Keep exact cosine, but process **blocks** of size `B` (e.g. 2k–4k faces): for each block pair `(I,J)` compute distances only for that tile, update Top-K lists. Still **O(n²)** flops, but:

- Better cache locality and progress granularity.
- Easier cancel mid-block.
- Can skip block pairs with cheap bounds later.

**Use when:** n is medium (5k–20k) or ANN crate is blocked; still too slow for 100k+.

### Option C — External index file / mmap

Persist ANN graph or HNSW to app data so **re-cluster after small scans** is incremental:

- New faces: insert into index + query neighbors only for new + 1-hop affected.
- Full rebuild on model change / “reset clustering”.

**Phase 2** after Option A lands.

### Decision (implemented 2026-07-22)

1. **Adaptive strategy (shipped)**  
   - `n < CLUSTER_N_EXACT` (**8000**) in `auto`: exact all-pairs + linear top-k.  
   - `n ≥ 8000` or `fast`: **HNSW ANN** via pure-Rust `instant-distance` 0.6.1 (MIT/Apache-2.0).  
   - ANN build/search hard failure → **blocked exact** fallback + stderr log (never silent empty).  
   - `exact` mode forces all-pairs.  
2. **P3 (disk ANN + incremental insert):** **Deferred / not doing for now** (owner 2026-07-22) — see “P3 deferred” below.

Knobs: `CLUSTER_N_EXACT`, `CLUSTER_BLOCK_SIZE`, `CLUSTER_ANN_EF_SEARCH`, `CLUSTER_ANN_EF_CONSTRUCTION` in `t_common`; user `face.clusterMode`.

## Algorithm integration (must preserve)

```
load slim faces (id, file_id, person_id, emb)
parse + L2-normalize embeddings once
--- REPLACE THIS SECTION ---
build undirected graph: for each face, Top-K neighbors with dist < threshold
  (exclude same file_id)
--- END REPLACE ---
seed labels from existing person_id; freeze those nodes
Chinese Whispers iterations (unassigned only)
assign: map cluster labels → existing Person or create Person N
```

ANN must produce the **same edge meaning**: weight = `1 - cosine_distance`, thresholded, mutual or one-way Top-K then symmetrize as today.

## Product / UX

| Surface | Behavior |
|---------|----------|
| Progress | Phases: `graph` (0–100% by blocks or by i in n), `iterate`, `assign` — already partially there |
| Settings | Optional advanced: “Fast clustering (approximate)” default **on** when n large; “Exact (slow)” for power users / small n |
| First-run large library | Toast/banner once: “Large library — using fast clustering” |
| Failure | If ANN init fails → automatic blocked/exact fallback + log; never silent empty people |

## Implementation phases

### P0 — Spec freeze & measurement (short)

- [x] Add face-count histogram logging at cluster start (`n`, assigned vs unassigned, elapsed graph vs whisper).
  - stderr lines prefixed `[cluster]`: start counts, parse, graph_exact, whisper, assign, thumbnail, done summary.
- [x] Benchmark fixtures: synthetic unit vectors in `t_cluster` tests (`bench_exact_graph_small_synthetic`).
  - Reference (dev unoptimized test binary, 2026-07-22): **n=128 / 512-d ≈ 39 ms**, **n=512 ≈ 375 ms** exact graph.
  - [ ] One real medium album timing (owner, when convenient).
- [x] Confirm Windows link policy for chosen ANN crate (or stick to blocked KNN only).
  - **Chosen:** `instant-distance` 0.6.1 pure Rust (no C/MSVC link); blocked exact remains fallback.

### P1 — Adaptive exact vs approximate graph build

- [x] Extract `build_knn_graph_exact(...)` from current loop.
- [x] Implement `build_knn_graph_blocked(...)` + **`build_knn_graph_ann(...)`** (HNSW / `instant-distance`).
- [x] Threshold `CLUSTER_N_EXACT` (8000) + block/ANN ef knobs in `t_common`; adaptive dispatch; unit tests: blocked≡exact, ANN soft parity, cancel, strategy.
- [x] Preserve same-file edge ban and frozen seeds (`try_add_edge` / post-query file_id filter; assign path unchanged).

### P2 — Quality & ops

- [x] Sample-set edge parity: exact↔blocked + exact↔ANN soft gate (`ann_vs_exact_parity_soft_gate`). Owner real-album person merge/split still manual.
- [x] Document knobs in `patterns/change-face-index.md`.
- [x] Config: `face.clusterMode = auto | exact | fast` (Settings + Pinia + `index_faces` IPC).

### P3 — Incremental / disk ANN — **DEFERRED (do not implement now)**

Originally planned:

- Persist graph or ANN index under app data per library id.
- On index finish with few new faces, partial neighbor update instead of full rebuild.

**Owner decision (2026-07-22): skip P3 for now.** ROI is low after P0–P2:

1. **Main graph bottleneck is gone.** Large-n path is HNSW (~O(n log n)), not O(n²). Target band for ≥20k faces is already addressed by in-process ANN + blocked fallback.
2. **Face index wall-time is dominated by embedding inference** (SCRFD + MobileFaceNet in `t_face` workers), not re-cluster graph build. P3 does not touch ONNX.
3. **`instant-distance` is build-from-points** — no post-build incremental insert API. “Incremental” would still re-build the graph (at best reusing deserialized vectors). True insert-only HNSW often needs periodic full rebuilds anyway (layer imbalance / recall drift).
4. **Disk serde of HNSW structure** needs feature flags, versioning, and hard fail-open to full rebuild across crate upgrades — brittle for little gain.
5. **Correctness surface:** frozen seeds + same-file ban must hold across partial updates; regression risk exceeds time saved vs full rebuild from DB embeddings.

**If re-cluster ever proves painful after measurement**, prefer a **lighter alternative first**: per-library normalized embedding binary/mmap cache (skip row→f32 parse + peak RAM), **not** a full incremental graph. Confirm with timings on a real 20k+ face library: load/parse vs graph vs whisper.

Higher-ROI face work if speed still matters: worker/batching on **inference** (GPU EP remains **G8 cancelled**).

## Risks & mitigations

| Risk | Mitigation |
|------|------------|
| ANN quality regressions (split people) | Conservative ef/K; default threshold slightly looser only after measurement; keep exact mode |
| MSVC / crate build breaks release | Prefer crates with CI on Windows x64; blocked KNN fallback always in tree |
| Memory blow-up HNSW | Cap M/ef; optional quantize later; monitor peak |
| Cancel mid-assign partial writes | Already returns cancelled with partial assigns; UI must say “partial — re-run” (P1 copy) |
| Same-file ban broken in ANN | Filter `file_id` after query before insert_top_k |

## Dependencies & policy

- Prefer **pure Rust** or statically linked C with existing MSVC story (like ONNX/LibRaw).
- No new cloud service.
- License: MIT/Apache-2.0 preferred; record in `context/stack.md` when chosen.

## Out-of-scope reminders

- GPU EP for SCRFD (G8 not doing).
- Replacing Chinese Whispers with Leiden/Louvain unless quality demands—it is not the current bottleneck vs O(n²) distances.

## References (in-repo)

- `src-tauri/src/t_cluster.rs` — Chinese Whispers + Top-K graph
- `src-tauri/src/t_common.rs` — `K_NEIGHBORS`, `MIN_SAMPLES`
- `src-tauri/src/t_face.rs` — index workers, cluster invoke, cancel flag
- `.mex/patterns/change-face-index.md` — operational runbook

## Exit criteria for “done” (program = P0–P2 + ANN; **not** P3)

1. Libraries with **≥ 20k faces** complete **clustering** in the target time band on a reference machine (in-process HNSW; measure owner-side).
2. Cancel never reports success with `total_persons = 0` unless truly empty.
3. Incremental **person freeze** still holds on re-index after renames (product incremental clustering — unrelated to P3 disk graph).
4. Docs + MEX pattern updated; decision logged in `context/decisions.md`.
5. P3 disk/incremental ANN explicitly deferred until measurement demands a lighter embedding-cache path.
