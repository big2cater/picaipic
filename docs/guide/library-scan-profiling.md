# Library Scan Profiling

Updated: 2026-07-29

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
| D:\MultiModalKaggleDataset | 10,343 | 10,340 JPEG + 3 PNG | local benchmark | 10,150 thumbnails/embeddings succeeded |
| D:\PicAiPicPrefetchTest-20260728-235617 | 1,000 | homogeneous disposable sample | local benchmark | 970 thumbnails/embeddings succeeded; bounded-prefetch validation |

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
- index_folder_seconds: parent-folder lookup/insert time.
- index_folder_cache_hits / index_folder_cache_misses: scan-local folder-id cache
  reuse and DB-resolution count. They should add up to successful folder lookups.
- index_fetch_seconds / index_stat_seconds: existing-row lookup and source-file stat time.
- index_metadata_seconds: new-file `AFile::new` metadata/header/media parsing time.
- index_metadata_*_seconds: opt-in cold-import subdivision of that metadata work:
  file info/inode, header read, dimensions, EXIF parse and extraction, legacy
  capture fallback, RAW metadata, binary fallback, Motion XMP, HEIC video,
  reverse geocode, AI prompt, and final row assembly. These are cumulative
  selected stages; use them to rank work, not as a sum that must equal metadata.
- index_refresh_seconds: changed-file metadata refresh time.
- index_write_seconds / index_refetch_seconds: SQLite mutation and post-insert row lookup time.
- index_seen_batch_items / index_seen_batch_batches / index_seen_batch_seconds:
  unchanged-file scan timestamps committed in bounded transactions. This time is
  included in `index_write_seconds`.
- index_assemble_seconds / index_other_seconds: follow-up task construction and unclassified synchronous index time.
- index_slow_files: files whose total synchronous index time crossed the optional slow-file threshold.
- drain_seconds: wall time spent waiting for thumbnail/embedding tasks after traversal ends.
- thumbnail_task_seconds: cumulative thumbnail service time after acquiring a worker permit; workers overlap, so it can exceed wall time.
- embedding_task_seconds: cumulative request latency across up to eight callers, including batch queue/reply wait; it is not pure engine time.
- embedding_permit_wait_seconds: cumulative time waiting to enter the bounded embedding pipeline after thumbnail work completes.
- embedding_send_seconds / embedding_reply_seconds: cumulative mpsc channel-send and worker-reply waits after the permit is held. Use them with the timeout counts to distinguish queue backpressure from late batch completion.
- embedding_send_timeouts / embedding_reply_timeouts: requests that exceeded the 60-second send or reply limit. A timeout does not by itself prove a vector was not later persisted; confirm with a warm rescan.
- embedding_batch_capacity / embedding_inflight_batches / embedding_batches / embedding_prefetched_batches / embedding_batch_items / embedding_prepared_items: configured size, bounded in-flight batch count, batch count, batches prepared ahead of ONNX execution, requested files, and files successfully decoded for batch inference.
- embedding_prepare_seconds: source image decode/resize preparation time. Current requests carry path/type/orientation directly and do not reload a full `AFile` row.
- embedding_engine_seconds: aggregate CLIP preprocessing plus ONNX work for comparison with older profiles. Its component work can overlap in the prefetched pipeline.
- embedding_preprocess_seconds: source cap/final 224 resize, RGB conversion, normalization, and contiguous NCHW construction outside the AI engine mutex.
- embedding_inference_seconds: ONNX input wrapping, vision session execution, output extraction, and 512-d result materialization.
- embedding_engine_overhead_seconds: aggregate engine time minus preprocessing and inference; primarily engine-lock wait and call bookkeeping.
- embedding_write_seconds: successful batch SQLite transaction time, including matrix-cache invalidation.
- embedding_fallback_attempts / embedding_fallback_seconds: files retried through single-file generation and their cumulative elapsed time.
- finalize_seconds: stale-row cleanup, Live/Motion pairing, recount, progress finalization, and cover selection.
- total_seconds: end-to-end worker wall time.

Use the attempt/success counters beside each cumulative phase to identify failures and normalize averages. The split embedding fields are measured once per batch. When `embedding_prefetched_batches` is nonzero, next-batch prepare/preprocess overlaps current-batch inference, so these cumulative fields must not be added to estimate wall time. Keep the same thumbnail size and AI settings when comparing cold and warm runs.

To identify outliers without default per-file log noise, enable phase profiling and
set an explicit threshold before app start:

```powershell
$env:PICAIPIC_SCAN_PHASE_PROFILE = "1"
$env:PICAIPIC_SCAN_SLOW_FILE_MS = "250"
cargo tauri dev
```

Accepted slow thresholds are 1-600000 milliseconds. `[scan-slow-index]` reports
the same folder/fetch/stat/metadata/refresh/write/refetch/assemble split for only
the files at or above the threshold. With profiling unset, the normal scan path
does not take these per-stage clocks.

The first 100k production run on 2026-07-27 used AI search indexing and completed with no index, thumbnail, or embedding failures:

| Files | Traversal | Index cumulative | Drain tail | Total | End-to-end throughput |
|---:|---:|---:|---:|---:|---:|
| 100,000 | 3,233.415s | 3,111.840s | 6,313.095s | 9,548.281s | 10.47 files/sec |

The run showed a serial embedding bottleneck and is now a historical pre-batch baseline. Later builds batch up to eight images per vision inference. On `D:\MultiModalKaggleDataset`, the transaction-batch build completed in `1716.720s`; the per-row batch build took `2121.0s` and the 4-thread pre-batch comparison took `2410.6s`, with `10150/10150` thumbnail and embedding successes throughout. The transaction result is 19.1% faster than per-row batch writes and 28.8% faster than the 4-thread pre-batch run. `embedding_task_seconds` is cumulative request latency across up to eight callers and includes queue/reply wait; it is not pure ONNX service time. A `.jpg` file with RIFF/WebP content and CMYK/YCCK JPEGs exercise content-sniffing and generic-decoder fallbacks; evaluate final success counts, not log count alone. Successful per-file embedding source logs are disabled by default; set `PICAIPIC_EMBED_FILE_TRACE=1` only for targeted diagnosis.

A later warm-source diagnostic repeat completed in `1025.576s` with the same
`10150/10150` successes. It is not a code-speed comparison because filesystem and
thumbnail caches were warm. Its batch-level split is useful: 1,270 nearly full
8-image batches spent `230.476s` preparing images, `776.918s` in CLIP
preprocessing + ONNX, `2.247s` writing SQLite, and `0s` in fallback. The engine
therefore owns 75.8% of that run while SQLite write is only 0.2%.

The first complete preprocess/inference split run then completed in `687.636s`
with the same `10150/10150` successes. Its 1,271 batch calls spent `239.678s` in
source preparation, `130.617s` constructing NCHW, `299.305s` in ONNX inference,
`0.003s` in residual engine overhead, `1.937s` writing, and `0s` in fallback.
ONNX is the largest single component, but preparation + preprocessing totals
`370.295s`. This is the baseline for the bounded one-batch prefetch change; do
not attribute the difference from older warm runs to one change because cache and
intermediate build state were not controlled across all historical runs.

The worker now passes path/type/orientation from the existing thumbnail task,
avoiding a full `AFile` lookup per image. It preprocesses at most one next batch
outside the AI mutex while the unchanged single ONNX session handles the current
batch. Batch size remains 8 and all failures keep single-file fallback.
`embedding_prefetched_batches` reports how many batches entered this overlap
path. Real end-to-end improvement is pending the next natural import; no extra
full-library reimport is required only for this measurement.

A post-change correctness smoke used the real heterogeneous `D:\Desktop`
library. It is valid safety and workload evidence: 1,255/1,255 embeddings
succeeded, fallback was zero, and 36 batches were prefetched. It is not a direct
comparison with the homogeneous embedding benchmark. The 1,293-file scan spent
668.406 of 672.863 seconds in synchronous indexing, while 1,255 embedding
requests fragmented into 915 batches (1.37 items per batch). Prefetch work was
therefore hidden behind a slow producer. Keep this album as a real-world index
bottleneck case; use a comparable homogeneous import to measure prefetch speed.

The next homogeneous disposable sample supplied the representative prefetch
validation: 1,000 files completed in `47.788s`, with 970/970 embeddings, zero
fallback, 123 batches, and 80 prefetched batches. Average fill was 7.89 items per
batch. Cumulative prepare + engine + write work was `65.071s`, greater than wall
time, which confirms useful overlap. This is not an exact speedup claim
because the same sample was not run with prefetch disabled under matched cache
conditions.

A warm rescan of that same 1,000-file album completed in `2.022s`: all 1,000
rows were indexed, with no thumbnail or embedding tasks scheduled. The split was
`index_folder=0.380s`, `index_fetch=0.461s`, `index_stat=0.115s`, and
`index_write=0.455s`; metadata/refresh/refetch/assemble were zero. This confirms
that existing thumbnails and embeddings are reused. The remaining warm producer
cost is folder/SQLite bookkeeping, including the scan timestamp write needed for
mark-and-sweep cleanup.

The scan worker now keeps a scan-local `folder path -> folder id` cache. It is
discarded at scan completion and changes neither folder rows nor database schema.
This removes repeated `AFolder::add_to_db` calls for sibling files while retaining
the existing first lookup/insert behavior. The same 1,000-file warm rescan after
this change completed in `1.677s`: `index_seconds` fell from `1.417s` to
`1.059s` (25.3%), while total fell from `2.022s` to `1.677s` (17.1%). It recorded
15 misses and 985 hits; folder time fell from `0.380s` to `0.008s`. This is a
matched warm-sample result. SQLite fetch + timestamp write now account for
`0.926s` of the `1.059s` index time; validate the existing `D:\Desktop` album
before changing those mark-and-sweep writes.

Unchanged-file timestamp writes are now deferred to bounded 50-item batches.
Each batch is committed before the existing scan-progress checkpoint is persisted;
a batch failure stops the scan before stale-row cleanup. This keeps crash/resume
and mark-and-sweep behavior safe while avoiding a separate SQLite transaction per
unchanged file. New, changed, and thumbnail-missing files retain their existing
immediate writes. Measure the same warm albums before claiming a speedup.

The matched 1,000-file warm run validated those batches: all 1,000 unchanged
rows flushed as 20 transactions. `index_write_seconds` fell from `0.454s` to
`0.029s` (93.6%), index fell from `1.059s` to `0.659s` (37.8%), and total fell
from `1.677s` to `1.303s` (22.3%). SQLite fetch is now the largest warm index
phase (`0.510s`); validate `D:\Desktop` before optimizing that lookup path.

The scan now preloads a lightweight per-album state cache before traversal. Each
entry retains only the file id, basename/folder key, modified time, thumbnail and
embedding flags, orientation, size, dimensions, and duration. Unchanged files
therefore avoid the former per-file full `AFile` SQLite query with joins; they
still `stat` the source path and use the bounded seen-timestamp batches. New,
changed, and missing-thumbnail files retain the established full lookup/update
path, and a preload failure falls back to that path. The cache is dropped at the
end of the scan and is not persisted. The next warm rescan reports preload time,
row count, and file-cache hits/misses; no speedup is claimed until that result.

`D:\Desktop` accepted the change on 1,293 unchanged files: total time was
`1.068s` and synchronous index time `0.182s`, down from the post-seen-write
`2.016s` and `0.954s` reference (47.0% and 80.9%). The `0.014s` preload produced
1,293 rows and 1,293 hits with zero misses; per-file `index_fetch_seconds` was
`0.000`. File stat (`0.153s`) is now the dominant warm index operation, followed
by the 26 seen-write batches (`0.047s`). This is an accepted cross-layout result;
do not optimize the stat path without a new representative profile.

A subsequent 10,343-file warm rescan confirmed the cache remains effective at
larger scale: preload was `0.111s`, all 10,343 rows hit, and fetch stayed at
`0.000s`. It also exposed the next bottleneck: 207 50-item seen transactions
took `1.130s`, while file stat took `1.536s`. Seen writes and their matching
progress checkpoint now use a 500-file window. The seen transaction still commits
before the checkpoint, so stale cleanup remains protected; after interruption,
at most 499 additional files need to be revisited. Measure the same existing
album again before claiming the transaction reduction.

That matched rerun completed in `8.786s` (from `10.164s`, 13.6% faster).
`index_seen_batch_batches` fell from 207 to 21 and write time from `1.130s` to
`0.605s` (46.5%). All 10,343 rows again hit the file-state cache, with no
thumbnail or embedding work scheduled. File stat is now the dominant warm index
cost at `1.311s`; retain the per-file stat for correct changed-file detection
across supported local and removable paths rather than trading that invariant for
a more complex optimization.

For controlled batch-size probes, set `PICAIPIC_EMBED_BATCH_SIZE` to `1`-`32`
before starting the app. The product default remains `8`; invalid values fall
back to `8`. Compare `embedding_batch_capacity`, engine seconds, total seconds,
and success counts on the same cache state.

The first batch-16 probe completed in `2173.659s` with all 10,150 embeddings, but
it overlapped a 100k matrix query and an unfinished background ANN build. Its
`1705.076s` engine time failed the performance gate even though the run is not a
clean A/B comparison. Keep the default at 8. Current builds disable ANN by
default, so future scan benchmarks should keep `PICAIPIC_EMBED_ANN` unset and
avoid searches during the measured run. Do not delete/reimport an album unless
the explicit purpose of the experiment is measuring the scan path.

When the app prints `embed_matrix warm ready db=... n=... dim=512`, it is loading existing 512-dimensional vectors from SQLite into a process-local matrix for search. It does not decode or re-embed the library on every startup. Persisted, versioned matrix/cache files are a future optimization.

The startup line now ends with `ann=disabled`. HNSW is not built merely by opening
or searching a 100k library; exact matrix scoring remains active (`matrix=1`). Set
`PICAIPIC_EMBED_ANN=1` only for an explicit ANN probe. That mode schedules a
2-thread background build after the first search; override it with
`PICAIPIC_EMBED_ANN_BUILD_THREADS=1..8`.

The product path was manually verified with 110,343 vectors: startup logged
`n=110343 dim=512 ann=disabled`, and repeated `bird` searches scored all 110,343
candidates through `matrix=1` and returned the same 30-result set. There is no
`embed_ann ready` milestone to wait for in the default configuration.

The first detailed warm profile completed in `3.150s`: SQLite step + BLOB copy
used `1.217s`, vector parse + norm construction used `1.912s`, and other work used
`0.021s`. The five-second post-settle sample reported `0.06%` process CPU, so the
old persistent 90% startup CPU issue is closed and a disk cache is not justified
for latency. Matrix allocation was `257.5 MiB` versus about `216.8 MiB` of live
ids/norms/f32 data. A `COUNT(*) + try_reserve_exact` follow-up reached `216.8 MiB`
but regressed warm time to `5.846s`, so the double-scan design was removed.
Current code performs one SQLite scan, parses borrowed BLOB slices instead of
copying each row into a temporary `Vec<u8>` (about `215.5 MiB` cumulative bytes),
then shrinks the three finished vectors once. Remeasure `sqlite_seconds`,
`build_seconds`, `matrix_mib`, and total time by restart only.

Final remeasure: `2.876s` total (`1.062s` SQLite, `1.804s` build), `216.8 MiB`
matrix allocation, and `0.00%` post-settle CPU. This is 8.7% faster than the
3.150s baseline and saves 40.7 MiB. Startup matrix work is complete; do not add a
disk cache without a new materially worse measurement.

## What Flow Launcher's File Search Actually Optimizes

Flow Launcher was reviewed at commit
`7a651ce9cceeca5596ea74cf806277cb2de42c8b`. Its fast global file search does not
perform a PicAiPic-style import:

- The Explorer plugin selects Everything or Windows Search as its index provider.
- Everything queries the already-running Everything service through its SDK/IPC,
  sets a maximum result count, and retrieves only full paths and result types.
- Windows Search generates an indexed AQS/OLE DB query selecting file name, URL,
  and item type. Both providers default to at most 100 results.
- Results are streamed with cancellation checks. Direct recursive filesystem
  enumeration is a path-navigation fallback, not the normal global search path.
- The Program plugin scans only configured executable suffixes. Filesystem
  watchers debounce changes for 500 ms and trigger a refresh of that smaller
  program index.

The transferable principle is to separate cheap discovery from expensive
hydration and to update incrementally. PicAiPic already does this for steady-state
library changes: startup checks stored folder mtimes, scans only dirty folders,
and schedules thumbnail/embedding work only for affected files. Its initial
import still has to parse image/RAW/video metadata, create previews, and run CLIP
vision inference. Flow Launcher does none of those operations.

Accordingly, Everything or Windows Search would only replace the filesystem
discovery slice. On the active 10,343-file benchmark, `count_seconds` is about
`0.02s`, while hundreds of seconds are spent in metadata/decode/inference. A
required external index would add Windows/service coupling without addressing
the measured bottleneck and would violate the cross-platform product path.

A future Windows-only adapter remains reasonable only if a representative
million-path or very-high-folder-count profile shows folder stat/traversal cost is
material. It must be optional, fall back to direct traversal, and retain periodic
reconciliation because filesystem watchers and external indexes can miss changes
or be unavailable.

## Cold Import Follow-up

The next copied cold import should read the EXIF split fields alongside the
existing aggregate: `index_metadata_exif_header_seconds` measures parsing the
128 KiB pre-read buffer, while `index_metadata_exif_file_fallback_seconds`
measures complete-file fallback. Their corresponding `*_attempts` fields provide
the denominator before choosing a parser or I/O optimization.

The first split sample measured `4.595s` in 1,000 header parses and `5.012s`
across 785 complete-file fallbacks. Its JPEG marker walk found 995 complete
headers, 780 without EXIF APP1. The scanner now skips only this proof-positive
empty case, retaining fallback for incomplete/non-JPEG/EXIF-present headers.
The matched copied sample reduced fallback to 5 attempts / `0.044s`, EXIF
`9.611s -> 4.563s`, index `25.881s -> 19.694s`, and retained 970/970 thumbnail
and embedding successes. The overall run fell `42.949s -> 40.371s`; the smaller
wall-clock gain reflects a higher drain tail (14.588s -> 18.234s), not a metadata
regression.

The next profile keeps EXIF behavior unchanged but breaks
`index_metadata_exif_header_seconds` into the permissive reader's container
parse (`index_metadata_exif_container_attempts` / `_seconds`), `Exif\0\0` and
TIFF signature walks (`index_metadata_exif_signature_scan_seconds`), and raw
slice copy plus reader (`index_metadata_exif_raw_attempts` / `_seconds`). Use a
fresh copied cold import to decide whether the general reader, linear scan, or
copy/raw parse dominates before implementing any no-EXIF JPEG marker shortcut.

That import isolated the signature walks at `4.420s`, against `0.110s` for the
container reader and zero raw parses. The scanner now uses the existing complete
JPEG marker proof before permissive parsing: a header reaching SOS/EOI without
EXIF APP1 returns no EXIF immediately. Incomplete/non-JPEG/EXIF-bearing headers
retain the prior recovery and fallback behavior. The matched 1,000-file validation
reduced header EXIF `4.532s -> 0.048s`, metadata `16.753s -> 12.979s`, and index
`19.841s -> 16.170s`, with 970/970 thumbnail and embedding successes. Wall time
was `38.064s -> 39.341s` only because drain independently rose `15.953s ->
20.921s`; it is not evidence against this isolated metadata improvement.

A new copied 1,000-file sample, `D:\PicAiPicColdImportTest-20260729-115336`,
completed in `44.662s` (970/970 thumbnail and embedding successes). Traversal
was producer-bound: index `39.991s`, of which new-file metadata was `37.379s`.
Embedding engine work was `40.972s` cumulatively but overlapped traversal, so it
is not the next wall-time target. The sample had no preload cache rows, as
expected for a first import.

The matching cold sample `D:\PicAiPicColdImportTest-20260729-121925` completed
in `45.172s` with the subphase profile: binary EXIF fallback `17.172s`, EXIF
parse `7.998s`, Motion XMP `4.731s`, EXIF field extraction `3.438s`, and legacy
capture fallback `1.393s`. The binary fallback previously made repeated linear
passes over the same 128 KiB header for individual tags. It now collects all
needed fallback fields in one pass, retaining the existing full EXIF priority,
Sony orientation, and Apple Live Photo ContentIdentifier behavior.

Keep batch size 8, two ONNX intra-op threads, the 1024px source cap, and ANN
disabled. Create another copied test directory for one clean cold import; do not
delete or recreate any existing album. Confirm binary fallback time and success
counts before considering the next measured target, Motion XMP.

The binary rerun reduced synchronous index from `40.338s` to `24.591s` and
binary fallback from `17.172s` to `2.372s` (86.2%). Overall wall time changed
from `45.172s` to `43.387s`; the faster producer exposed a `16.693s` drain tail,
so metadata reductions alone no longer translate one-for-one to wall time. The
next metadata change reuses the existing 128 KiB JPEG header for Motion XMP. It
skips the old 512 KiB reopen only after a complete JPEG marker walk reaches
SOS/EOI; truncated headers and HEIC retain the existing file-read fallback.

The Motion-header validation completed in `43.739s`: index fell to `22.618s`
and Motion XMP to `3.067s`, but `drain_seconds` rose to `18.967s`. A follow-up
that received the next batch throughout inference is rejected: despite 121/122
prefetched batches, its copied sample regressed to `100.724s`, `73.209s` drain,
and 962/970 embedding successes. Waiting for an open tail batch to reach eight
items delayed replies into the 60s embedding timeout. The code is restored to
opportunistic full-batch prefetch plus normal 3ms tail coalescing. Do not use
prefetch coverage alone as a throughput result. A following warm rescan completed
in `0.758s` with zero thumbnail or embedding tasks, confirming that those timed-
out callers had still persisted their vectors.

Profile mode now splits post-thumbnail embedding latency into permit, channel
send, and worker reply waits with separate timeout counts. Leave the accepted
opportunistic prefetch unchanged for the next copied cold sample; choose any
subsequent change from the largest wait field rather than from prefetch coverage.

The clean sample isolated permit wait as the dominant queue constraint:
`10088.257s` cumulative permit wait versus `0.002s` channel send and `720.324s`
reply, with zero timeouts. The matched three-batch probe retained 970/970
successes and zero timeout, reducing permit wait to `5816.884s`, drain to
`13.601s` (-31.7%), and total to `41.239s` (-11.0%). It is accepted as the
default. `PICAIPIC_EMBED_INFLIGHT_BATCHES` remains bounded to 2-4 for diagnostics
and normally stays unset. It admits one bounded queued third batch without a
second model session or a full-tail wait.

## Cold metadata marker-path follow-up

Three later copied 1,000-file cold imports accepted further producer-only
metadata reductions. Complete JPEG headers already proven to lack EXIF now keep
default orientation 1 without another binary scan; orientation extraction fell
from `4.086s` to `0.842s`. Motion profiling then attributed `3.407s` of its
`3.502s` total to generic XMP header searches. A complete JPEG whose APP1
segments contain neither the XMP namespace nor `<x:xmpmeta>` now skips that
search; candidates, incomplete headers, and HEIC retain tolerant parsing and
file fallback. Motion fell to `0.149s`.

Binary fallback profiling found `1.965s` in TIFF signature scans across 1,000
headers, but no TIFF base among complete no-EXIF JPEGs. Only that proven-empty
group now skips binary fallback; EXIF-bearing, incomplete, and non-JPEG headers
retain it. The validation reduced binary fallback `2.931s -> 1.004s`, metadata
`6.689s -> 4.347s`, and synchronous index `10.024s -> 7.197s`, while thumbnails
and embeddings remained `970/970`. The `28.296s` drain tail is independent and
must not be credited to metadata work. Profile capture fallback, TIFF entry
scanning, or the remaining conservative orientation fallback before changing
another parser path.

## Decision Rules

- Metadata dominates: profile `AFile::new` and file opens on the real mix before changing parser fallback order.
- Preview drain dominates: split by JPEG/RAW/video and inspect timeout/failure counts before raising worker budgets.
- Search preparation dominates: measure decode and single-session CLIP throughput separately; do not assume the scan loop is at fault.
- Warm rescan remains slow: inspect DB lookups, stale-row cleanup, Live Photo pairing, and thumbnail cache hit rate.

Report the hardware, dataset, commands, raw timings, and any limitations with each conclusion.
