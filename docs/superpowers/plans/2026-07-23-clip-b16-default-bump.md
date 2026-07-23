# CLIP B/16 Default Bump (Track B0) Implementation Plan

> **Status (2026-07-23): ABANDONED as product default.** Owner trial found B/16 quant ≈ B/32 subjectively. **Do not execute** this plan to change the shipping default. Branch `feat/clip-b16-default-bump` is experimental only and must not be merged for B/16 default. Prefer Track B (SigLIP) if quality work resumes. Plan kept for historical reference.

> **For agentic workers:** Do **not** implement this plan unless the owner explicitly revives B0. If revived, use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal (historical):** Replace the bundled default image-search model (Xenova CLIP ViT-B/32 quant) with CLIP ViT-B/16 int8/quant, hard-cut per-library embeds via `app_meta`, and add a real whole-library embed rebuild path — without SigLIP or multi-model UI.

**Architecture:** Keep the same ONNX resource filenames and CLIP preprocess (224 + CLIP mean/std, 512-d). Logical id `clip-b16` is the active engine space. Each library DB stores `app_meta.embedding_model_id`. On mismatch, clear all `afiles.embeds`, write `clip-b16`, and require `rebuild_image_embeddings` (new IPC) before ranked AI search. Multilingual text-only (`model=1`) is rejected in Rust and hidden in Settings. Track A thresholds stay as-is.

**Tech Stack:** Tauri 2 / Rust (`ort`, `rusqlite`, `tokenizers`), Vue 3 + Pinia, download scripts (PowerShell/bash).

**Design spec:** `docs/superpowers/specs/2026-07-23-clip-b16-default-bump-design.md`  
**Pattern:** `.mex/patterns/change-image-search-model.md`

---

## File map

| File | Responsibility |
|------|----------------|
| `scripts/download_models.ps1` | HF URLs → B/16 quantized vision/text + matching tokenizer |
| `scripts/download_models.sh` | Same for Unix |
| `src-tauri/resources/models/*` | Local/packaged weights (not git) |
| `src-tauri/src/t_migration.rs` | Schema v9 `app_meta` + repair ensure |
| `src-tauri/src/t_sqlite.rs` | Meta helpers, ensure/clear, search/generate gates, create_db seed |
| `src-tauri/src/t_ai.rs` | Active model id constant, reject Multilingual, optional loadable helper |
| `src-tauri/src/t_cmds.rs` | Status / rebuild / cancel commands |
| `src-tauri/src/main.rs` | Register commands + manage rebuild state |
| `src-vite/src/common/api.js` | IPC wrappers; surface INDEX_STALE to UI |
| `src-vite/src/views/Settings.vue` | Hide multilingual option; model hint = B/16 |
| `src-vite/src/components/Content.vue` | Rebuild banner + CTA on stale |
| `src-vite/src/stores/configStore.js` | Force `imageSearch.model` default 0 if needed |
| `src-vite/src/locales/en.json`, `zh.json` | Banner + settings copy |
| `.mex/ROUTER.md`, pattern | GROW after ship |

---

## Constants (use everywhere)

```rust
// Logical ids — not file names
pub const EMBEDDING_MODEL_CLIP_B16: &str = "clip-b16";
pub const EMBEDDING_MODEL_CLIP_B32_LEGACY: &str = "clip-b32";
pub const APP_META_EMBEDDING_MODEL_ID: &str = "embedding_model_id";
pub const APP_META_EMBEDDING_MODEL_VER: &str = "embedding_model_ver";
pub const EMBEDDING_MODEL_VER_B0: &str = "1";

// Error codes (string prefixes the frontend can match)
// "INDEX_STALE: ..."
// "MODEL_MISSING: ..."
// "REBUILD_RUNNING: ..."
// "MULTILINGUAL_DISABLED: ..."
```

Event name for rebuild progress:

```text
image_embedding_rebuild_progress
```

Payload (camelCase serde):

```rust
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEmbeddingRebuildProgress {
    pub current: u64,
    pub total: u64,
    pub running: bool,
    pub cancelled: bool,
    pub last_error: Option<String>,
}
```

Status payload:

```rust
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEmbeddingStatus {
    pub active_model_id: String,      // always "clip-b16" in B0
    pub library_model_id: String,     // from app_meta or legacy default
    pub needs_rebuild: bool,
    pub rebuild_running: bool,
    pub rebuild_progress: Option<ImageEmbeddingRebuildProgress>,
    pub embedded_count: i64,
    pub total_image_count: i64,
    pub models_present: bool,
}
```

---

### Task 1: Point download scripts at B/16 quant + verify URLs

**Files:**
- Modify: `scripts/download_models.ps1`
- Modify: `scripts/download_models.sh`

- [ ] **Step 1: Probe Hugging Face paths (manual)**

Open / curl HEAD these candidates (pick first that returns 200 for all three files):

```
# Preferred (mirrors current patch32 layout)
https://huggingface.co/Xenova/clip-vit-base-patch16/resolve/main/onnx/vision_model_quantized.onnx
https://huggingface.co/Xenova/clip-vit-base-patch16/resolve/main/onnx/text_model_quantized.onnx
https://huggingface.co/Xenova/clip-vit-base-patch16/resolve/main/tokenizer.json
# Fallback if tokenizer missing on Xenova:
https://huggingface.co/openai/clip-vit-base-patch16/resolve/main/tokenizer.json
```

If `onnx/` 404, try `onnx/int8/` or non-quantized names documented on the repo page. Record final URLs in the commit message.

- [ ] **Step 2: Update PowerShell script URLs**

Replace the three CLIP entries in `scripts/download_models.ps1` (keep face models unchanged):

```powershell
$Models = @(
    @{ Url = "<TOKENIZER_URL>"; File = "tokenizer.json" },
    @{ Url = "<TEXT_MODEL_URL>"; File = "text_model.onnx" },
    @{ Url = "<VISION_MODEL_URL>"; File = "vision_model.onnx" },
    @{ Url = "https://huggingface.co/deepghs/insightface/resolve/main/buffalo_s/det_500m.onnx?download=true"; File = "det_500m.onnx" },
    @{ Url = "https://huggingface.co/deepghs/insightface/resolve/main/buffalo_s/w600k_mbf.onnx?download=true"; File = "w600k_mbf.onnx" }
)
```

Also change skip behavior for model swap: if upgrading from B/32, **delete existing** `vision_model.onnx` / `text_model.onnx` / `tokenizer.json` before download, or force re-download when a marker file is absent. Minimal approach — at top of script after `$TargetDir` is set:

```powershell
# B0: force replace CLIP pack when switching patch32 → patch16 (one-time local upgrade)
$ClipMarker = Join-Path $TargetDir ".clip-b16"
if (-not (Test-Path $ClipMarker)) {
    foreach ($f in @("vision_model.onnx", "text_model.onnx", "tokenizer.json")) {
        $p = Join-Path $TargetDir $f
        if (Test-Path $p) { Remove-Item $p -Force }
    }
}
# after successful CLIP downloads:
# New-Item -ItemType File -Path $ClipMarker -Force | Out-Null
```

Write the marker only after all three CLIP files downloaded successfully.

- [ ] **Step 3: Mirror the same logic in `download_models.sh`**

Same three URLs + marker `.clip-b16` + force remove old CLIP files when marker missing.

- [ ] **Step 4: Download and size-check**

```bash
# Git Bash / PowerShell from repo root
powershell -ExecutionPolicy Bypass -File ./scripts/download_models.ps1
# or
bash ./scripts/download_models.sh
ls -la src-tauri/resources/models/
```

Expected: three CLIP files present; vision+text total size roughly near previous B/32 pack (order-of-magnitude, not exact). **Do not git add** these files.

- [ ] **Step 5: Commit scripts only**

```bash
git add scripts/download_models.ps1 scripts/download_models.sh
git commit -m "chore: download CLIP ViT-B/16 quantized models for B0 default"
```

---

### Task 2: Smoke-load B/16 with existing engine (manual)

**Files:** none (dev verification)

- [ ] **Step 1: Ensure models on disk from Task 1**

- [ ] **Step 2: Run app or a minimal check**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Then `cargo tauri dev` (or existing dev workflow). Trigger any AI path that loads models (image search once). Confirm no ORT load errors in console.

- [ ] **Step 3: Optional norm probe**

If convenient, temporary `println!` in `run_vision_model` / `encode_text` after extract:

```rust
let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
println!("embed_norm len={} l2={:.4}", embedding.len(), norm);
```

Expect `len == 512`. Remove temporary prints before commit (or keep behind `#[cfg(debug_assertions)]` if useful). Cosine path already L2-normalizes at search time — norms need not be 1.0.

- [ ] **Step 4: No commit required** unless you leave a permanent debug flag (prefer not).

---

### Task 3: Schema v9 `app_meta` + seed helpers

**Files:**
- Modify: `src-tauri/src/t_migration.rs`
- Modify: `src-tauri/src/t_sqlite.rs` (meta helpers + seed after migrate)
- Test: unit tests in `t_sqlite.rs` or `t_migration.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Add migration entry v9**

In `get_migrations()` after version 8:

```rust
Migration {
    version: 9,
    description: "Add app_meta key-value table for embedding model binding",
    sql: "
        CREATE TABLE IF NOT EXISTS app_meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );
    ",
},
```

- [ ] **Step 2: Apply v9 in `check_and_migrate`**

In the migration loop, after `version == 8` branch, add:

```rust
} else if migration.version == 9 {
    conn.execute_batch(migration.sql)
        .map_err(|e| format!("Migration {} failed: {}", migration.version, e))?;
```

Also add a repair call at the end of `check_and_migrate` (like collections):

```rust
ensure_app_meta_table(conn)?;
```

```rust
fn ensure_app_meta_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Implement app_meta helpers on a small module section in `t_sqlite.rs`**

```rust
pub fn get_app_meta(key: &str) -> Result<Option<String>, String> {
    let conn = open_conn()?;
    conn.query_row(
        "SELECT value FROM app_meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn set_app_meta(key: &str, value: &str) -> Result<(), String> {
    let conn = open_conn()?;
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Missing key = legacy B/32 space (pre-B0 libraries).
pub fn library_embedding_model_id() -> Result<String, String> {
    Ok(get_app_meta(crate::t_ai::APP_META_EMBEDDING_MODEL_ID)?
        .unwrap_or_else(|| crate::t_ai::EMBEDDING_MODEL_CLIP_B32_LEGACY.to_string()))
}
```

Put the string constants in `t_ai.rs` (or `t_common.rs`) and re-export / use from sqlite.

- [ ] **Step 4: Seed on fresh DB after migrate in `create_db_internal`**

After `check_and_migrate(&conn)?`:

```rust
seed_embedding_model_meta_if_needed(&conn)?;
```

```rust
fn seed_embedding_model_meta_if_needed(conn: &Connection) -> Result<(), String> {
    ensure_app_meta_table(conn)?; // if not already
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = ?1",
            params![crate::t_ai::APP_META_EMBEDDING_MODEL_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if existing.is_some() {
        return Ok(());
    }
    let embed_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM afiles WHERE embeds IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if embed_rows == 0 {
        // Fresh library: bind to active B/16 without forcing a false stale rebuild.
        conn.execute(
            "INSERT INTO app_meta(key, value) VALUES(?1, ?2)",
            params![
                crate::t_ai::APP_META_EMBEDDING_MODEL_ID,
                crate::t_ai::EMBEDDING_MODEL_CLIP_B16
            ],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO app_meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![
                crate::t_ai::APP_META_EMBEDDING_MODEL_VER,
                crate::t_ai::EMBEDDING_MODEL_VER_B0
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    // else: legacy with embeds and no meta → leave missing → library_embedding_model_id = clip-b32
    Ok(())
}
```

Note: `seed_embedding_model_meta_if_needed` needs a conn-taking ensure for app_meta; implement `ensure_app_meta_table` once shared.

- [ ] **Step 5: Unit test meta helpers with temp DB (if project already uses rusqlite in tests)**

Add `#[cfg(test)]` that creates in-memory tables and asserts:

```rust
#[test]
fn library_model_id_defaults_to_legacy_when_missing() {
    // open memory conn, create app_meta only
    // assert get returns None → default string "clip-b32"
}

#[test]
fn seed_sets_b16_when_no_embeds() {
    // afiles empty embeds → seed → id is clip-b16
}
```

If in-memory wiring is too heavy vs existing patterns, skip automated test and rely on manual; prefer at least one pure helper test.

- [ ] **Step 6: Check compile**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: success.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/t_migration.rs src-tauri/src/t_sqlite.rs src-tauri/src/t_ai.rs
git commit -m "feat: app_meta schema v9 and embedding model id helpers"
```

---

### Task 4: `models_loadable` (cheap) + `ensure_embedding_space_ok`

**Files:**
- Modify: `src-tauri/src/t_ai.rs`
- Modify: `src-tauri/src/t_sqlite.rs`

- [ ] **Step 1: Cheap models_present check**

In `t_ai.rs`:

```rust
pub fn resource_models_present(app: &AppHandle) -> bool {
    match AiEngine::resource_model_dir(app) {
        Ok(dir) => {
            dir.join(crate::t_common::AI_VISION_MODEL).is_file()
                && dir.join(crate::t_common::AI_TEXT_MODEL).is_file()
                && dir.join(crate::t_common::AI_TOKENIZER).is_file()
        }
        Err(_) => false,
    }
}
```

Make `resource_model_dir` accessible (pub(crate) if needed). **Do not** open ORT sessions here.

- [ ] **Step 2: Clear all embeds**

```rust
pub fn clear_all_image_embeddings() -> Result<usize, String> {
    let conn = open_conn()?;
    let n = conn
        .execute("UPDATE afiles SET embeds = NULL", [])
        .map_err(|e| e.to_string())?;
    Ok(n)
}
```

- [ ] **Step 3: `ensure_embedding_space_ok`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingSpaceState {
    Ready,
    NeedsRebuild,
}

pub fn ensure_embedding_space_ok(app: &AppHandle) -> Result<EmbeddingSpaceState, String> {
    let active = crate::t_ai::EMBEDDING_MODEL_CLIP_B16;
    let library = library_embedding_model_id()?;
    if library == active {
        return Ok(EmbeddingSpaceState::Ready);
    }
    if !crate::t_ai::resource_models_present(app) {
        return Err(format!(
            "MODEL_MISSING: CLIP model files not found under resources/models"
        ));
    }
    // Hard-cut order: clear first, then write id.
    let _ = clear_all_image_embeddings()?;
    set_app_meta(crate::t_ai::APP_META_EMBEDDING_MODEL_ID, active)?;
    set_app_meta(
        crate::t_ai::APP_META_EMBEDDING_MODEL_VER,
        crate::t_ai::EMBEDDING_MODEL_VER_B0,
    )?;
    Ok(EmbeddingSpaceState::NeedsRebuild)
}
```

Also treat as NeedsRebuild when Ready but `COUNT(embeds IS NULL)` for images is high **only if** a prior cancel left holes — search policy can still fail closed when `embedded_count == 0 && total_image_count > 0` after user expected rebuild. Status command will expose counts.

- [ ] **Step 4: Wire ensure into search/generate**

At the start of:

1. `AFile::get_query_embedding` — before `encode_text` / image path  
2. `AFile::search_similar_images` — before scoring (also covers similar-image after query embed)  
3. `AFile::generate_embedding` — before skip-guard  

These need `AppHandle`. Today generate/search take `State<AiState>` only. Prefer:

- Pass `AppHandle` into `search_similar_images` / `generate_embedding` from `t_cmds` (add param), **or**
- Store `AppHandle` weakly on first `load_models` — avoid if possible  
- Minimal: change command signatures to take `app_handle: AppHandle` and pass to AFile methods.

Example command:

```rust
#[tauri::command]
pub fn search_similar_images(
    app_handle: AppHandle,
    state: State<t_ai::AiState>,
    params: ImageSearchParams,
) -> Result<Vec<AFile>, String> {
    AFile::search_similar_images(&app_handle, &state, params)
}
```

In method:

```rust
match ensure_embedding_space_ok(app_handle)? {
    EmbeddingSpaceState::Ready => {}
    EmbeddingSpaceState::NeedsRebuild => {
        return Err("INDEX_STALE: AI model upgraded; rebuild image embeddings".into());
    }
}
```

For `generate_embedding` during intentional rebuild, commands will call ensure first then loop with a `force` flag — see Task 5.

Skip-guard change:

```rust
// Only skip if library is already clip-b16 AND embeds non-empty AND !force
if !force {
    if let Ok(embeds) = Self::get_embedding_by_id(file_id) {
        if !embeds.is_empty() {
            return Ok("Embedding already exists".to_string());
        }
    }
}
```

Add `force: bool` parameter to internal generate, default false from public single-file command.

- [ ] **Step 5: cargo check**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/t_ai.rs src-tauri/src/t_sqlite.rs src-tauri/src/t_cmds.rs
git commit -m "feat: hard-cut embedding space with ensure_embedding_space_ok"
```

---

### Task 5: Rebuild IPC — status, rebuild, cancel

**Files:**
- Modify: `src-tauri/src/t_ai.rs` (or new small section in `t_sqlite.rs` / `t_cmds.rs`)
- Modify: `src-tauri/src/t_cmds.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-vite/src/common/api.js`

- [ ] **Step 1: Managed state (mirror face index)**

In `t_ai.rs` (or `t_sqlite.rs`):

```rust
pub struct ImageEmbedRebuildCancellation(pub Arc<Mutex<bool>>);
pub struct ImageEmbedRebuildStatus(pub Arc<Mutex<bool>>);
pub struct ImageEmbedRebuildProgressState(pub Arc<Mutex<ImageEmbeddingRebuildProgress>>);
```

Register in `main.rs` `.manage(...)` next to face state.

- [ ] **Step 2: Implement status**

```rust
#[tauri::command]
pub fn get_image_embedding_status(
    app_handle: AppHandle,
    rebuild_status: State<ImageEmbedRebuildStatus>,
    progress_state: State<ImageEmbedRebuildProgressState>,
) -> Result<ImageEmbeddingStatus, String> {
    let library_model_id = library_embedding_model_id()?;
    let active = EMBEDDING_MODEL_CLIP_B16.to_string();
    let models_present = resource_models_present(&app_handle);
    let (embedded_count, total_image_count) = count_image_embed_stats()?;
    let rebuild_running = *rebuild_status.0.lock().unwrap();
    let rebuild_progress = if rebuild_running {
        Some(progress_state.0.lock().unwrap().clone())
    } else {
        None
    };
    let needs_rebuild = library_model_id != active
        || (total_image_count > 0 && embedded_count == 0)
        || (rebuild_progress.as_ref().map(|p| p.cancelled).unwrap_or(false)
            && embedded_count < total_image_count);
    Ok(ImageEmbeddingStatus {
        active_model_id: active,
        library_model_id,
        needs_rebuild,
        rebuild_running,
        rebuild_progress,
        embedded_count,
        total_image_count,
        models_present,
    })
}
```

```rust
fn count_image_embed_stats() -> Result<(i64, i64), String> {
    let conn = open_conn()?;
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM afiles WHERE file_type IN (1, 3)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let embedded: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM afiles WHERE file_type IN (1, 3) AND embeds IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok((embedded, total))
}
```

- [ ] **Step 3: Implement rebuild command**

Prefer **async spawn** so UI does not block (face index pattern):

```rust
#[tauri::command]
pub fn rebuild_image_embeddings(
    app_handle: AppHandle,
    state: State<t_ai::AiState>,
    cancel_state: State<ImageEmbedRebuildCancellation>,
    status_state: State<ImageEmbedRebuildStatus>,
    progress_state: State<ImageEmbedRebuildProgressState>,
) -> Result<(), String> {
    {
        let mut running = status_state.0.lock().unwrap();
        if *running {
            return Err("REBUILD_RUNNING: embedding rebuild already in progress".into());
        }
        *running = true;
    }
    *cancel_state.0.lock().unwrap() = false;

    // Ensure space (clear + id) before listing work
    match ensure_embedding_space_ok(&app_handle)? {
        EmbeddingSpaceState::Ready | EmbeddingSpaceState::NeedsRebuild => {}
    }

    let ids = list_image_ids_missing_or_all_null_first()?;
    // list: prefer WHERE file_type IN (1,3) AND embeds IS NULL; if ensure just cleared, all NULL.

    let cancel = cancel_state.0.clone();
    let status = status_state.0.clone();
    let progress = progress_state.0.clone();
    let ai = state.0.clone(); // if Mutex in Arc — AiState is Mutex; use existing pattern

    // Run on blocking thread:
    std::thread::spawn(move || {
        let total = ids.len() as u64;
        let mut current = 0u64;
        let mut last_error = None;
        for id in ids {
            if *cancel.lock().unwrap() {
                let _ = app_handle.emit(
                    "image_embedding_rebuild_progress",
                    ImageEmbeddingRebuildProgress {
                        current,
                        total,
                        running: false,
                        cancelled: true,
                        last_error: last_error.clone(),
                    },
                );
                *status.lock().unwrap() = false;
                return;
            }
            // call generate with force=false after clear (NULL only)
            let gen = {
                let mut engine_guard = /* lock AiState */;
                AFile::generate_embedding_inner(&engine_guard, id, false)
            };
            if let Err(e) = gen {
                last_error = Some(e);
            }
            current += 1;
            if current % 5 == 0 || current == total {
                let _ = app_handle.emit(
                    "image_embedding_rebuild_progress",
                    ImageEmbeddingRebuildProgress {
                        current,
                        total,
                        running: true,
                        cancelled: false,
                        last_error: last_error.clone(),
                    },
                );
            }
        }
        *status.lock().unwrap() = false;
        let _ = app_handle.emit(
            "image_embedding_rebuild_progress",
            ImageEmbeddingRebuildProgress {
                current,
                total,
                running: false,
                cancelled: false,
                last_error,
            },
        );
    });
    Ok(())
}
```

Adjust locking to match how `AiState` is defined (`pub struct AiState(pub Mutex<AiEngine>)` — clone `AppHandle` + use `State` carefully: face index clones `FaceState` which is `Arc`. For `AiState`, take `AppHandle` and resolve via `app.state::<AiState>()` inside the thread.

**Concrete pattern (use this):**

```rust
std::thread::spawn(move || {
    let ai_state = app_handle.state::<t_ai::AiState>();
    for id in ids {
        if *cancel.lock().unwrap() { /* emit cancelled; break */ }
        let res = AFile::generate_embedding(&ai_state, id);
        // ...
    }
});
```

- [ ] **Step 4: Cancel command**

```rust
#[tauri::command]
pub fn cancel_rebuild_image_embeddings(
    cancel_state: State<ImageEmbedRebuildCancellation>,
) -> Result<(), String> {
    *cancel_state.0.lock().unwrap() = true;
    Ok(())
}
```

- [ ] **Step 5: Register in `main.rs`**

```rust
.manage(t_ai::ImageEmbedRebuildCancellation(Arc::new(Mutex::new(false))))
.manage(t_ai::ImageEmbedRebuildStatus(Arc::new(Mutex::new(false))))
.manage(t_ai::ImageEmbedRebuildProgressState(Arc::new(Mutex::new(
    ImageEmbeddingRebuildProgress {
        current: 0,
        total: 0,
        running: false,
        cancelled: false,
        last_error: None,
    },
))))
// invoke_handler:
t_cmds::get_image_embedding_status,
t_cmds::rebuild_image_embeddings,
t_cmds::cancel_rebuild_image_embeddings,
```

- [ ] **Step 6: Frontend API wrappers**

In `api.js` next to image search:

```js
export async function getImageEmbeddingStatus() {
  try {
    return await invoke('get_image_embedding_status');
  } catch (error) {
    console.error('getImageEmbeddingStatus error:', error);
    throw error;
  }
}

export async function rebuildImageEmbeddings() {
  try {
    return await invoke('rebuild_image_embeddings');
  } catch (error) {
    console.error('rebuildImageEmbeddings error:', error);
    throw error;
  }
}

export async function cancelRebuildImageEmbeddings() {
  try {
    return await invoke('cancel_rebuild_image_embeddings');
  } catch (error) {
    console.error('cancelRebuildImageEmbeddings error:', error);
  }
}

export async function listenImageEmbeddingRebuildProgress(callback) {
  return await listen('image_embedding_rebuild_progress', callback);
}
```

Also update `searchSimilarImages` to **rethrow** or return a structured error when message starts with `INDEX_STALE` so Content can show the banner (today it swallows to `[]`):

```js
export async function searchSimilarImages(params) {
  try {
    // ... setImageSearchModel only for model 0 after multilingual disabled
    const results = await invoke('search_similar_images', { params });
    return results || [];
  } catch (error) {
    const msg = String(error?.message || error || '');
    if (msg.includes('INDEX_STALE') || msg.includes('MODEL_MISSING')) {
      const err = new Error(msg);
      err.code = msg.startsWith('MODEL_MISSING') ? 'MODEL_MISSING' : 'INDEX_STALE';
      throw err;
    }
    console.error('searchSimilarImages error:', error);
    return [];
  }
}
```

- [ ] **Step 7: cargo check + commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/t_ai.rs src-tauri/src/t_sqlite.rs src-tauri/src/t_cmds.rs src-tauri/src/main.rs src-vite/src/common/api.js
git commit -m "feat: whole-library image embedding rebuild IPC and status"
```

---

### Task 6: Reject multilingual text-only

**Files:**
- Modify: `src-tauri/src/t_ai.rs` (`set_text_model`)
- Modify: `src-tauri/src/t_cmds.rs` (`set_image_search_model`)
- Modify: `src-vite/src/views/Settings.vue`
- Modify: `src-vite/src/stores/configStore.js` (normalize model to 0)
- Modify: `src-vite/src/common/api.js` (`searchSimilarImages` no longer tries model 1)

- [ ] **Step 1: Rust reject**

In `set_text_model`:

```rust
if model == ImageSearchTextModel::Multilingual {
    return Err(
        "MULTILINGUAL_DISABLED: multilingual text-only is disabled with CLIP B/16 vision; use default model"
            .into(),
    );
}
```

- [ ] **Step 2: Settings UI — only default option**

In `imageSearchModelOptions` computed, return a single Default option, or disable the select and show static text:

```js
const imageSearchModelOptions = computed(() => {
  const options = localeMsg.value.settings.image_search.search_model_options || ['Default'];
  return [{ value: 0, label: options[0] || 'Default' }];
});
```

Remove / hide download multilingual UI block (v-if false) for B0.

Force on load:

```js
config.settings.imageSearch.model = 0;
```

- [ ] **Step 3: api.js** — remove multilingual fallback branch in `searchSimilarImages` or keep only `setImageSearchModel(0)`.

- [ ] **Step 4: i18n hint** — default model hint mentions ViT-B/16 quantized (Task 7 keys).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/t_ai.rs src-vite/src/views/Settings.vue src-vite/src/common/api.js src-vite/src/stores/configStore.js
git commit -m "fix: disable multilingual text-only with CLIP B/16 default"
```

---

### Task 7: UI banner + i18n

**Files:**
- Modify: `src-vite/src/components/Content.vue`
- Modify: `src-vite/src/locales/en.json`
- Modify: `src-vite/src/locales/zh.json`
- Modify: `src-vite/src/views/Settings.vue` (one-line model description)

- [ ] **Step 1: Add i18n keys**

`en.json` under `search` or `settings.image_search`:

```json
"embedding_rebuild_banner": "AI search model upgraded. Rebuild the image index to use search and smart tags.",
"embedding_rebuild_action": "Rebuild now",
"embedding_rebuild_progress": "Rebuilding AI index… {current}/{total}",
"embedding_rebuild_cancel": "Cancel",
"embedding_model_missing": "AI model files are missing. Run the model download script or reinstall.",
"default_model_hint": "CLIP ViT-B/16 (quantized), English-oriented image search"
```

`zh.json` equivalents (中文).

- [ ] **Step 2: Content.vue — state + poll/listen**

On library open / when entering search or smart-tag:

```js
import {
  getImageEmbeddingStatus,
  rebuildImageEmbeddings,
  cancelRebuildImageEmbeddings,
  listenImageEmbeddingRebuildProgress,
} from '@/common/api';

const embeddingStatus = ref(null);
const rebuildProgress = ref(null);

async function refreshEmbeddingStatus() {
  try {
    embeddingStatus.value = await getImageEmbeddingStatus();
  } catch (e) {
    console.error(e);
  }
}

async function startEmbeddingRebuild() {
  await rebuildImageEmbeddings();
  await refreshEmbeddingStatus();
}
```

In `getImageSearchFileList` catch:

```js
try {
  const result = await searchSimilarImages(...);
  // ...
} catch (e) {
  if (e?.code === 'INDEX_STALE' || String(e.message||'').includes('INDEX_STALE')) {
    await refreshEmbeddingStatus();
    // show banner; clear fileList
    fileList.value = [];
  } else if (/* MODEL_MISSING */) {
    // toast
  }
}
```

Banner template (near content header when `embeddingStatus?.needsRebuild || rebuildProgress`):

```html
<div v-if="embeddingStatus?.needsRebuild || embeddingStatus?.rebuildRunning" class="alert ...">
  <span>{{ $t('...embedding_rebuild_banner') }}</span>
  <button v-if="!embeddingStatus?.rebuildRunning" @click="startEmbeddingRebuild">
    {{ $t('...embedding_rebuild_action') }}
  </button>
  <span v-else>{{ progress text }}</span>
  <button v-if="embeddingStatus?.rebuildRunning" @click="cancelRebuildImageEmbeddings">
    {{ $t('...embedding_rebuild_cancel') }}
  </button>
</div>
```

Listen progress on mount; unlisten on unmount.

- [ ] **Step 3: Frontend build**

```bash
pnpm --dir src-vite build
```

Expected: success.

- [ ] **Step 4: Commit**

```bash
git add src-vite/src/components/Content.vue src-vite/src/locales/en.json src-vite/src/locales/zh.json src-vite/src/views/Settings.vue
git commit -m "feat: AI embedding rebuild banner and B/16 settings copy"
```

---

### Task 8: End-to-end verification

**Files:** none (manual)

- [ ] **Step 1: Fresh library**

1. New library (or empty).  
2. Confirm `app_meta` has `embedding_model_id=clip-b16` (use DB browser or log).  
3. Import a few images; index/scan so embeds generate (scan path still uses `generate_embedding`).  
4. Text + smart tags work without INDEX_STALE.

- [ ] **Step 2: Legacy library simulation**

1. Open a DB that has embeds from B/32 (or set `app_meta` to missing / `clip-b32` and leave embeds).  
2. Open AI search → must **not** return ranked garbage; banner / INDEX_STALE.  
3. Rebuild → progress events → search works.  
4. Cancel mid-rebuild → resume fills remaining NULLs.

- [ ] **Step 3: Multilingual**

1. Settings has no usable model=1 path.  
2. `set_image_search_model(1)` via invoke fails with MULTILINGUAL_DISABLED.

- [ ] **Step 4: Tooling**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
pnpm --dir src-vite build
```

- [ ] **Step 5: GROW**

Update `.mex/ROUTER.md` Working line: B0 **shipped** (not just design).  
Update pattern status table B0 → implemented date.  
`mex log` decision note.

```bash
git add .mex/ROUTER.md .mex/patterns/change-image-search-model.md .mex/events/decisions.jsonl
git commit -m "docs: mark Track B0 CLIP B/16 default bump as shipped"
```

---

## Call-site checklist (do not miss)

| Call site | Action |
|-----------|--------|
| `AFile::search_similar_images` | `ensure_embedding_space_ok`; INDEX_STALE if NeedsRebuild |
| `AFile::get_query_embedding` | same before encode |
| `AFile::generate_embedding` | ensure when not inside rebuild-after-ensure; skip only if Ready + non-empty + !force |
| `rebuild_image_embeddings` | ensure first, then NULL-id loop |
| Scan/index workers calling generate | After B0, library already clip-b16 or first generate triggers ensure — ensure must not clear on every single generate when already Ready |

**Cheap `models_loadable`:** file existence only (or `engine.is_loaded()`). Never reload ORT on each search.

---

## Self-review vs design

| Design requirement | Task |
|--------------------|------|
| B/16 quant download scripts | Task 1 |
| ort smoke | Task 2 |
| app_meta v9 | Task 3 |
| Seed new library clip-b16 | Task 3 |
| ensure clear→id order; no clear if missing models | Task 4 |
| Search/generate gates | Task 4 |
| Required rebuild + status + cancel IPC | Task 5 |
| Multilingual Rust+UI reject | Task 6 |
| Banner CTA | Task 7 |
| Acceptance / GROW | Task 8 |
| No SigLIP | Out of scope all tasks |
| Track A thresholds unchanged | No task touches floors |
| No ONNX in git | Task 1 note |

---

## Out of scope (do not implement in this plan)

- SigLIP / model registry / download packs under app-data  
- Progressive search while rebuild running (optional polish later)  
- Store-time L2 normalize (search already normalizes)  
- Changing smart-tag prompts  
