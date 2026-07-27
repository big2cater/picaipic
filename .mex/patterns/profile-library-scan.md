---
name: profile-library-scan
description: Measure real library scan throughput before changing metadata, thumbnail, or embedding performance.
last_updated: 2026-07-27
---

# Profile Library Scan

Use `docs/guide/library-scan-profiling.md` before proposing scan performance changes. Compare cold, warm, and small-delta scans on the same media mix; preserve originals and do not run concurrent scans.

Current scan logs expose total time only. Add phase instrumentation only after collecting a baseline that cannot identify the bottleneck.

For metadata/header/GPS isolation, run ignored test profile_afile_new_for_directory with PICAIPIC_SCAN_PROFILE_DIR and optional PICAIPIC_SCAN_PROFILE_LIMIT. The 2026-07-27 D:\100k all-JPEG reference measured 10k and 100k at the same debug-test throughput: 43.7 files/sec, 0 failures. Treat that as AFile::new micro-benchmark evidence only, not full import throughput.
