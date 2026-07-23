---
name: clip-b16-default-bump-design
description: Replace bundled default image-search model CLIP ViT-B/32 quant with Xenova CLIP ViT-B/16 int8/quantized; hard-cut per-library embeds via app_meta. No SigLIP this track.
last_updated: 2026-07-23
---

# Design: Default image-search model bump (CLIP B/32 → B/16 int8)

## Status
**Abandoned as product default (owner 2026-07-23):** B/16 quant was prototyped on `feat/clip-b16-default-bump` and **subjectively ≈ B/32** for library search. **Do not merge** that branch as the app default. Keep bundled **CLIP B/32**. Historical design below may still inform a future optional pack or Track B scaffolding (`app_meta`, hard-cut, rebuild IPC) if needed — not a default model swap.

Track naming relative to `.mex/patterns/change-image-search-model.md`:
- **Track A** (thresholds + score histogram) — **shipped**.
- **Track B0** (this design) — **abandoned** for product default after owner trial.
- **Track B** (multilingual SigLIP sideload) — deferred; the real quality path if pursued.

## Goals / non-goals

### Goals
1. Installer default vision+text+tokenizer = **CLIP ViT-B/16 int8/quantized** (Xenova export lineage), same resource filenames as today.
2. Logical model id **`clip-b16`**. Legacy libraries with no meta or old vectors treated as **`clip-b32`**.
3. Per-library SQLite **`app_meta.embedding_model_id`**; mismatch → **fail closed**, clear embeds, rebuild — never silent rank with wrong space.
4. Keep preprocess **224 + CLIP mean/std**, embedding dim **512**, family **`clip`** so engine changes stay small.
5. Disk size expected near current B/32 quant pack (owner observation); not a product blocker.

### Non-goals
- SigLIP / BGE / multi-model Settings dropdown / pack download UI.
- Keeping B/32 and B/16 dual-active or silent dual-space ranking.
- Claiming a large jump in Chinese free-text quality (still CLIP).
- Fixing empty grids via new thresholds (Track A already landed: floors `0.30/0.26/0.22/0.18`, smart-tag `0.22`, histogram logs).
- Committing large ONNX binaries into git.

## Context (code truth)

| Piece | Today |
|-------|--------|
| Download | `scripts/download_models.ps1` / `.sh` → Xenova `clip-vit-base-patch32` `*_quantized.onnx` + OpenAI patch32 `tokenizer.json` |
| Engine | `t_ai.rs` — ORT sessions, resize 224, CLIP mean/std, `pixel_values` / flexible text outputs |
| Search | `AFile::search_similar_images` — no model-id gate; cosine over LE f32 blobs |
| Skip guard | `generate_embedding`: non-empty embeds → `"Embedding already exists"` |
| Cosine | `cosine_similarity_blob` already L2-normalizes query and file vectors at score time (thresholds stay cosine-scale even if ONNX raw vectors are unnormalized) |
| Multilingual | `imageSearch.model=1` swaps **text+tokenizer only**; vision stays bundled |
| Schema | Migrations through **v8**; next free = **v9** |
| Track A | Floors + histogram log line already in `search_similar_images` |

Owner note: existing test libraries may be deleted; product path still hard-cuts, not “dev-only wipe”.

## Decision summary

| Decision | Choice |
|----------|--------|
| Approach | **Plan B** (bundled default replace + library hard-cut meta), not A (file-only) or C (full registry UI) |
| Default id | `clip-b16` |
| Legacy id | missing / unknown / pre-B0 embeds → treat as `clip-b32` (stale) |
| Multilingual text-only (`model=1`) | **Rust reject + UI hide** while default vision is B/16 (misaligned dual tower) |
| SigLIP | Out of scope for B0 |
| Thresholds | Keep Track A numbers; re-measure with existing histogram after rebuild if needed |
| ONNX in git | No; scripts + packaging only |

## Architecture

```
Settings / library open / AI search entry
        │
        ▼
ensure_embedding_space_ok(active_id=clip-b16)
  1) Models loadable? (resource ONNX + session already ok)
  2) Read app_meta.embedding_model_id (default clip-b32 if missing)
  3) Match → OK
  4) Mismatch → INDEX_STALE path:
       a) Confirm models loadable (never clear if load would fail)
       b) COMMIT: UPDATE afiles SET embeds = NULL
       c) COMMIT: upsert embedding_model_id = clip-b16
       d) Queue force rebuild (bypass “already exists”)
       e) Search returns rebuild-required until usable embeds exist
        │
        ▼
AiEngine (bundled B/16 quant) → embeds BLOB → cosine search
```

## Assets

### Download scripts
Point at **patch16** quantized vision/text and a **matching** CLIP tokenizer:

- Vision/text: `Xenova/clip-vit-base-patch16` ONNX quantized files (exact HF path may be `onnx/…` or `onnx/int8/…` — **verify before scripting**; current patch32 script uses root-style names and must not be assumed).
- Tokenizer: CLIP BPE for **patch16** (OpenAI or Xenova patch16 `tokenizer.json`), not leftover patch32-only assumptions if files differ.

Filenames on disk remain:
```
src-tauri/resources/models/vision_model.onnx
src-tauri/resources/models/text_model.onnx
src-tauri/resources/models/tokenizer.json
```

### Pre-download / packaging gate
1. Confirm real quantized URLs and sizes (owner expects ≈ current pack).
2. `ort` trial: load vision + text + one `encode_text` + one `encode_image`.
3. Optional: print vector L2 norms for sanity; search already normalizes, but store-time L2 remains optional consistency polish if norms are wild.
4. Do **not** `git add` ONNX; keep gitignore / local resource workflow; `tauri` bundle still picks same resource names.

## Schema (v9)

```sql
CREATE TABLE IF NOT EXISTS app_meta (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL
);
```

Keys:
| key | value examples |
|-----|----------------|
| `embedding_model_id` | `clip-b16` (active default after B0) |
| `embedding_model_ver` | `1` (optional; write `1` on B0) |

Helpers (Rust): `get_app_meta` / `set_app_meta`, `library_embedding_model_id()` defaulting missing → `clip-b32`.

### Seed meta on library create (required)
- Migration **v9** only creates the `app_meta` **table**.
- **New libraries** must insert `embedding_model_id=clip-b16` (and ver `1`) when the library DB is first created/opened after schema init — fallthrough on `add_library` / first `open_conn` for a brand-new DB is fine as long as it runs before any search.
- Do **not** rely on “missing key → clip-b32 → clear+rebuild” for **fresh** libraries (wastes a no-op clear). Missing key remains the **legacy** signal only.

## Hard-cut algorithm (`ensure_embedding_space_ok`)

Active engine id for B0: always **`clip-b16`**.

```
if !models_loadable():
  return Err(MODEL_MISSING)   // do not touch embeds
id = library_embedding_model_id()  // missing → clip-b32
if id == "clip-b16":
  return Ok(Ready)  // or Ok(Rebuilding) if rebuild job open
// stale:
UPDATE afiles SET embeds = NULL;  -- commit
set_app_meta embedding_model_id=clip-b16, ver=1;  -- commit
start force_rebuild_job();
return Err(INDEX_STALE) or Ok(NeedsRebuild) for UI
```

**Order is intentional:** clear embeds **before** writing `clip-b16`. After crash mid-rebuild, rows may be partially filled with B/16 vectors while id is already `clip-b16` — that is OK if rebuild is force/completable; never leave id=`clip-b16` while **old B/32** blobs remain.

**Never clear first if models cannot load** (review point 1).

## Generate / search gates

### `generate_embedding`
- Call space check (or assume caller already did).
- “Already exists” skip **only if** `library id == clip-b16` **and** embeds non-empty **and** not in force-rebuild mode.
- Force rebuild path: rewrite embed even if non-empty (cancel-safe fill-in for remaining NULLs and optional full re-encode).

### `search_similar_images` / smart tags / similar-from-file
- Before scoring: space check.
- On stale / rebuild incomplete policy:
  - Prefer **fail closed** with clear error / empty + UI banner rather than ranking a thin partial index as if complete (product can allow progressive results only if UI labels “indexing…” — default **fail closed until rebuild finishes or user accepts partial**).
  - **Default for B0:** block ranked AI search while `INDEX_STALE` or rebuild job active with zero complete embeds; after first batch exists, optional progressive is a polish item — ship fail-closed + progress first.

### Multilingual text-only (review point 3)
- `set_text_model(Multilingual)` / `set_image_search_model(1)`: **return Err** while bundled vision is B/16 (or always for B0).
- Settings UI: hide or disable control; i18n note that Chinese free-text upgrade is future (Track B), not this toggle.
- Do not invent `clip-b16-multilingual` space without a real paired text tower.

## Engine notes
- Preprocess: keep 224 + CLIP mean/std unless trial proves otherwise.
- Output tensors: keep existing name fallbacks (`pooler_output` / `image_embeds` / first).
- Text: existing tokenizer `encode(..., true)` + attention_mask probe.
- Performance: B/16 denser patches → slower encode; accept; reuse thumb-first path already in `generate_embedding`.

## Frontend / UX
- Banner or dialog when library needs rebuild: model upgraded, AI index must rebuild; primary action starts rebuild.
- Progress: reuse index/embedding queue patterns if present; cancel leaves incomplete embeds → resume force rebuild, do not un-gate search as fully healthy.
- Settings one-liner: default model CLIP ViT-B/16 (quantized). No model dropdown in B0.
- i18n: en + zh.

## IPC (required — not optional)

**Code truth (pre-B0):** `main.rs` only registers **single-file** `generate_embedding`. There is **no** whole-library embedding rebuild command. Index/scan workers in `t_utils.rs` may generate embeds opportunistically during scan, but that is **not** a user-triggerable “rebuild AI index” path and must not be the only recovery after a hard-cut clear.

| Command | Role | Required? |
|---------|------|-----------|
| `get_image_embedding_status` | `{ activeModelId, libraryModelId, needsRebuild, rebuildRunning, rebuildProgress?, embeddedCount?, totalImageCount? }` for banner / CTA | **Yes** |
| `rebuild_image_embeddings` | Kick embed-only full (or NULL-only) rebuild after ensure has cleared; progress events; cancellable | **Yes** |
| `cancel_rebuild_image_embeddings` | Set cancel flag; leave partial NULLs; status stays needs-work until complete | **Yes** (or fold cancel into rebuild with a separate cancel cmd if that matches other long jobs) |
| Existing `generate_embedding` | Single file; used by rebuild loop and scan | Keep |

### `rebuild_image_embeddings` contract
1. Ensure models loadable; if library id still stale, run ensure path first (clear → write `clip-b16`).
2. `SELECT id FROM afiles WHERE file_type IN (1, 3)` (same image/HEIC set as single-file generate). Prefer **NULL embeds first**, or all ids if force full re-encode.
3. For each id: call `AFile::generate_embedding` (after clear, skip-guard does not short-circuit; if any non-NULL left mid-job, force path must rewrite or only select NULL).
4. Progress: emit events (same style as multilingual download / index progress) — `current`, `total`, optional errors.
5. Cancel: `AtomicBool` checked each iteration; on cancel leave remaining NULL; search stays fail-closed / not fully healthy until resumed and finished.
6. **Do not** require a full filesystem rescan solely to regenerate embeds (embed-only loop is the B0 path). Optional later: reuse index-worker semaphore patterns from `t_utils` for concurrency/budget — not a full re-index.

Wire through `main.rs` + `src-vite/src/common/api.js` per `patterns/add-tauri-command.md`.

## Files to touch (implementation map)

| Layer | Path |
|-------|------|
| Download | `scripts/download_models.ps1`, `scripts/download_models.sh` |
| Resources (local/pack) | `src-tauri/resources/models/*` (not git) |
| Migration | `src-tauri/src/t_migration.rs` (v9) |
| Meta + search + embed | `src-tauri/src/t_sqlite.rs` |
| Engine | `src-tauri/src/t_ai.rs` (reject multilingual; comments/ids) |
| Commands | `src-tauri/src/t_cmds.rs`, `main.rs` as needed |
| Settings / content UI | `Settings.vue`, `Content.vue` (or search entry) |
| Config | `configStore.js` (hide model=1) |
| i18n | `locales/en.json`, `zh.json` |
| Pattern / router | `.mex/patterns/change-image-search-model.md`, `.mex/ROUTER.md` |

## Implementation order
1. Verify HF quantized paths + sizes for patch16; update download scripts.
2. Local `ort` dummy encode_text + encode_image; note norms.
3. Migration v9 `app_meta` + seed `clip-b16` on **new** library create; legacy missing → `clip-b32`.
4. Meta get/set + `ensure_embedding_space_ok` (loadable guard → clear → id; **does not** invent a fake in-process queue alone).
5. **Required IPC:** `get_image_embedding_status` + `rebuild_image_embeddings` (+ cancel) with progress events.
6. Gate search / generate_embedding (skip guard + force/NULL-fill path).
7. Rust + UI block multilingual text-only.
8. Banner CTA → rebuild command + i18n.
9. `cargo check`, `pnpm --dir src-vite build`, manual old-library open + new-library smoke.

## Risks

| Risk | Mitigation |
|------|------------|
| Wrong HF path 404 | Probe real tree before script change |
| Clear embeds then missing ONNX | Loadable check before clear |
| Partial rebuild + “already exists” | Clear first; rebuild selects NULL / force rewrite |
| No whole-library rebuild API | **Required** `rebuild_image_embeddings` — do not rely on rescan-only |
| Multilingual IPC bypass | Rust Err, not only UI hide |
| Score scale surprise | Cosine path already L2; keep histogram logs |
| Encode slower | Progress UI; thumb-first encode |
| Accidental git of ONNX | Do not add models; document pack-only |

## Acceptance
1. Fresh library: AI text search, smart tags, similar-image work on B/16.
2. Legacy library with B/32 embeds: **no** silent plausible ranking; user sees rebuild requirement; after clear, no B/32 blobs remain under id `clip-b16`.
3. Cancel mid-rebuild then resume via `rebuild_image_embeddings`: remaining NULLs fill; skip-guard does not strand them.
4. New library never spuriously `INDEX_STALE` solely due to missing meta (seeded `clip-b16` at create).
5. `set_image_search_model(1)` fails closed; Settings cannot enable it as “upgrade”.
6. `resources/models` content is patch16 quant (local); download scripts point at patch16.
7. Track A floors/histogram still present; empty-result calibration remains log-driven.
8. `cargo check --manifest-path src-tauri/Cargo.toml` and `pnpm --dir src-vite build` pass when code lands.
9. Subjective spot-check (birds/insects/architecture) ≥ previous B/32 on a small album after rebuild.

## Relation to SigLIP plan
- Do **not** implement Track B phases 0–5 in this work.
- Keep `app_meta` + fail-closed so Track B can later set `embedding_model_id` to a SigLIP id without reinventing binding.
- Update pattern doc: default production path becomes **B/16 int8**; “always keep B/32” becomes historical; B0 is the active default-bump track.

## Open items resolved in review
1. Clear only after models loadable — **yes**.
2. Force rebuild / NULL-fill bypasses empty-skip — **yes**.
3. Multilingual: Rust reject + UI hide — **yes**.
4. L2: search cosine already normalizes; optional store-time normalize; verify norms in trial — **yes**.
5. Commit order clear → write id → rebuild — **yes**.
6. Verify real quantized HF paths before scripting — **yes**.
7. No ONNX git commit — **yes**.
8. Whole-library rebuild is **not** optional IPC — **required** `rebuild_image_embeddings` + status (+ cancel); scan pipeline alone is insufficient — **yes** (owner review 2026-07-23).
9. Seed `clip-b16` on new library create so fresh DBs are not legacy-stale — **yes**.
