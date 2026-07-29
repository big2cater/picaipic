---
name: profile-library-scan
description: Measure real library scan throughput before changing metadata, thumbnail, or embedding performance.
last_updated: 2026-07-29
---

# Profile Library Scan

Use `docs/guide/library-scan-profiling.md` before proposing scan performance changes. Compare cold, warm, and small-delta scans on the same media mix; preserve originals and do not run concurrent scans.

Opt-in scan profiling exposes wall phases plus batch-level embedding preparation,
engine, SQLite write, and fallback timings. Keep default scans free of per-file
timing/log overhead.

For metadata/header/GPS isolation, run ignored test profile_afile_new_for_directory with PICAIPIC_SCAN_PROFILE_DIR and optional PICAIPIC_SCAN_PROFILE_LIMIT. The 2026-07-27 D:\100k all-JPEG reference measured 10k and 100k at the same debug-test throughput: 43.7 files/sec, 0 failures. Treat that as AFile::new micro-benchmark evidence only, not full import throughput.

For a reproducible middle layer without app state, run ignored test profile_directory_index_and_thumbnails. On D:\100k at 200px, 1k and 10k measured 21.9/22.0 files/sec with zero failures; the 10k split was 51.6% temporary SQLite/index and 48.4% thumbnail codec. The fixture is serial and excludes app cache, worker concurrency, events, and embedding.

For end-to-end album scans, start the app with `PICAIPIC_SCAN_PHASE_PROFILE=1`. Compare count, traversal, cumulative index, drain tail, cumulative thumbnail work, cumulative embedding request latency, finalize, and total. Thumbnail work overlaps across permits. Embedding request latency also overlaps across up to eight callers and includes queue/reply wait; neither field may be summed into wall time or treated as pure codec/ONNX time. The historical 100k AI run (2026-07-27) took 9,548.281s, but its single-permit embedding path predates batching. The active 10,343-file mixed dataset (10,340 JPEG + 3 PNG) measured 1,716.720s after CPU and transaction batching, with 10,150 successful thumbnails and embeddings; the per-row batch run measured 2,121.0s and the 4-thread pre-batch run measured 2,410.6s. Per-file success logs require `PICAIPIC_EMBED_FILE_TRACE=1`.
## DirectML probe

The bundled Rust ONNX Runtime must stay on CPU until DirectML is isolated in a
separately packaged subprocess. On the test machine, registering or even querying
the bundled DirectML provider caused a native `STATUS_ACCESS_VIOLATION` during
model loading; Rust `Result` fallback cannot catch that class of crash.

For a CPU thread probe, set `PICAIPIC_AI_INTRA_THREADS` to a value such as `4` or
`8` before starting the app. The default remains `2`; compare total scan time and
embedding active time on the same dataset before changing the default.

Embedding decode must use content sniffing after the JPEG extension fast path;
some benchmark fixtures contain RIFF/WebP bytes under a `.jpg` name.

The scan embedding worker batches up to 8 images through the dynamic vision model
and commits successful vectors through one SQLite transaction. Preparation,
inference, or transaction failures retry through the single-file path, so compare
both throughput and embedding success count against the pre-batch baseline. The
transaction behavior has unit coverage and reduced the active dataset run from
2,121.0s to 1,716.720s (19.1%) with the same 10,150 successes.

The later warm-source diagnostic run took 1,025.576s. Do not compare that wall
time directly with cold runs; use its split evidence: 1,270 nearly full batches,
230.476s prepare, 776.918s engine, 2.247s SQLite write, zero fallback. Engine work
is the next measured bottleneck.

Use `embedding_prepare_seconds` for source decode/resize,
`embedding_engine_seconds` for tensor preprocessing + ONNX,
`embedding_write_seconds` for the batch transaction, and fallback fields for
single-file retries. These are recorded once per batch, unlike the overlapping
per-request `embedding_task_seconds` field.

Set `PICAIPIC_EMBED_BATCH_SIZE=16` for the next controlled probe. Accepted values
are 1-32 and invalid values use the product default of 8. Keep cache state and all
other settings fixed; compare capacity, batches, engine seconds, total, and
success count.

The first batch-16 run took 2,173.659s (646 batches, 448.216s prepare, 1,705.076s
engine, 9.271s write, zero fallback). It overlapped a 100k matrix search and an
unfinished background ANN build, so it is not a clean A/B result; it is still too
slow to promote. Keep default 8. Current builds keep ANN disabled by default;
leave `PICAIPIC_EMBED_ANN` unset and do not search mid-scan. Reimport only when a
controlled scan benchmark specifically requires it.

At startup, `embed_matrix warm ready ... n=... dim=512` means the process-local
search matrix was rebuilt from existing SQLite vectors. It is not a re-embedding
pass. Future cache work may persist this matrix, but any disk cache must be
versioned and atomically replaced after generation checks.

The 110,343-vector product validation logged `ann=disabled`; repeated `bird`
queries used exact `matrix=1` and returned 30 results without scheduling HNSW.
There is no default-path `embed_ann ready` milestone to wait for.

The first warm profile measured 3.150s total (1.217s SQLite, 1.912s build) and
0.06% process CPU in the post-settle window. This closes the persistent startup
CPU issue and does not justify disk persistence. `COUNT + exact reserve` reached
216.8 MiB but regressed total time to 5.846s, so it was removed. Current loading
uses one scan, borrowed SQLite BLOB slices, and one final vector shrink. One
restart-only time/memory remeasure remains.

Final matrix warm was 2.876s (SQLite 1.062s, build 1.804s), 216.8 MiB, and 0.00%
post-settle CPU. Startup matrix work is done; split preprocess from ONNX before
attempting another scan throughput change.

The engine split is implemented. `embedding_preprocess_seconds` covers resize,
RGB, normalization, and NCHW construction; `embedding_inference_seconds` covers
ORT input/run/output materialization; `embedding_engine_overhead_seconds` is the
aggregate remainder. Gather it on a real import or controlled small sample, not a
gratuitous full-library reimport.

The first complete split baseline finished in 687.636s with 10,150/10,150
embedding and thumbnail successes. It spent 239.678s in source preparation,
130.617s in NCHW preprocessing, 299.305s in ONNX inference, 0.003s in residual
engine overhead, and 1.937s writing. ONNX is the largest single phase, but source
preparation + preprocessing totals 370.295s, so the next change overlaps those
CPU stages with the current ONNX batch instead of changing model output.

The embedding worker now carries file path/type/orientation from `ThumbnailTask`
instead of reloading a full `AFile` per image. It holds at most two logical
batches: the single ONNX session consumes one preprocessed tensor while one next
batch decodes and builds NCHW outside the engine mutex. Batch size remains 8 and
all failed items retain single-file fallback. `embedding_prefetched_batches`
counts batches whose preparation started while the previous batch was running.
After prefetch, prepare/preprocess/inference are cumulative work timings that can
overlap; do not add them to predict wall time.

The homogeneous disposable 1,000-file validation completed in 47.788s with
970/970 embeddings, fallback 0, 123 batches, and 80 prefetched batches (7.89
items/batch). Cumulative prepare + engine + write was 65.071s, confirming useful
overlap. Do not claim an exact prefetch speedup without a matched prefetch-off A/B.

The first post-change smoke used the real heterogeneous `D:\Desktop` library. It
completed 1,255/1,255 embeddings with zero fallback and 36 prefetched batches,
confirming pipeline correctness. Do not compare its 672.863s wall time directly
with the homogeneous embedding benchmark: synchronous indexing consumed
668.406s, and 1,255 items fragmented into 915 batches (1.37 items/batch), so
embedding work was hidden behind a slow producer rather than being the wall-time
bottleneck. Keep it as the real-world index diagnostic case.

Synchronous index profiling is now implemented. With
`PICAIPIC_SCAN_PHASE_PROFILE=1`, use `index_folder_seconds`, fetch, stat,
metadata, refresh, write, refetch, assemble, other, and slow-file count to locate
the producer bottleneck. Cold metadata also reports file-info/inode, header,
dimensions, EXIF parse/extract, compatibility/RAW/binary fallbacks, Motion XMP,
HEIC, geocode, prompt, and assembly subphases. These selected subphases are
cumulative diagnostic counters, not a sum invariant. Set
`PICAIPIC_SCAN_SLOW_FILE_MS=250` when per-file outliers are needed; accepted
values are 1-600000ms. The default scan path does not take these per-stage clocks
or emit per-file logs.

For EXIF diagnosis, `index_metadata_exif_seconds` is split into the 128 KiB
pre-read parse (`index_metadata_exif_header_attempts` and
`index_metadata_exif_header_seconds`) and complete-file fallback
(`index_metadata_exif_file_fallback_attempts` and
`index_metadata_exif_file_fallback_seconds`). The fallback remains for incomplete
JPEG headers, headers with EXIF that permissive parsing did not decode, and the
rare no-header path; these counters show whether to investigate parser cost or
exceptional full-file I/O.

The header parse is further divided without changing its recovery behavior:
`index_metadata_exif_container_attempts` / `_container_seconds` cover the
generic EXIF container reader; `_signature_scan_seconds` covers the `Exif\0\0`
and TIFF signature walks; `_raw_attempts` / `_raw_seconds` cover the selected
slice's `to_vec` and raw EXIF reader together. Compare all three on one new
copied cold import before adding a no-EXIF JPEG marker fast path. Do not use a
variable `drain_seconds` result to redirect this isolated metadata probe.

The marker-path validation is accepted. On matched 1,000-file imports, signature
scans accounted for `4.420s` while container parsing was `0.110s` and raw parsing
was unused. Complete JPEG headers with no EXIF APP1 now skip the permissive reader;
all incomplete or EXIF-bearing cases retain it. Header EXIF fell `4.532s ->
0.048s`, metadata `16.753s -> 12.979s`, index `19.841s -> 16.170s`, and
thumbnail/embedding success remained `970/970`. Do not report the total
`38.064s -> 39.341s` as a regression: its independent drain tail changed
`15.953s -> 20.921s`.

Subsequent cold probes accepted three more marker-aware reductions. First, the
complete no-EXIF JPEG group now retains orientation 1 without a redundant binary
orientation scan; orientation extraction fell `4.086s -> 0.842s`. Second, Motion
profiling isolated `3.407s` in 1,000 generic header XMP searches. Complete JPEGs
without APP1 XMP namespace or `<x:xmpmeta>` now skip that search, while candidate,
incomplete, and HEIC cases retain tolerant parsing/fallback; Motion fell
`3.502s -> 0.149s`. Third, binary fallback profiling found TIFF signature scans
at `1.965s` across 1,000 headers but zero TIFF bases in the complete no-EXIF JPEG
group. That group now skips the impossible fallback, reducing binary work
`2.931s -> 1.004s`. The final validation had metadata `4.347s`, index `7.197s`,
and 970/970 derived-media successes. Do not use its `28.296s` drain tail to judge
these producer-only changes. Next profile the remaining capture fallback, TIFF
entry scan, or conservative orientation fallback before changing them.

The matched 1,000-file split showed `4.595s` of header parse and `5.012s` from
785 complete-file fallback attempts. All 995 JPEG headers reached SOS/EOI and
780 contained no EXIF APP1, so reopening those sources could not produce EXIF.
The scanner now skips only this complete-and-EXIF-free case; headers that are
incomplete, non-JPEG, or contain EXIF still retain the established fallback.
Validation reduced fallback to 5 attempts / `0.044s`, EXIF to `4.563s`, and
index to `19.694s`, with 970/970 derived-media successes.

The 1,000-file sample warm rescan took 2.022s with all 1,000 rows indexed and no
thumbnail/embedding tasks. Folder/fetch/stat/write were 0.380/0.461/0.115/0.455s;
metadata, refresh, refetch, and assemble were zero. This is a reuse check, not a
cold-import throughput result. The scan timestamp write is part of mark-and-sweep
correctness, so do not remove it based on this sample alone.

The corresponding `D:\Desktop` warm rescan took 2.643s, with folder/fetch/stat/
write at 0.505/0.610/0.145/0.545s and no slow files. The scan now caches `folder
path -> folder id` for its lifetime only. `index_folder_cache_hits/misses` must be
reported on the next same-album rerun; do not infer the win before that result.

The matched 1,000-file rerun validated it: 2.022s -> 1.677s total (-17.1%) and
1.417s -> 1.059s synchronous index (-25.3%), with 985 cache hits / 15 misses and
folder time 0.380s -> 0.008s. Next validate `D:\Desktop`; fetch + timestamp write
is the remaining warm cost, but preserve mark-and-sweep semantics before batching.

`D:\Desktop` then validated the cache at 2.643s -> 2.198s total and 1.813s ->
1.355s index, with folder time 0.505s -> 0.018s. Unchanged seen timestamps now
flush in transactions of at most 50 ids before a progress checkpoint. A failed
flush prevents stale cleanup. Check `index_seen_batch_items`, batches, and seconds
on the next warm reruns; these seconds are already included in `index_write_seconds`.

The matched 1,000-file batch result is accepted: 1,000 rows / 20 transactions,
write 0.454s -> 0.029s, index 1.059s -> 0.659s, and total 1.677s -> 1.303s.
The scan now preloads lightweight per-album file state so unchanged files avoid
full per-file SQLite fetches. Inspect `index_file_cache_preload_seconds`, rows,
hits, and misses along with wall time; misses, changed rows, missing thumbnails,
and preload failure must retain the existing full path. Re-run existing albums,
never delete/re-add one solely for this measurement.

`D:\Desktop` accepted the cache on 1,293 unchanged files: 0.014s preload,
1,293 hits, zero misses/fetch seconds, 2.016s -> 1.068s total, and 0.954s ->
0.182s index. Its next warm bottleneck is filesystem stat at 0.153s. Do not
replace that stat path until a larger representative run demonstrates a useful
wall-time opportunity.

The 10,343-file `D:\MultiModalKaggleDataset` rerun kept all cache hits after a
0.111s preload and zero fetch time. Its 207 50-item seen transactions took
1.130s, so the shared seen-write/recovery-checkpoint window is now 500. Verify
that the next same-album rerun reports about 21 batches and lower write time;
the seen commit must remain before the checkpoint and no album must be recreated.

The matched rerun accepted the window: 207 -> 21 seen batches, 1.130s -> 0.605s
seen-write time, and 10.164s -> 8.786s total. Stop warm-path changes here unless
a new representative profile justifies them: the remaining 1.311s stat time is
the deliberate cost of correct changed-file detection.

The copied 1,000-file cold sample `D:\PicAiPicColdImportTest-20260729-115336`
took 44.662s, with 39.991s of synchronous index and 37.379s of new-file
metadata. Its 40.972s cumulative embedding engine work overlapped indexing, so
do not retune ONNX or batch size from this result. Make the next cold benchmark
from a new copied directory, not by deleting or recreating an existing album.

The matching profile `D:\PicAiPicColdImportTest-20260729-121925` took 45.172s
and made binary EXIF fallback the clear cold-path target at 17.172s (EXIF parse
7.998s; Motion XMP 4.731s). The fallback is now a single tolerant header pass
that collects all requested tags. It must retain complete-EXIF precedence, Sony
orientation, and Apple ContentIdentifier behavior. Validate with a third copied
directory and compare the fallback counter plus thumbnail/embedding successes.

The third copied import accepted binary fallback: `17.172s -> 2.372s` (86.2%),
index `40.338s -> 24.591s`, and 970/970 derived-media successes. Its faster
producer exposed `16.693s` drain time, so keep producer and drain timings
separate. Motion XMP is now the next measured metadata I/O: reuse the pre-read
JPEG header only if a complete marker walk reaches SOS/EOI, otherwise retain the
512 KiB file fallback. HEIC always keeps its existing fallback. Validate this
change with a new copied directory before further parser or concurrency work.

The Motion validation finished in `43.739s`: index fell again to `22.618s` and
Motion XMP to `3.067s`, but drain grew to `18.967s`. A follow-up that collected
the next batch throughout inference looked promising by coverage (121/122
prefetched batches) but failed product validation: the copied sample took
`100.724s`, drained for `73.209s`, and completed only 962/970 embeddings. An
open channel waiting for a full tail batch delayed replies into the 60s timeout.
The next warm rescan completed in `0.758s` with zero derived-media tasks, proving
those timed-out callers had still persisted their vectors; the failure was reply
latency/progress accounting, not lost embeddings. The change is reverted. Retain
opportunistic full-batch prefetch and the normal 3ms tail coalescer; do not use
prefetch coverage alone as a performance metric.

Before changing that scheduling again, profile mode splits each embedding request
into `embedding_permit_wait_seconds`, `embedding_send_seconds`, and
`embedding_reply_seconds`, plus send/reply timeout counts. These are cumulative
across requests and must not be added to wall time. Run one fresh copied cold
sample with no batch/thread/permit override, then target only its dominant wait.

The clean sample measured `10088.257s` cumulative permit wait, `0.002s` channel
send, `720.324s` reply, zero timeouts, and `19.902s` drain. The controlled
3-batch probe retained 970/970 successes with no timeout, lowered permit wait to
`5816.884s`, drain to `13.601s`, and total to `41.239s`. It is accepted as the
product default. `PICAIPIC_EMBED_INFLIGHT_BATCHES` remains bounded to 2-4 for
diagnostics; normally leave it unset. The third batch is queued only and does not
add an ONNX session or full-tail wait.

## External file-index comparisons

Do not compare PicAiPic initial-import throughput directly with launcher-style
file search. Flow Launcher commit `7a651ce9` delegates global filename discovery
to Everything or Windows Search, asks those persistent indexes for at most 100
paths, streams/cancels the small result set, and hydrates no media metadata. Its
direct directory enumeration is a path-navigation fallback. Its program plugin
only enumerates configured executable suffixes and debounces filesystem watcher
events before rebuilding that much smaller program list.

PicAiPic already applies the transferable incremental idea at startup:
`start_folder_mtime_sync` stats known folders, queues only dirty folders, and
processes new/changed files without rescanning clean folder contents. A full
initial import still must parse media, generate previews, and compute CLIP
embeddings. Everything or Windows Search can at most replace the negligible file
discovery portion; it cannot remove those costs.

Keep Everything/Windows Search optional if a future Windows-only discovery
adapter is justified by a profile with millions of paths or very many folders.
Preserve direct traversal as the cross-platform source of truth and retain a
periodic reconciliation path because watchers and external indexes can be
disabled, stale, overflow, or omit unsupported volumes.

## Next performance work

1. Keep the accepted warm path as-is. Capture a new profile only when media or
   filesystem conditions differ materially; do not delete or recreate an album.
2. Focus future performance work on cold import thumbnail/decode/embedding work,
   or use `[scan-slow-index]` evidence before changing concurrency.
3. Prefetch correctness and overlap are validated by the 1,000-file homogeneous
   result; run a matched prefetch-off A/B only if an exact speedup is required.
4. Keep batch 8 and two intra-op threads until a controlled result wins.
5. Probe DirectML only in a separately packaged Python/runtime subprocess. The
   in-process Rust provider crashed with native `STATUS_ACCESS_VIOLATION`.
6. Re-run cold, warm, and small-delta benchmarks after each change; report wall
   time and success counts together.
