---
name: change-dedup-scan
description: Exact/similar duplicate scanning, hash caches, group rebuilds, cancellation, and SQLite concurrency.
last_updated: 2026-08-29
---

# Change dedup scanning

## When to use

- Changing BLAKE3 exact duplicate scanning or dHash similar scanning
- Changing `file_hashes`, `file_phashes`, or duplicate group rebuilds
- Diagnosing `SQLITE_BUSY`, stalled indexing, or dedup cancellation

## Concurrency contract

- File reads, image decode, hashing, Hamming comparisons, clustering, and sorting run outside SQLite write transactions.
- Ready hash rows flush in bounded batches (`DEDUP_WRITE_BATCH_SIZE`, currently 256). A write transaction contains only prepared INSERT statements and commit.
- Cancellation flushes already-computed hash rows, but skips group rebuilding so the previous visible groups remain intact.
- Similar-group O(n²) planning runs before the atomic delete/insert transaction. Do not move it back under the writer lock.
- Dedup uses `t_sqlite::open_conn()` so WAL, foreign keys, synchronous mode, and the 5-second busy timeout match the rest of the application.
- Full album indexing and dedup scanning share `ActiveMediaScans`; their RAII guards are mutually exclusive and close the start-check race.
- Multiple different album scans may still coexist; the cross-subsystem gate is only between any album scan and dedup.
- `indexAlbum` must rethrow a gate rejection. `Content.vue` pauses the queued albums and shows the localized `blocked_by_dedup` warning instead of leaving indexing stuck in a running state.

## Touchpoints

| Area | Path |
|------|------|
| Hash/phash scan and group plans | `src-tauri/src/t_dedup.rs` |
| Shared scan RAII gate | `src-tauri/src/t_utils.rs` |
| Album command preflight | `src-tauri/src/t_cmds.rs` |
| Shared SQLite connection policy | `src-tauri/src/t_sqlite.rs` |
| Index queue rejection handling | `src-vite/src/common/api.js`, `src-vite/src/components/Content.vue` |

## Related Photos grouping strictness

- The Similar Photos ("Related Photos") panel groups by **dHash Hamming distance**, while "Find Similar Photos" ranks by **AI-search embedding cosine**. They are unrelated measures and must stay independently configurable.
- `SIMILAR_GROUPING_DISTANCES` maps the user-facing strictness (`config.dedup.similarGrouping`: 0/1/2) onto distances 6/8/12, resolved by `resolve_similar_grouping_distance`. Out-of-range values fall back to the default.
- The distance is applied **while clustering**, so changing it requires a rescan — the UI calls `triggerBackendDedup(true)` on change rather than trying to regroup in place.
- Exact duplicates are excluded from related-photo analysis (`file_phashes` rows whose `file_id` is in `duplicate_group_items`), otherwise every byte-identical pair is reported in both panels.

## Keep / unkeep contract

- `t_dedup::set_keep_in` designates exactly one keeper per group: it clears `is_keep` on every item, then sets it on the requested one, and marks the group `reviewed = 1`.
- `file_id <= 0` is a deliberate **unkeep**: the group is cleared with no replacement designated, so every candidate becomes selectable again. Without it a mis-clicked keeper could never be undone, which is why `changed == 0` may no longer be treated as an error unconditionally.
- `DedupPane` surfaces this as an Unkeep button on the keeper plus a Reanalyze button that forces `triggerBackendDedup(true)`, bypassing the per-mode `lastScanKeyByMode` memo that would otherwise reuse the previous scan.

## Delete contract

- Dedup trash uses a same-directory staged rename before touching SQLite.
- The final `is_keep=0` check, optional `is_selected=1` check, and `AFile` deletion share one `BEGIN IMMEDIATE` transaction.
- Guard, DB, or zero-row failures restore the staged source. Only a committed DB deletion proceeds to OS trash.
- Permanent dedup deletion is orchestrated by `Content.vue` through `batchDeleteFiles(..., true)`; `dedupDeleteSelected` is intentionally trash-only.
- File deletion cascades tags, collections, faces, and dedup memberships; a deleted face used as a person cover is replaced with another remaining face or NULL.

## Verify

- `cargo test --manifest-path src-tauri/Cargo.toml dhash_tests -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml active_media_scan_tests -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml dedup_delete_ -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml afile_crud_round_trip_uses_temporary_sqlite_fixture -- --nocapture`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- Large-library manual check: start dedup, confirm album indexing is rejected with an explicit busy message; cancel dedup and confirm completed cache rows resume on the next scan.
- Manual: set a keeper, then Unkeep it — every item in the group must become selectable again.
- Manual: Reanalyze must re-run the scan even when the same album/selection is already memoized.
