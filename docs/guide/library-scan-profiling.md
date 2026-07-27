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

### Direct Index + Thumbnail Fixture

Use the second ignored fixture to combine temporary SQLite folder/file inserts with real image thumbnail decode/resize/JPEG encode:

    $env:PICAIPIC_SCAN_PROFILE_DIR='D:\100k'
    $env:PICAIPIC_SCAN_PROFILE_LIMIT='10000'
    $env:PICAIPIC_SCAN_PROFILE_THUMB_SIZE='200'
    cargo.exe test profile_directory_index_and_thumbnails -- --ignored --nocapture

The fixture reads source media only, writes a temporary SQLite database, and removes the temporary DB on normal completion. It does not use the app thumbnail cache, Tauri events, concurrent worker budgets, or AI embedding.

Reference results on 2026-07-27 using the debug test profile:

| Limit | Index failed | Thumbnail failed | Index seconds | Thumbnail seconds | Total seconds | Files/sec |
|---:|---:|---:|---:|---:|---:|---:|
| 10 | 0 | 0 | 0.190 | 0.220 | 0.411 | 24.3 |
| 1,000 | 0 | 0 | 23.948 | 21.681 | 45.638 | 21.9 |
| 10,000 | 0 | 0 | 234.076 | 219.778 | 453.930 | 22.0 |

At 10k, synchronous metadata + SQLite indexing accounted for 51.6% of fixture time and thumbnail work for 48.4%. Throughput stayed linear from 1k to 10k. This identifies two similarly sized serial cost centers; it does not predict the production worker wall time because production overlaps thumbnail work with traversal and adds cache writes plus optional embedding.

## Full Scan Phase Timing

Set PICAIPIC_SCAN_PHASE_PROFILE=1 before starting PicAiPic, then run a normal album scan. The final terminal output adds one [scan-profile] line with:

- count_seconds: initial filesystem count pass.
- traversal_seconds: traversal wall time while indexing and scheduling background work.
- index_seconds: cumulative synchronous index_single_file time, including folder/file DB work and AFile::new.
- drain_seconds: wall time spent waiting for thumbnail/embedding tasks after traversal ends.
- thumbnail_task_seconds / embedding_task_seconds: cumulative active service time after acquiring the phase semaphore; thumbnail work can exceed wall time because workers overlap, while the current single-permit embedding total should stay close to its occupied wall time.
- finalize_seconds: stale-row cleanup, Live/Motion pairing, recount, progress finalization, and cover selection.
- total_seconds: end-to-end worker wall time.

Use the attempt/success counters beside each cumulative phase to identify failures and normalize averages. Keep the same thumbnail size and AI settings when comparing cold and warm runs.

The first 100k production run on 2026-07-27 used AI search indexing and completed with no index, thumbnail, or embedding failures:

| Files | Traversal | Index cumulative | Drain tail | Total | End-to-end throughput |
|---:|---:|---:|---:|---:|---:|
| 100,000 | 3,233.415s | 3,111.840s | 6,313.095s | 9,548.281s | 10.47 files/sec |

The run showed a serial embedding bottleneck: traversal/indexing finished after about 54 minutes, then the worker spent another 105 minutes draining queued search-index work. Its historical `embedding_task_seconds=336550966.103` and `thumbnail_task_seconds=12544.523` included semaphore queue waits because the timers started before permit acquisition; do not use those two values. Later builds time active service only. Successful per-file embedding source logs are disabled by default; set `PICAIPIC_EMBED_FILE_TRACE=1` only for targeted diagnosis.

## Decision Rules

- Metadata dominates: profile `AFile::new` and file opens on the real mix before changing parser fallback order.
- Preview drain dominates: split by JPEG/RAW/video and inspect timeout/failure counts before raising worker budgets.
- Search preparation dominates: measure decode and single-session CLIP throughput separately; do not assume the scan loop is at fault.
- Warm rescan remains slow: inspect DB lookups, stale-row cleanup, Live Photo pairing, and thumbnail cache hit rate.

Report the hardware, dataset, commands, raw timings, and any limitations with each conclusion.
