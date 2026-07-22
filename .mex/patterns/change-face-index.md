---
name: change-face-index
description: Runbook for face detection/embedding index and clustering performance.
last_updated: 2026-07-22
---

# Change face indexing

## When to use

- Speed up or change face scan / embedding pipeline
- Change face DB write batching or progress/cancel behavior
- Change clustering so it preserves manual person names / assignments
- Plan large-library O(n²) removal (ANN / blocked KNN) — see product plan
- Add GPU execution providers later (DirectML/CoreML) — **not doing (G8)**

## Touchpoints

| Area | Path |
|------|------|
| Engine + index loop | `src-tauri/src/t_face.rs` |
| Face tables / scan writes | `src-tauri/src/t_sqlite.rs` (`Face`, `apply_scan_batch_with_conn`) |
| Clustering | `src-tauri/src/t_cluster.rs` |
| Constants | `src-tauri/src/t_common.rs` (`K_NEIGHBORS`, `MIN_SAMPLES`, `CLUSTER_N_EXACT`, `CLUSTER_BLOCK_SIZE`) |
| Cluster mode | Pinia `settings.face.clusterMode`; Settings UI; IPC `index_faces.clusterMode` |
| IPC | `t_cmds.rs` face index / cancel / status |
| Models | resource `models/` via `t_common::DETECTION_MODEL` / `EMBEDDING_MODEL` |
| Large-n product plan | `docs/guide/face-cluster-ann-plan.md` |

## Rules

- Local-only models; never upload media.
- **Detection model is InsightFace SCRFD** (`det_500m.onnx`), not classic RetinaFace. Decode uses single-channel scores + distance boxes; anchor centers are cell origin `(x*stride, y*stride)`. Runtime checks: ≥9 outputs and `scores/boxes` count == anchor count per stride — fail closed on mismatch.
- **CPU parallel (2026-07-19):** `run_face_indexing` uses a worker pool (2–4 threads, ~half cores). Each worker loads its **own** ONNX sessions (`load_models_from_paths`) — `ort::Session` is not Sync; do not share one engine across threads under a Mutex (that re-serializes inference).
- Per-session `intra_threads` is 1–2 when multi-worker so total ONNX threads stay bounded.
- **Batched DB writes:** workers send scan results on a channel; coordinator flushes with `Face::apply_scan_batch_with_conn` (BEGIN IMMEDIATE + mark + inserts + COMMIT) every ~32 files.
- Thumbnail-first path preserved; bbox scaled to original size when thumb used.
- Inference failure leaves `has_faces` untouched so a later run can retry.
- Cancel is cooperative: stop feeding jobs, workers still report discarded jobs so progress can hit 100%; clustering skipped if cancelled during index.
- **Incremental clustering (2026-07-20):** `cluster_faces` must **not** call `Face::reset_all_assignments()`. Existing `person_id` / person names are seeds and frozen; only unassigned faces move; new auto names use `Face::next_auto_person_number()`. `reset_all_assignments` remains only for explicit full-reset paths.

## Clustering performance (current + next)

### Shipped (2026-07-22)

- `insert_top_k`: **linear ordered insert** (no full sort per pair).
- Cancel during graph / whisper / pre-assign → `Err("cancelled")`; mid-assign → `Err("cancelled after assigning N")`.
- `t_face` maps cancel errors to `face_index_finished.cancelled = true` (not “success, 0 persons”).
- `cosine_distance`: `debug_assert` equal lengths.

### P0 measurement (2026-07-22)

- stderr timing / histogram logs (`[cluster] start|parse|graph_*|whisper|done …`) with `n`, pre_assigned/pre_unassigned, newly_assigned, phase ms.
- Unit tests: same-file ban, cancel, `insert_top_k`, synthetic bench `bench_exact_graph_small_synthetic` (prints n=128/512 ms).

### P1 adaptive graph + ANN (2026-07-22)

- `build_knn_graph_exact` + `build_knn_graph_blocked` + **`build_knn_graph_ann`** + adaptive.
- **`n < CLUSTER_N_EXACT` (8000)** + `auto` → row-wise exact.
- **`n ≥ 8000` or `fast`** → **HNSW ANN** (`instant-distance` pure Rust); on ANN `Err` (non-cancel) → blocked exact fallback.
- Edge meaning: cosine distance, `(1-d)²` weight, Top-K, same-file ban (post-query filter for ANN).
- Logs: `mode=` + `strategy=exact|ann|blocked`.
- Dep: `instant-distance = 0.6.1` (MIT/Apache-2.0) in `src-tauri/Cargo.toml`.
- Tests: blocked≡exact, ANN same-file/edges, `ann_vs_exact_parity_soft_gate` (soft Jaccard ≥ 0.5).
- Run: `cargo test --manifest-path src-tauri/Cargo.toml -- t_cluster:: --nocapture`

### P2 quality & ops (2026-07-22)

- **`face.clusterMode`**: `auto` | `exact` | `fast` (Settings + Pinia + IPC).
- Parity: exact↔blocked + exact↔ANN reports on stderr.

### Knobs (host)

| Knob | Where | Default | Notes |
|------|--------|---------|--------|
| Similarity / epsilon | `face.clusterThresholdIndex` → thresholds array | Medium 0.55 | Cosine **distance** threshold |
| `clusterMode` | `settings.face.clusterMode` | `auto` | auto/exact/fast |
| `CLUSTER_N_EXACT` | `t_common` | 8000 | Auto: exact vs ANN |
| `CLUSTER_BLOCK_SIZE` | `t_common` | 2048 | Blocked fallback tiles |
| `CLUSTER_ANN_EF_SEARCH` | `t_common` | 120 | HNSW efSearch floor |
| `CLUSTER_ANN_EF_CONSTRUCTION` | `t_common` | 200 | HNSW build quality |
| `K_NEIGHBORS` | `t_common` | 80 | Top-K edges per face |

### Deferred / not doing

- **P3 disk ANN + incremental HNSW insert** — owner 2026-07-22: low ROI after in-process ANN; `instant-distance` has no real incremental API; face-index time is mostly ONNX. See plan “P3 deferred”. If re-cluster is still slow after measuring a 20k+ library, prefer **embedding binary cache** over graph persistence.

### Optional follow-ups (only if measured need)

- Owner real-album person merge/split sample set.
- Tune `CLUSTER_ANN_EF_*` / K if splits rise.
- Embedding binary/mmap cache for re-cluster load path (not graph serde).

## Verify

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml -- t_cluster:: --nocapture`
- Manual: Settings/face index on a library with many images → progress advances faster than serial; cancel mid-run leaves partial marks only for completed files; progress reaches 100% on cancel
- Manual: re-run index after cancel resumes unprocessed (`has_faces` null/0)
- Manual: rename a person → re-run face index → name and prior face membership still present; new faces may join that person
- Manual: cancel during clustering phase → finished event has `cancelled: true` (not silent empty success)
- Manual: after face index clustering, console/stderr shows `[cluster] start n=…` and `[cluster] done … total_ms=`
