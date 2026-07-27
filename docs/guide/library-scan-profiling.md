# Library Scan Profiling

## Goal

Measure the actual bottleneck before changing scan, metadata, thumbnail, or embedding code. Do not compare runs with different cache state or media mixes.

## Dataset

- Use a disposable copy of a real library or a dedicated test library; never benchmark against originals that may be modified by another process.
- Record file count, total bytes, image/video/RAW counts, representative RAW extensions, and whether an existing database/thumbnails are present.
- Prefer at least 10k files. State the limitation when a smaller set is the only available input.

Current reference media set (local only, not committed):

| Path | Files | Mix | Bytes | Notes |
|---|---:|---|---:|---|
| D:\100k | 100,000 | .jpg only | 5,791,783,234 (5.39 GiB) | Read-only metadata micro-benchmark input |

## Runs

1. Cold scan: remove the library database and thumbnail cache through the app-supported reset/reindex workflow, then index once.
2. Warm rescan: index again without changing media.
3. Metadata-only change set: add or touch a small known subset, then index again.

Keep thumbnail size, AI search settings, power mode, disk location, and app build constant across the three runs. Do not enable another library scan concurrently.

## Capture

Save terminal lines beginning with `[scan]` and record the final UI payload values:

| Field | Why |
|---|---|
| elapsed time / files | baseline throughput |
| discovered / processed / failed | traversal and preview completion |
| search_ready / search_total | embedding lag behind previews |
| image/video/RAW counts | media-mix normalization |
| database/cache state | cold vs warm interpretation |

The current host emits a total scan summary only. If that does not isolate the bottleneck, add phase timings around traversal/indexing, thumbnail drain, cleanup/pairing, and final recount before changing concurrency or decode behavior.

## Metadata Micro-Benchmark

Use the ignored Rust fixture when the question is specifically AFile::new metadata/header/GPS cost, not full library import cost. From src-tauri, set PICAIPIC_SCAN_PROFILE_DIR=D:\100k and PICAIPIC_SCAN_PROFILE_LIMIT=10000, then run:

    cargo.exe test profile_afile_new_for_directory -- --ignored --nocapture

Repeat with PICAIPIC_SCAN_PROFILE_LIMIT=100000 for the full reference set. This fixture is read-only and calls AFile::new(1, path, 1) for each file; it excludes SQLite writes, thumbnail generation, CLIP embedding, Live/Motion pairing side effects, and UI progress.

Reference results on 2026-07-27 using the debug test profile:

| Limit | Files | Failed | Seconds | Files/sec |
|---:|---:|---:|---:|---:|
| 10,000 | 10,000 | 0 | 228.752 | 43.7 |
| 100,000 | 100,000 | 0 | 2,290.637 | 43.7 |

Interpretation: AFile::new metadata/header/GPS pre-read stayed linear on this all-JPEG set from 10k to 100k files. Do not use this result as end-to-end scan throughput; the next bottleneck check must include database writes, thumbnail drain, and optional embedding.

## Decision Rules

- Metadata dominates: profile `AFile::new` and file opens on the real mix before changing parser fallback order.
- Preview drain dominates: split by JPEG/RAW/video and inspect timeout/failure counts before raising worker budgets.
- Search preparation dominates: measure decode and single-session CLIP throughput separately; do not assume the scan loop is at fault.
- Warm rescan remains slow: inspect DB lookups, stale-row cleanup, Live Photo pairing, and thumbnail cache hit rate.

Report the hardware, dataset, commands, raw timings, and any limitations with each conclusion.
