---
name: profile-library-scan
description: Measure real library scan throughput before changing metadata, thumbnail, or embedding performance.
last_updated: 2026-07-27
---

# Profile Library Scan

Use `docs/guide/library-scan-profiling.md` before proposing scan performance changes. Compare cold, warm, and small-delta scans on the same media mix; preserve originals and do not run concurrent scans.

Current scan logs expose total time only. Add phase instrumentation only after collecting a baseline that cannot identify the bottleneck.

For metadata/header/GPS isolation, run ignored test profile_afile_new_for_directory with PICAIPIC_SCAN_PROFILE_DIR and optional PICAIPIC_SCAN_PROFILE_LIMIT. The 2026-07-27 D:\100k all-JPEG reference measured 10k and 100k at the same debug-test throughput: 43.7 files/sec, 0 failures. Treat that as AFile::new micro-benchmark evidence only, not full import throughput.

For a reproducible middle layer without app state, run ignored test profile_directory_index_and_thumbnails. On D:\100k at 200px, 1k and 10k measured 21.9/22.0 files/sec with zero failures; the 10k split was 51.6% temporary SQLite/index and 48.4% thumbnail codec. The fixture is serial and excludes app cache, worker concurrency, events, and embedding.

For end-to-end album scans, start the app with PICAIPIC_SCAN_PHASE_PROFILE=1. Compare the emitted count, traversal, cumulative index, drain tail, cumulative thumbnail/embedding task latency, finalize, and total fields. Cumulative task latency overlaps under concurrency and must not be summed into total wall time.
