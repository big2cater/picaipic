---
name: audit-face-cluster-tx
description: Code-audit findings for face batch-write transaction, clustering assignment tx, and in-memory clustering scale in t_face.rs / t_cluster.rs / t_sqlite.rs. Read before changing face batch write or clustering paths.
triggers:
  - face batch write audit
  - clustering transaction review
  - silent batch drop
  - apply_scan_batch_with_conn change
  - cluster_faces memory scale
edges:
  - target: patterns/change-face-index.md
    condition: when a finding requires a face-index worker / clustering contract change
  - target: patterns/change-destructive-file-ops.md
    condition: when batch write failure must surface instead of being swallowed (F-A)
  - target: patterns/change-database-schema.md
    condition: when the sqlite batch tx needs busy_timeout / retry (T-1)
last_updated: 2026-07-30
---

# Audit: face batch-write tx / clustering (t_face.rs, t_cluster.rs, t_sqlite.rs)

Static audit of the face write path (`t_face.rs`), the clustering body (`t_cluster.rs::
cluster_faces`), and the shared batch transaction (`t_sqlite.rs::apply_scan_batch_with_conn`).
F-A and F-C are resolved; F-D remains a bounded-memory follow-up requiring representative
large-library measurement. Verified-safe checks are listed so they are not re-flagged.

## Findings

| ID | Severity | Location | Summary |
|----|----------|----------|---------|
| F-A | Resolved | `t_face.rs` batch flush | Batch transaction errors stop indexing and emit `face_index_finished.error`; no silent completion. |
| F-C | Resolved | `cluster_faces` assignment phase | Plan assignments outside the write lock, then atomically create people and link faces in one `BEGIN IMMEDIATE` transaction. |
| F-D | Medium (bounded follow-up) | `cluster_faces` `get_all_for_clustering` | All embeddings are still loaded for graph construction; Top-K and ANN bound edge memory, but a 100k+ RSS measurement is needed before a mmap/chunk design. |
| F-E | Low (verified safe) | `cosine_distance` / `parse_embedding` | Dimension mismatch → `2.0` (no false edge); bad bytes → `None` (skip, no panic). |
| F-F | Low (known trade-off) | ANN vs Exact mode | Auto picks HNSW above `CLUSTER_N_EXACT`; ANN vs Exact clusters may differ slightly. |
| T-1 | Verified safe | shared `open_conn` setup | Every pooled connection has a 5-second SQLite `busy_timeout`; F-A now surfaces a timeout/error rather than dropping the batch. |
| T-2 | Verified safe | `reorder` / `add_files` | `unchecked_transaction()` remains limited to freshly opened connections; unrelated to face indexing and retained as documented maintenance debt. |

## F-A — resolved: scan write failures abort visibly

The `main` loop flushes a batch like this:

```1001:1009:d:/ailab/PicAiPic/src-tauri/src/t_face.rs
        if batch.is_empty() {
            return;
        }
        match t_sqlite::Face::apply_scan_batch_with_conn(&db_conn, batch) {
            Ok(n) => *total_faces += n,
            Err(e) => eprintln!("Failed to apply face scan batch: {}", e),
        }
```

`apply_scan_batch_with_conn` is itself transactional (failure ROLLBACKs), so the **entire
batch of faces is lost**, yet the error only hits stderr, progress continues, and indexing
"finishes". If the failure is persistent (disk full / `SQLITE_BUSY` / corruption) the user
sees "done" with no faces.

`flush_face_scan_batch` now returns `Result`. A failed transactional batch clears the
coordinator's remaining work, requests cooperative worker shutdown, and emits the error in
`face_index_finished`; `Person.vue` displays it. The failed transaction rolls back all marks
and inserted faces, leaving affected files eligible for a later retry.

## F-C — resolved: atomic clustering commit

`t_cluster.rs::cluster_faces` step 9 calls `Face::assign_to_person` **per face**, committing
one row at a time. On cancel the function returns `Err`, but already-assigned faces are
persisted and new `persons` rows may already exist. Consistency is eventually restored via
"frozen existing labels + incremental seed" on rerun, but there is a **partial-cluster
window** that can fragment a cluster.

The CPU-heavy graph/whisper work stays outside a transaction. The assignment phase first
builds an in-memory plan, checks cancellation, then opens one immediate transaction that
creates new person rows and conditionally assigns every previously-unassigned face. Any
failure or concurrent face assignment rolls back the whole plan.

## F-D — deferred: all faces loaded for clustering

`cluster_faces` calls `Face::get_all_for_clustering()` to pull every face's embedding into
`slim_faces`, then builds `parsed_embeddings` and `candidate_lists`. At 100k+ faces ×
512-dim this is hundreds of MB, with a 2–3× peak during candidate construction.

ANN plus Top-K now bound graph edges to `N * K_NEIGHBORS`, and slim rows avoid bbox/metadata
loading, but `Vec` copies of embedding bytes and parsed `f32` vectors remain. Do not chunk
without preserving graph neighborhood correctness and seed semantics. First measure peak RSS
on a representative 100k-face library; consider an embedding mmap/cache if needed.

## F-E — distance / parse are fail-closed (verified safe)

`cosine_distance` returns `2.0` on dimension mismatch (greater than any valid distance, so
no false edge is created); `parse_embedding` returns `None` on bad bytes and the face is
skipped rather than panicking. ✅

## F-F — ANN approximate recall (known trade-off)

Auto mode selects HNSW when `n >= CLUSTER_N_EXACT`; the quality gate only asserts mean
Jaccard ≥ 0.5. ANN and Exact clusters may differ slightly. Expected behavior.

## T-1 — verified safe: shared busy handling

```8859:...:d:/ailab/PicAiPic/src-tauri/src/t_sqlite.rs
// BEGIN IMMEDIATE ... COMMIT / ROLLBACK
```

`open_conn` applies a five-second `busy_timeout` to every pooled connection. A lock that
outlasts it now reaches the F-A error path instead of disappearing. No extra retry loop is
needed because retrying a full batch after SQLite's own wait adds no stronger guarantee.

## T-2 — `unchecked_transaction()` usage

`reorder` (L9654) and `add_files` (L9683) use `conn.unchecked_transaction()`. Today each
call opens a fresh `conn` with no nesting, so it is safe. But `unchecked_transaction` does
not detect an already-active transaction on the connection; if refactored to reuse a
connection that already holds a transaction, it would silently return `Err`.

Fix: switch to checked `conn.transaction()`, or add a comment hardening the "fresh conn, no
nesting" invariant.

## Verify

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `face_write_transaction_tests` verifies a failed scan batch rolls back marks/faces and a
  failed cluster plan rolls back new people and face links.
- `cargo test --manifest-path src-tauri/Cargo.toml t_cluster:: -- --nocapture`
- Manual: force a write failure during face indexing and confirm the Person panel shows the
  terminal error and affected files remain unprocessed.

## Update Scaffold

- [x] Record scan-batch error propagation and atomic cluster assignment in `change-face-index.md`.
- [x] Record the F-D large-library memory measurement follow-up in this audit.
