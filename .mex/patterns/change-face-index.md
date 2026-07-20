---
name: change-face-index
description: Runbook for face detection/embedding index and clustering performance.
last_updated: 2026-07-19
---

# Change face indexing

## When to use

- Speed up or change face scan / embedding pipeline
- Change face DB write batching or progress/cancel behavior
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
- **CPU parallel (2026-07-19):** `run_face_indexing` uses a worker pool (2–4 threads, ~half cores). Each worker loads its **own** ONNX sessions (`load_models_from_paths`) — `ort::Session` is not Sync; do not share one engine across threads under a Mutex (that re-serializes inference).
- Per-session `intra_threads` is 1–2 when multi-worker so total ONNX threads stay bounded.
- **Batched DB writes:** workers send scan results on a channel; coordinator flushes with `Face::apply_scan_batch_with_conn` (BEGIN IMMEDIATE + mark + inserts + COMMIT) every ~32 files.
- Thumbnail-first path preserved; bbox scaled to original size when thumb used.
- Inference failure leaves `has_faces` untouched so a later run can retry.
- Cancel is cooperative: stop feeding jobs, workers discard remaining queue items without writing, flush in-flight results, then clustering is skipped if cancelled during index.

## Verify

- `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual: Settings/face index on a library with many images → progress advances faster than serial; cancel mid-run leaves partial marks only for completed files
- Manual: re-run index after cancel resumes unprocessed (`has_faces` null/0)
