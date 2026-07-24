# AI Search Stop-Bleed + Embed Matrix Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the broken multilingual text-only model from silently killing image search, speed large-library semantic search with an in-memory embedding matrix, and clean up small ranking/face-distance consistency footguns.

**Architecture:** Keep the production default as **CLIP ViT-B/32 full-stack** (vision + text + tokenizer). Disable / fail-closed the legacy `imageSearch.model=1` text-only tower (it is a sentence-embedding text tower in a different space, still 512-d — not SigLIP2 768-d). Search stays exact cosine top-k, but loads library embeds once into a process-local matrix cache keyed by library DB path + rebuild generation, invalidating on embed write/clear/library switch. Ranking keeps the calibrated thr ladder; Low tier respects the user hard limit while thr_cap remains a soft upper bound.

**Tech Stack:** Tauri 2 / Rust (`rusqlite`, optional `rayon`), Vue 3 + Pinia settings UI, existing ONNX `AiEngine`, SQLite per-library DBs.

**Out of scope (do not implement in this plan):**
- Full Track B multi-model sideload / SigLIP2 product UI / `app_meta.embedding_model_id` rebuild pipeline (already designed in `patterns/change-image-search-model.md`; this plan only stop-bleeds legacy `model=1`).
- ANN / HNSW for image search.
- Perceptual-hash “similar duplicates” (product backlog; exact blake3 stays).
- Reworking `get_video_thumbnail_sync` runtime ownership (document-only note if touched; no behavior change required).

**Evidence locked before coding (do not re-derive wrongly):**
- Bundled CLIP text/vision outputs are **512-d**.
- Sideloaded multilingual `text_model.onnx` outputs **`sentence_embeddings` [batch, 512]** with `attention_mask` — dim-compatible, **space-incompatible** with CLIP image embeds.
- Empty multilingual text search is **cross-space ranking**, not `i != query.len()` dim reject.
- `open_conn` already has an idle pool (max 8); do not “fix” connection pooling.
- Low tier intentionally ignored UI limit for slider visibility; product fix = **user limit is hard cap**, thr_cap is soft max.

---

## File map

| File | Responsibility in this plan |
|------|-----------------------------|
| `src-tauri/src/t_ai.rs` | Reject / no-op multilingual text-only activation; expose clear error code |
| `src-tauri/src/t_cmds.rs` | Surface model-switch errors; optional status flag `multilingualEnabled: false` |
| `src-tauri/src/t_sqlite.rs` | Embed matrix cache; search uses cache; invalidate on embed write/clear; Low×limit; unit tests |
| `src-tauri/src/t_cluster.rs` | `cosine_distance` fail-closed on dim mismatch |
| `src-vite/src/views/Settings.vue` | Hide or disable multilingual option; honest i18n; force model=0 |
| `src-vite/src/stores/configStore.js` | Comment + coerce legacy model=1 → 0 on load |
| `src-vite/src/locales/en.json`, `zh.json` | Disabled/deprecated multilingual copy |
| `src-vite/src/common/api.js` | Map host error codes if needed |
| `.mex/patterns/change-ai-search-filters.md` | thr_cap + limit hard-cap contract |
| `.mex/patterns/change-library-perf.md` | Embed matrix cache rules |
| `.mex/patterns/change-image-search-model.md` | Legacy model=1 disabled status |
| `.mex/ROUTER.md` + `.mex/context/decisions.md` | GROW after ship |
| `docs/superpowers/plans/2026-07-24-ai-search-stopbleed-and-embed-cache.md` | This plan |

---

## Product decisions (locked for implementers)

1. **Legacy multilingual (`model=1`) is disabled for product use.** Prefer UI hide/disable + host reject. Do not ship a half-working text-only tower.
2. **User `imageSearch.limit` is a hard upper bound for all tiers**, including Low.  
   `top_k = min(requested_or_thr_cap, thr_cap, 200)` where `requested = params.limit if > 0 else thr_cap`.  
   Practical effect with default limit=50: Low returns ≤50 (same as Medium count ceiling). Slider still differentiates via **absolute_floor** (stricter tiers cut more by score). If product later wants Low to show more by default, raise **default limit** in settings (e.g. 100/200) — do not special-case Low past the user hard cap.
3. **Embed matrix is exact search only** (no ANN). Cache miss falls back to current SQL BLOB scan path so cold start never hard-fails.
4. **Matrix memory:** f32 N×D, D=512 for CLIP. Optional soft skip if N×D×4 would exceed a conservative budget (default **512 MiB**). Below budget: always cache after first successful full load.

---

### Task 1: Host rejects multilingual text-only activation

**Files:**
- Modify: `src-tauri/src/t_ai.rs` (`set_text_model`, optionally `model_status`)
- Modify: `src-tauri/src/t_cmds.rs` (`set_image_search_model`)
- Test: unit tests in `src-tauri/src/t_ai.rs` or thin pure helper

- [ ] **Step 1: Add a stable error constant / helper**

In `t_ai.rs`, near `ImageSearchTextModel`:

```rust
/// Legacy text-only multilingual tower is space-incompatible with CLIP vision embeds.
/// Kept downloadable assets may remain on disk; activation is rejected.
pub const ERR_MULTILINGUAL_TEXT_ONLY_DISABLED: &str =
    "MULTILINGUAL_TEXT_ONLY_DISABLED: optional multilingual text tower is disabled; image search requires matching vision+text embeds (CLIP full-stack). Stay on default model.";

pub fn assert_text_model_activatable(model: ImageSearchTextModel) -> Result<(), String> {
    match model {
        ImageSearchTextModel::Default => Ok(()),
        ImageSearchTextModel::Multilingual => Err(ERR_MULTILINGUAL_TEXT_ONLY_DISABLED.to_string()),
    }
}
```

- [ ] **Step 2: Fail closed in `set_text_model`**

At the top of `AiEngine::set_text_model`, after the early “already active” check for Default only:

```rust
// Refuse Multilingual even if already somehow loaded — force Default path via caller.
assert_text_model_activatable(model)?;
```

Also change the early-return:

```rust
if self.text_model.is_some() && self.text_model_kind == model {
    // If Multilingual was loaded by an older build, still reject re-entry; caller must set Default.
    assert_text_model_activatable(model)?;
    return Ok(());
}
```

If `text_model_kind` is Multilingual from a stale process state, `load_models` / startup must force Default (Task 2 / Settings coerce covers settings; host `load_models` already loads Default when text is None).

- [ ] **Step 3: Unit test pure helper**

```rust
#[cfg(test)]
mod multilingual_gate_tests {
    use super::*;

    #[test]
    fn default_text_model_is_allowed() {
        assert!(assert_text_model_activatable(ImageSearchTextModel::Default).is_ok());
    }

    #[test]
    fn multilingual_text_only_is_rejected() {
        let err = assert_text_model_activatable(ImageSearchTextModel::Multilingual).unwrap_err();
        assert!(err.starts_with("MULTILINGUAL_TEXT_ONLY_DISABLED"));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml multilingual_gate_tests -- --nocapture
```

Expected: both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/t_ai.rs
git commit -m "fix(ai): reject legacy multilingual text-only model activation"
```

---

### Task 2: Settings UI disables multilingual + coerce legacy preference

**Files:**
- Modify: `src-vite/src/views/Settings.vue` (model select, hints, download flow)
- Modify: `src-vite/src/stores/configStore.js` (load coerce)
- Modify: `src-vite/src/locales/en.json`, `src-vite/src/locales/zh.json`
- Modify: `src-vite/src/common/api.js` only if toast needs cleaner message strip

- [ ] **Step 1: i18n strings**

`en.json` under `settings.image_search` (keep old keys if referenced; update meaning):

```json
"multilingual_model_hint": "Temporarily disabled: text-only multilingual tower does not match image embeds. Default CLIP remains available.",
"multilingual_model_disabled_toast": "Multilingual search model is temporarily disabled. Using default English CLIP model.",
"default_model_hint": "English CLIP (vision + text). Best match for current library embeds."
```

`zh.json`:

```json
"multilingual_model_hint": "暂时不可用：仅替换文本塔无法与图片向量对齐。请继续使用默认 CLIP。",
"multilingual_model_disabled_toast": "多语言模型暂时不可用，已切回默认 CLIP 英文模型。",
"default_model_hint": "英文 CLIP（图文同栈），与当前图库向量一致。"
```

- [ ] **Step 2: Coerce settings on load**

In `configStore.js` where `imageSearch.model` is hydrated / set:

```js
// Legacy model=1 (text-only multilingual) is disabled: space-incompatible with CLIP vision embeds.
const n = Number(imageSearchModel);
this.settings.imageSearch.model = n === 1 ? 0 : (Number.isFinite(n) ? Math.max(0, Math.trunc(n)) : 0);
```

- [ ] **Step 3: Settings UI**

In `Settings.vue`:
1. `imageSearchModelOptions` → only Default (or keep Multilingual option `disabled` with hint).
2. Prefer **single option** to avoid dead download UX:

```js
const imageSearchModelOptions = computed(() => {
  const options = localeMsg.value.settings.image_search.search_model_options || ['Default'];
  // Only default is activatable; hide multilingual until full-stack Track B ships.
  return [{ label: options[0] || 'Default', value: 0 }];
});
```

3. `onImageSearchModelChange`: if `nextModel === 1`, toast disabled message, force select back to 0, call `setImageSearchModel(0)`.
4. Do **not** auto-start multilingual download from settings while disabled.
5. On `syncImageSearchModelStatus`, always `setImageSearchModel(0)` and clear model=1.

- [ ] **Step 4: Manual UI check**

Run: `cargo tauri dev` (or existing dev flow).  
Open Settings → Image search model shows only Default; old saved model=1 becomes 0 after reload; free-text EN search still works.

- [ ] **Step 5: Commit**

```bash
git add src-vite/src/views/Settings.vue src-vite/src/stores/configStore.js src-vite/src/locales/en.json src-vite/src/locales/zh.json
git commit -m "fix(ui): disable legacy multilingual text-only image search model"
```

---

### Task 3: Ranking — all tiers honor user limit hard cap

**Files:**
- Modify: `src-tauri/src/t_sqlite.rs` (`search_similar_images` top_k block ~3247–3269)
- Modify: `.mex/patterns/change-ai-search-filters.md`
- Test: pure unit test for top_k policy helper (extract small fn)

- [ ] **Step 1: Extract pure top_k helper**

Near `search_similar_images` (or above it as `fn image_search_top_k`):

```rust
/// User limit is a hard cap for every similarity tier. thr_cap is a soft tier max.
/// Soft global max remains 200.
fn image_search_top_k(settings_thr: f32, params_limit: i64) -> (usize, usize) {
    let thr_cap: usize = if settings_thr >= 0.27 {
        30
    } else if settings_thr >= 0.23 {
        40
    } else if settings_thr >= 0.19 {
        50
    } else {
        200
    };
    let requested = if params_limit > 0 {
        params_limit as usize
    } else {
        thr_cap
    };
    let top_k = requested.min(thr_cap).min(200).max(1);
    (thr_cap, top_k)
}
```

Replace the old Low special-case with:

```rust
let (thr_cap, top_k) = image_search_top_k(settings_thr, params.limit);
```

- [ ] **Step 2: Unit tests**

```rust
#[cfg(test)]
mod image_search_top_k_tests {
    use super::image_search_top_k;

    #[test]
    fn low_respects_user_limit_20() {
        let (cap, k) = image_search_top_k(0.16, 20);
        assert_eq!(cap, 200);
        assert_eq!(k, 20);
    }

    #[test]
    fn low_default_limit_50_caps_at_50() {
        let (_cap, k) = image_search_top_k(0.16, 50);
        assert_eq!(k, 50);
    }

    #[test]
    fn very_high_never_exceeds_30() {
        let (cap, k) = image_search_top_k(0.28, 1000);
        assert_eq!(cap, 30);
        assert_eq!(k, 30);
    }

    #[test]
    fn medium_honors_smaller_limit() {
        let (_cap, k) = image_search_top_k(0.20, 10);
        assert_eq!(k, 10);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml image_search_top_k_tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Pattern note**

In `change-ai-search-filters.md` ranking section, replace “Low uses thr_cap even if UI limit is 50” with:

> User `limit` is a **hard cap** for all tiers. `top_k = min(limit_or_thr_cap, thr_cap, 200)`. Slider differentiation for Low vs Medium with the same limit is primarily via **absolute_floor**, not count.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/t_sqlite.rs .mex/patterns/change-ai-search-filters.md
git commit -m "fix(search): honor user limit as hard cap on all similarity tiers"
```

---

### Task 4: Face clustering fail-closed on embedding dim mismatch

**Files:**
- Modify: `src-tauri/src/t_cluster.rs` (`cosine_distance`)
- Test: existing `mod tests` in same file

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn cosine_distance_rejects_length_mismatch() {
    let a = vec![1.0f32, 0.0, 0.0];
    let b = vec![1.0f32, 0.0];
    assert!(cosine_distance_checked(&a, &b).is_none());
}
```

- [ ] **Step 2: Implement checked helper + thin wrapper**

```rust
/// Distance = 1 - cosine for L2-normalized vectors. Returns None on dim mismatch.
fn cosine_distance_checked(emb1: &[f32], emb2: &[f32]) -> Option<f32> {
    if emb1.len() != emb2.len() || emb1.is_empty() {
        return None;
    }
    let mut dot = 0.0f32;
    for i in 0..emb1.len() {
        dot += emb1[i] * emb2[i];
    }
    let similarity = dot.clamp(-1.0, 1.0);
    Some(1.0 - similarity)
}

fn cosine_distance(emb1: &[f32], emb2: &[f32]) -> f32 {
    // Fail closed: mismatched dims must not silently truncate (release used to min()).
    cosine_distance_checked(emb1, emb2).unwrap_or(2.0) // > any real distance in [0, 2]
}
```

Update call sites that can skip pairs to prefer `cosine_distance_checked` and **continue** on `None` when building edges (if a loop already filters). If only `cosine_distance` is used, `unwrap_or(2.0)` keeps pairs from clustering together (distance larger than any valid cosine distance on unit vectors).

- [ ] **Step 3: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml cosine_distance -- --nocapture
```

Expected: PASS (including existing identical-is-zero test).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/t_cluster.rs
git commit -m "fix(face): fail closed on embedding dimension mismatch in clustering"
```

---

### Task 5: In-memory embedding matrix — data structures + load

**Files:**
- Modify: `src-tauri/src/t_sqlite.rs` (new module section near search / pool)
- Optional: add `rayon` to `src-tauri/Cargo.toml` only if Task 6 enables parallel score (can defer rayon to Task 6)

- [ ] **Step 1: Add cache types**

Near the top of the search-related section in `t_sqlite.rs` (after imports allow `std::sync::{Mutex, Arc}` if not present):

```rust
/// Process-local image-search embedding matrix for one library DB.
/// Layout: ids[i] corresponds to row i of data[i*dim .. (i+1)*dim] (row-major f32).
struct EmbedMatrix {
    db_key: String,
    /// Bumped when any embed row is written/cleared for this library.
    generation: u64,
    dim: usize,
    ids: Vec<i64>,
    /// Row-major f32, len = ids.len() * dim
    data: Vec<f32>,
    /// Precomputed L2 norms per row (0 if invalid)
    norms: Vec<f32>,
}

struct EmbedMatrixCache {
    /// Only one active library matrix (current DB). Simple + enough for MVP.
    current: Option<EmbedMatrix>,
    /// Monotonic generation counter per db_key (process lifetime).
    generations: std::collections::HashMap<String, u64>,
}

impl EmbedMatrixCache {
    fn new() -> Self {
        Self {
            current: None,
            generations: std::collections::HashMap::new(),
        }
    }
}

static EMBED_MATRIX_CACHE: Mutex<EmbedMatrixCache> = Mutex::new(EmbedMatrixCache {
    current: None,
    generations: std::collections::HashMap::new(),
});

/// Soft cap: skip caching if matrix would exceed this many bytes (data only).
const EMBED_MATRIX_MAX_BYTES: usize = 512 * 1024 * 1024;
```

- [ ] **Step 2: Invalidation API**

```rust
pub(crate) fn bump_embed_matrix_generation(db_path_key: &str) {
    if let Ok(mut cache) = EMBED_MATRIX_CACHE.lock() {
        let g = cache.generations.entry(db_path_key.to_string()).or_insert(0);
        *g = g.saturating_add(1);
        if cache.current.as_ref().map(|m| m.db_key.as_str()) == Some(db_path_key) {
            cache.current = None;
        }
    }
}

pub(crate) fn invalidate_embed_matrix_for_current_db() {
    if let Ok(path) = t_storage::get_current_db_path() {
        let key = normalize_db_path_key(&path);
        bump_embed_matrix_generation(&key);
    }
}
```

Call `invalidate_embed_matrix_for_current_db()` after:
1. Successful `UPDATE afiles SET embeds = ?` in `generate_embedding`
2. Any path that sets `embeds = NULL` (scan modified-file clear ~2132)
3. Library DB switch / `clear_conn_pool` (same places that already clear pool after storage migrate)

- [ ] **Step 3: Loader**

```rust
fn load_embed_matrix(conn: &Connection, db_key: &str, generation: u64) -> Result<Option<EmbedMatrix>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.embeds FROM afiles a
             LEFT JOIN afolders b ON a.folder_id = b.id
             WHERE a.embeds IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;

    let mut ids: Vec<i64> = Vec::new();
    let mut data: Vec<f32> = Vec::new();
    let mut norms: Vec<f32> = Vec::new();
    let mut dim: Option<usize> = None;

    let rows = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((id, blob))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (id, blob) = row.map_err(|e| e.to_string())?;
        if blob.len() % 4 != 0 || blob.is_empty() {
            continue;
        }
        let row_dim = blob.len() / 4;
        if let Some(d) = dim {
            if row_dim != d {
                // Skip mismatched rows (fail closed per row).
                continue;
            }
        } else {
            dim = Some(row_dim);
            // Budget check once we know dim: estimate remaining unknown; use soft check after push.
        }
        let d = dim.unwrap();
        if ids.len().saturating_add(1).saturating_mul(d).saturating_mul(4) > EMBED_MATRIX_MAX_BYTES {
            // Over budget: do not cache.
            return Ok(None);
        }
        let mut norm_sq = 0.0f32;
        for chunk in blob.chunks_exact(4) {
            let v = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            data.push(v);
            norm_sq += v * v;
        }
        ids.push(id);
        norms.push(if norm_sq > 0.0 { norm_sq.sqrt() } else { 0.0 });
    }

    let d = match dim {
        Some(d) if !ids.is_empty() => d,
        _ => {
            return Ok(Some(EmbedMatrix {
                db_key: db_key.to_string(),
                generation,
                dim: 0,
                ids,
                data,
                norms,
            }));
        }
    };

    Ok(Some(EmbedMatrix {
        db_key: db_key.to_string(),
        generation,
        dim: d,
        ids,
        data,
        norms,
    }))
}
```

- [ ] **Step 4: Get-or-load**

```rust
fn get_or_load_embed_matrix(conn: &Connection) -> Result<Option<std::sync::Arc<EmbedMatrix>>, String> {
    let path = t_storage::get_current_db_path().map_err(|e| e.to_string())?;
    let db_key = normalize_db_path_key(&path);

    let generation = {
        let mut cache = EMBED_MATRIX_CACHE.lock().map_err(|e| e.to_string())?;
        *cache.generations.entry(db_key.clone()).or_insert(0)
    };

    {
        let cache = EMBED_MATRIX_CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(ref m) = cache.current {
            if m.db_key == db_key && m.generation == generation {
                // Return by cloning Arc — store Arc in cache for cheap hits.
            }
        }
    }

    // Prefer storing Arc in cache:
    // Adjust EmbedMatrixCache.current type to Option<Arc<EmbedMatrix>> when implementing.
    let loaded = load_embed_matrix(conn, &db_key, generation)?;
    let Some(matrix) = loaded else {
        return Ok(None);
    };
    let arc = std::sync::Arc::new(matrix);
    if let Ok(mut cache) = EMBED_MATRIX_CACHE.lock() {
        // Race: if generation advanced while loading, drop stale matrix.
        let g_now = *cache.generations.entry(db_key.clone()).or_insert(0);
        if g_now == generation {
            cache.current = Some(arc.clone());
        }
    }
    Ok(Some(arc))
}
```

Implementers: store `current: Option<Arc<EmbedMatrix>>` from the start to avoid rework.

- [ ] **Step 5: Compile check**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: success (search still on old path until Task 6 wires it).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/t_sqlite.rs
git commit -m "feat(search): add process-local embedding matrix cache skeleton"
```

---

### Task 6: Wire `search_similar_images` to matrix + optional rayon

**Files:**
- Modify: `src-tauri/src/t_sqlite.rs` (`search_similar_images`)
- Modify: `src-tauri/Cargo.toml` only if enabling `rayon`
- Modify: `.mex/patterns/change-library-perf.md`

- [ ] **Step 1: Score from matrix (serial first)**

```rust
fn score_matrix(
    matrix: &EmbedMatrix,
    query: &[f32],
    query_norm: f32,
) -> Vec<(i64, f32)> {
    if matrix.dim == 0 || query.len() != matrix.dim || query_norm <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(matrix.ids.len());
    for (i, &id) in matrix.ids.iter().enumerate() {
        let row_norm = matrix.norms[i];
        if row_norm <= 0.0 {
            continue;
        }
        let start = i * matrix.dim;
        let row = &matrix.data[start..start + matrix.dim];
        let mut dot = 0.0f32;
        for j in 0..matrix.dim {
            dot += query[j] * row[j];
        }
        let score = dot / (query_norm * row_norm);
        if score >= 0.16 {
            out.push((id, score));
        }
    }
    out
}
```

- [ ] **Step 2: Integrate into `search_similar_images`**

After computing `query_norm`, before SQL scan:

```rust
let conn = open_conn()?;
let mut used_matrix = false;
let mut scores: Vec<(i64, f32)> = Vec::new();
let mut candidates: u32 = 0;
let mut max_score = f32::NEG_INFINITY;
let mut band_gt = [0u32; 5];

if let Ok(Some(matrix)) = get_or_load_embed_matrix(&conn) {
    if matrix.dim == embedding.len() {
        used_matrix = true;
        // Optional: apply SQL exclusion / file_type by pre-filtering ids via a cheap id set.
        // MVP: if params.search_file_type != 0 OR exclusions may apply, fall back to SQL path
        // to avoid incorrect inclusion. Exact rule:
        let has_type_filter = params.search_file_type != 0;
        // Folder exclusion always exists in SQL path — matrix load currently has NO exclusion.
        // REQUIRED: either
        //   (A) load_embed_matrix uses the same WHERE as search (exclusion + optional type), or
        //   (B) fall back to SQL whenever exclusion tables might filter.
        // Choose (A): push the same WHERE into load_embed_matrix and into a
        // `load_embed_matrix_filtered(conn, search_file_type)` used by search.
    }
}
```

**Required correctness rule (do not ship without it):**  
Matrix rows must respect:
1. `search_exclusion_condition("b")`
2. `build_file_type_condition(params.search_file_type)` when present  

Implement by moving the SELECT used today into a shared SQL builder, used by both SQL fallback and matrix loader **for this query**. For cache reuse across queries with different file-type filters:

- **Preferred MVP:** cache stores **all non-null embeds without type filter**, but **with exclusion applied** (exclusions change rarely). Apply file-type filter by joining ids to a `HashSet` of allowed ids from a cheap `SELECT id FROM afiles WHERE <type>` when mask ≠ 0, **or** fall back to SQL path when `search_file_type != 0` (simpler, still wins on common “all types” searches).

Lock this MVP choice in code comments:

```rust
// MVP cache: all embeds + search exclusions. If search_file_type != 0, use SQL blob path.
```

- [ ] **Step 3: Fallback path**

Keep existing `query_map` + `cosine_similarity_blob` loop when:
- matrix load returns None (budget),
- dim mismatch,
- `search_file_type != 0` (if MVP chooses fallback),
- cache poisoned / lock fail.

- [ ] **Step 4: Diagnostics**

Extend the println to include `matrix=1/0 candidates=...`.

- [ ] **Step 5 (optional same commit or follow-up): rayon**

If `rayon` already a dependency or added carefully:

```toml
rayon = "1.10"
```

Parallelize only the scoring loop with `into_par_iter` + fold/reduce of local score vecs. Keep serial path as default if rayon adds friction; **serial matrix without SQLite blob re-read is already the main win**.

- [ ] **Step 6: Pattern update**

`change-library-perf.md` add:

> Image search: process-local `EmbedMatrix` (row-major f32 + norms), keyed by normalized DB path + generation. Invalidate on embed write/clear/library switch. First query may load ~N×dim×4 bytes; subsequent queries score in RAM. Exact cosine only (no ANN).

- [ ] **Step 7: Verify**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml image_search_top_k_tests multilingual_gate_tests cosine_distance -- --nocapture
```

Manual: library with embeddings → first AI search may be slower (load), second faster; change an image embed / re-scan → generation bumps → matrix reloads.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/t_sqlite.rs src-tauri/Cargo.toml .mex/patterns/change-library-perf.md
git commit -m "perf(search): score AI search from in-memory embedding matrix"
```

---

### Task 7: Invalidation call sites audit

**Files:**
- Modify: `src-tauri/src/t_sqlite.rs` (embed write/clear)
- Modify: `src-tauri/src/t_cmds.rs` if storage migrate already clears pool — also invalidate matrix

- [ ] **Step 1: Grep all embed mutators**

```bash
rg -n "embeds\\s*=" src-tauri/src --glob '*.rs'
```

Every write/NULL of `afiles.embeds` must call `invalidate_embed_matrix_for_current_db()` (or library-specific key).

- [ ] **Step 2: Wire each site**

Minimum:
1. After successful embed UPDATE in `generate_embedding`
2. Scan path that NULLs embeds on file change
3. Next to `clear_conn_pool()` after DB storage move/reset

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/t_sqlite.rs src-tauri/src/t_cmds.rs
git commit -m "fix(search): invalidate embed matrix on write, clear, and library switch"
```

---

### Task 8: Docs / MEX GROW + regression checklist

**Files:**
- Modify: `.mex/ROUTER.md` current state bullet
- Modify: `.mex/context/decisions.md` (decision: disable legacy multilingual text-only)
- Modify: `.mex/patterns/change-image-search-model.md` status row for legacy model=1
- Optional: `docs/guide/picaipic-progress.md` one line if that file is actively maintained this week

- [ ] **Step 1: Decision log entry**

```markdown
### 2026-07-24 — Disable legacy multilingual text-only image search
- **Decision:** `imageSearch.model=1` activation is rejected; UI forces Default CLIP full-stack.
- **Why:** Sideloaded multilingual text tower is a sentence-embedding model (512-d) not aligned with CLIP vision embeds; dim guard does not fire; text search empties / ranks garbage. Track B full-stack remains the only path for multilingual product UI.
- **Not done here:** `app_meta.embedding_model_id`, rebuild UX, SigLIP2 sideload.
```

- [ ] **Step 2: Final verification**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml multilingual_gate_tests image_search_top_k_tests cosine_distance -- --nocapture
pnpm --dir src-vite build
```

Manual checklist:
1. Settings: only default model; cannot enable multilingual.
2. EN free-text search + smart tags still work with slider.
3. Low + limit=20 → ≤20 results.
4. VH → ≤30 results.
5. Second AI search on same library is faster than cold first search (matrix hit); host log shows `matrix=1`.
6. Re-embed one file or NULL embeds → next search reloads matrix (generation bump).
7. Face cluster still works on a small library (smoke).

- [ ] **Step 3: Commit docs**

```bash
git add .mex/ROUTER.md .mex/context/decisions.md .mex/patterns/change-image-search-model.md .mex/patterns/change-ai-search-filters.md .mex/patterns/change-library-perf.md docs/superpowers/plans/2026-07-24-ai-search-stopbleed-and-embed-cache.md
git commit -m "docs(mex): record AI search stop-bleed and embed matrix decisions"
```

---

## Suggested PR / ship order

| Slice | Tasks | Ship alone? |
|-------|-------|-------------|
| A. Stop-bleed multilingual | 1–2 | **Yes — highest user safety** |
| B. Limit + face dim | 3–4 | Yes, tiny |
| C. Embed matrix | 5–7 | Yes, largest; keep SQL fallback |
| D. GROW | 8 | With C or final |

Prefer merge A before C so users are not left on broken model=1 while perf work lands.

---

## Self-review (plan)

| Spec item | Task |
|-----------|------|
| #1 multilingual empty search | 1–2 |
| #2 full-table BLOB I/O | 5–7 |
| #3 face silent min truncate | 4 |
| #4 video runtime | Out of scope (documented) |
| #5 pHash dedup | Out of scope |
| #6 Low ignores limit | 3 |
| open_conn pool “fix” | Explicitly not done |
| dim myth (768) corrected | Architecture + Task 1 notes |

No placeholder steps remain for in-scope work. Rayon is optional inside Task 6; serial matrix is sufficient acceptance.
