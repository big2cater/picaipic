---
name: change-face-index
description: Runbook for face detection/embedding index and clustering performance.
last_updated: 2026-07-20
---

# Change face indexing

## When to use

- Speed up or change face scan / embedding pipeline
- Change face DB write batching or progress/cancel behavior
- Change clustering so it preserves manual person names / assignments
- Add GPU execution providers later (DirectML/CoreML) — not yet

## Touchpoints

| Area | Path |
|------|------|
| Engine + index loop | `src-tauri/src/t_face.rs` |
| Face tables / scan writes | `src-tauri/src/t_sqlite.rs` (`Face`, `apply_scan_batch_with_conn`) |
| Clustering | `src-tauri/src/t_cluster.rs` |
| IPC | `t_cmds.rs` face index / cancel / status |
| Models | resource `models/` via `t_common::DETECTION_MODEL` / `EMBEDDING_MODEL` |

## Rules

- Local-only models; never upload media.
- **Detection model is InsightFace SCRFD** (`det_500m.onnx`), not classic RetinaFace. Decode uses single-channel scores + distance boxes; anchor centers are cell origin `(x*stride, y*stride)`. Runtime checks: ≥9 outputs and `scores/boxes` count == anchor count per stride — fail closed on mismatch.
- **CPU parallel (2026-07-19):** `run_face_indexing` uses a worker pool (2–4 threads, ~half cores). Each worker loads its **own** ONNX sessions (`load_models_from_paths`) — `ort::Session` is not Sync; do not share one engine across threads under a Mutex (that re-serializes inference).
- Per-session `intra_threads` is 1–2 when multi-worker so total ONNX threads stay bounded.
- **Batched DB writes:** workers send scan results on a channel; coordinator flushes with `Face::apply_scan_batch_with_conn` (BEGIN IMMEDIATE + mark + inserts + COMMIT) every ~32 files.
- Thumbnail-first path preserved; bbox scaled to original size when thumb used.
- Inference failure leaves `has_faces` untouched so a later run can retry.
- Cancel is cooperative: stop feeding jobs, workers discard remaining queue items without writing, flush in-flight results, then clustering is skipped if cancelled during index.
- **Incremental clustering (2026-07-20):** `cluster_faces` must **not** call `Face::reset_all_assignments()`. Existing `person_id` / person names are seeds and frozen; only unassigned faces move; new auto names use `Face::next_auto_person_number()`. `reset_all_assignments` remains only for explicit full-reset paths.

## Verify

- `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual: Settings/face index on a library with many images → progress advances faster than serial; cancel mid-run leaves partial marks only for completed files
- Manual: re-run index after cancel resumes unprocessed (`has_faces` null/0)
- Manual: rename a person → re-run face index → name and prior face membership still present; new faces may join that person
