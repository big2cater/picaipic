---
name: change-image-search-model
description: Optional multilingual SigLIP sideload (Track B); bundled default stays CLIP B/32. B0 B/16 default bump abandoned after owner trial.
last_updated: 2026-07-23
---

# Change image-search model (bundled CLIP B/32 + optional multilingual SigLIP)

## When to use
- Switch the local text↔image embedding model used by AI search, similar-image, and smart tags
- Wire settings UI for downloadable BETA models (primary: **multilingual SigLIP** ONNX pack) — Track B
- Bind per-library embeddings to a `modelId` and rebuild when the active model changes
- Generalize the existing multilingual text-model download path into multi-file model packs (Track B)

## Status
| Track | Status |
|-------|--------|
| **A. Stop-bleed** | **Shipped (2026-07-22)** on working tree: floors `0.30/0.26/0.22/0.18`, smart-tag `0.22`, histogram logs in `search_similar_images` |
| **B0. CLIP B/16 default** | **Abandoned (2026-07-23)** — owner trial: B/16 quant felt **≈ B/32** for library search; not worth reindex/slower encode. **Do not merge** `feat/clip-b16-default-bump`. Design/plan kept as historical: `docs/superpowers/specs/2026-07-23-clip-b16-default-bump-design.md`, `docs/superpowers/plans/2026-07-23-clip-b16-default-bump.md` |
| **B. Multi-model SigLIP/SigLIP2** | **Phase 0 Python+Rust (2026-07-23):** `onnx-community/siglip2-base-patch16-224-ONNX` — Python ORT OK on int8/quant/fp16; **Rust ort FAIL on int8** (`ConvInteger`); **Rust ort PASS on `*_quantized` and fp16**. Prefer **quantized dual-tower** for product. Scripts: `probe_siglip2_onnx.py` + `probe_siglip2_ort/`. Guide: `docs/guide/siglip2-phase0-probe.md`. Default remains CLIP B/32; product UI still deferred until real-album smoke. |

Empty smart-tag / text-search results were primarily **threshold vs score-scale** (Track A), not “need a bigger CLIP”.

**Default production path:** bundled **CLIP ViT-B/32** quantized (Xenova `clip-vit-base-patch32`; `vision_model.onnx` / `text_model.onnx` / `tokenizer.json`).

## Stop-bleed vs long-term (do not conflate)

| Track | What | Fixes empty results? | Fixes “not accurate enough”? | Cost |
|-------|------|----------------------|------------------------------|------|
| **A. Stop-bleed** | Lower CLIP floors into the real text→image score band; short smart-tag prompts; score histogram logs | **Yes** (when floors sat above max scores) | Only slightly | No download, no reindex — **done** |
| **B0. B/16 (abandoned)** | Bundled CLIP B/16 int8 | No | Owner: little subjective gain | Reindex; slower encode — **not shipping** |
| **B. Long-term** | Optional **full-stack** sideload — **SigLIP2 int8** (onnx-community) and/or multilingual SigLIP | Only after Phase 0–3 | **Main goal** (quality + ideally CN) | Download + full reindex + multi-day eng |

**Do not implement B0 as default.**  
**Track B Phase 0** starts with `python scripts/probe_siglip2_onnx.py` + Rust ort confirmation — see `docs/guide/siglip2-phase0-probe.md`.  
**Track A is done.** Keep histogram logs when calibrating any future floors.

### Threshold truth table (easy to get wrong)
| Layer | Values | Notes |
|-------|--------|--------|
| `main` / last published intent | `[0.40, 0.34, 0.28, 0.22]`, smart-tag `0.28` | Too high for many text/smart-tag queries → empty UI |
| Working tree stop-bleed (2026-07-22) | `[0.30, 0.26, 0.22, 0.18]`, smart-tag `0.22` | **Provisional** until histogram logs prove the band |
| BGE/SigLIP manifest placeholders | e.g. illustrative threshold arrays in sample manifests | **Guess — do not ship** without calibration on real albums |
| “Typical 0.18–0.30” prose | Heuristic for CLIP-scale cosine | **Hypothesis**, not measured in CI |

Rule: **any number in a shipped threshold table must come from measured distributions** (see diagnostics in `search_similar_images`), not from this doc’s examples.

## Goals / non-goals

### Goals (B0 — implement next)
1. Replace bundled default with **CLIP B/16 int8/quant** (same resource filenames; download scripts + packaging).
2. Per-library `app_meta.embedding_model_id`; legacy → treat as `clip-b32`; active → `clip-b16`; mismatch → clear embeds + rebuild (fail closed).
3. Reject multilingual text-only (`model=1`) while vision is B/16 (Rust + UI).
4. Smart tags, free-text search, and find-similar share one active engine.

### Goals (Track B — deferred)
1. Settings: choose **bundled CLIP** vs BETA sideload — **primary: multilingual SigLIP**.
2. Download → checksum → trial load → activate; on failure keep previous model.
3. Rebuild embeds on model change; never mix spaces.

### Non-goals
- **B0:** do **not** build SigLIP UI/download; do **not** keep dual B/32+B/16 ranking.
- **Track B sideload:** do **not** overwrite `resources/models/*` with download packs (install under app-data). **B0 packaging may replace** bundled resource files via download scripts.
- Do **not** ship large/VLM in the same MVP.
- Do **not** treat legacy `imageSearch.model=1` as a vision upgrade.
- Do **not** run two embedding spaces in one library at once.
- Do **not** claim “download finished ⇒ search works” without rebuild.
- Empty smart tags today: Track A (done), not SigLIP.

## Current baseline (code truth)
| Piece | Location / behavior |
|-------|---------------------|
| Bundled files (pre-B0) | `src-tauri/resources/models/{vision_model,text_model}.onnx`, `tokenizer.json` — was B/32 quant; **B0 → B/16 quant**, same filenames |
| B0 design | `docs/superpowers/specs/2026-07-23-clip-b16-default-bump-design.md` |
| Constants | `t_common::AI_VISION_MODEL` / `AI_TEXT_MODEL` / `AI_TOKENIZER` |
| Engine | `t_ai.rs` `AiEngine` — ONNX Runtime, CLIP preprocess **224** + CLIP mean/std |
| Optional download today | Multilingual **text + tokenizer only** → `{app_data}/models/multilingual/` |
| Settings | `imageSearch.model` `0` default / `1` multilingual text; download UX in `Settings.vue` |
| Search | `AFile::search_similar_images` + `ImageSearchParams.threshold` |
| Thresholds | See **Threshold truth table** above; do not assume main tip already matches stop-bleed |
| Diagnostics | `search_similar_images` logs one line per query: `thr`, `candidates`, `hit`, `max`, counts `>0.18/>0.22/>0.28/>0.34/>0.40`, top-3 scores — use to calibrate floors |

## Architecture

```
Settings (modelId, download, rebuild CTA)
        │ IPC
        ▼
AiEngine registry (manifest → load/unload sessions)
        │
   ┌────┴────┐
   ▼         ▼
Resource   App-data sideload
(CLIP)     (SigLIP pack)
        │
        ▼
SQLite embeds + library embedding_model_id
search_similar_images (cosine, per-manifest thresholds)
```

**Principle:** sideload install + `modelId` switch + embeddings bound to model.

## Directory layout

### Bundled (read-only, never overwrite)
```
src-tauri/resources/models/
  vision_model.onnx      # CLIP B/32
  text_model.onnx
  tokenizer.json
```
CLIP manifest may live in code as a built-in `ModelManifest` (no file required for MVP).

### Optional download (writable)
```
{app_data}/models/image-search/
  siglip-base-patch16-256/
    vision.onnx
    text.onnx
    tokenizer.json
    manifest.json
  siglip-base-patch16-256.download.<id>/   # temp; atomic rename on success
```

### manifest.json (per sideload model)
```json
{
  "id": "siglip-base-patch16-256",
  "family": "siglip",
  "displayName": "SigLIP base patch16-256",
  "beta": true,
  "embeddingDim": 768,
  "imageSize": 256,
  "mean": [0.5, 0.5, 0.5],
  "std": [0.5, 0.5, 0.5],
  "normalizeEmbeddings": true,
  "files": {
    "vision": "vision.onnx",
    "text": "text.onnx",
    "tokenizer": "tokenizer.json"
  },
  "thresholds": [0.28, 0.24, 0.20, 0.16],
  "smartTagThreshold": 0.20,
  "version": 1
}
```
**Placeholder values — do not ship as-is:**
- `mean` / `std` / `embeddingDim` / ONNX I/O names → from **actual exported ONNX** (Phase 0 probe).
- `thresholds` / `smartTagThreshold` → from **measured score histograms** on real libraries after SigLIP embeds exist (same log line shape as CLIP diagnostics). The numbers above are **illustrative only**.

### Download source
| Source | Role |
|--------|------|
| Primary | Self-hosted release (same pattern as `picaipic-binaries` multilingual assets) with **sha256** |
| Optional | Hugging Face mirror |
| Payload | Pre-exported ORT-friendly **vision + text + tokenizer**, not raw safetensors alone |

## Config & library metadata

### Frontend settings
Migrate from magic int toward stable id:

```js
imageSearch: {
  modelId: 'clip-b32',  // primary
  // legacy: model 0 → clip-b32, 1 → clip-multilingual-text (if kept)
  thresholdIndex: 3,
  limit: 1000,
}
```

### Per-library binding (required) — **storage decision**
**Decision (spec lock-in):** store in the **per-library SQLite** via a small kv table, not app-global settings (each library can be mid-rebuild / on a different model history).

Migration (next schema bump when implementing Phase 3):

```sql
CREATE TABLE IF NOT EXISTS app_meta (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL
);
-- keys:
--   embedding_model_id   e.g. clip-b32 | siglip-base-patch16-256
--   embedding_model_ver  manifest.version as decimal string
```

| Key | Meaning |
|-----|---------|
| `embedding_model_id` | Active embeds space for **this** library DB |
| `embedding_model_ver` | `manifest.version` |

Defaults when row missing: treat as `clip-b32` / ver `0` (legacy libraries that only ever used bundled CLIP).

**Before search:** if `library.embedding_model_id != engine.active_model_id` → fail closed with a clear error (e.g. `INDEX_STALE`); UI offers rebuild. Never silently rank with the wrong space.

### Embeds
- Keep `afiles.embeds` BLOB.
- On model change: **clear all embeds** (or mark invalid), then batch `generate_embedding`.
- No mixed dimensions in one library.

## Behaviour contract
1. Bundled CLIP remains the default and the only required install asset.
2. Sideload models install under app-data; **never** replace resource-dir CLIP files.
3. Activate only after trial ORT load succeeds; settings `modelId` updates only then.
4. Switching models (including back to CLIP) requires embed rebuild when ids differ.
5. Thresholds and smart-tag floor come from the **active manifest** (or CLIP defaults), not a single global table forever.
6. Multilingual text-only option (if retained) is another `modelId`; only one vision+text stack loaded at a time.
7. Download is cancellable; temp dirs cleaned; corrupt packs must not become active.
8. Local-first: no required cloud inference; download is optional asset fetch only.

## IPC (proposed)
| Command | Role |
|---------|------|
| `list_image_search_models` | Built-in + optional: id, beta, installed, size, active |
| `get_image_search_model_status` | activeId, libraryMatch, needsRebuild, download state |
| `set_image_search_model` | Load by id; Err keeps previous engine state |
| `download_image_search_model` | Pack download + verify + trial load; progress events |
| `cancel_image_search_model_download` | Cancel |
| `clear_image_embeddings` | Clear library embeds + meta |
| `rebuild_image_embeddings` | Async batch + progress (reuse scan/embedding concurrency patterns) |

Events (extend existing multilingual progress style):
- `image_search_model_download_progress`
- `image_embedding_rebuild_progress`

Wire through `main.rs` + `src-vite/src/common/api.js` per `patterns/add-tauri-command.md`.

## Key files (implementation map)
| Layer | Path |
|-------|------|
| Engine | `src-tauri/src/t_ai.rs` |
| Constants | `src-tauri/src/t_common.rs` |
| Commands | `src-tauri/src/t_cmds.rs`, `main.rs` |
| DB / search / embed gen | `src-tauri/src/t_sqlite.rs` |
| Index queue | `src-tauri/src/t_utils.rs` |
| Settings UI | `src-vite/src/views/Settings.vue` |
| Settings state | `src-vite/src/stores/configStore.js` |
| IPC wrappers | `src-vite/src/common/api.js` |
| Search / smart-tag call sites | `Content.vue`, `smartTags.ts` |
| i18n | `locales/en.json`, `zh.json` |
| Related | `change-ai-search-filters.md`, `change-smart-tags.md`, `change-library-perf.md` |

## Engine design notes
- Introduce `ModelManifest` + `family`: at least `clip` | `siglip` (later optional `bge_vl`).
- Fold current CLIP path into `family=clip` without behavior change.
- **Primary sideload path — `family=siglip` (multilingual pack by default):**
  - Image: typically **256**, mean/std often **0.5**, output names from Phase 0 probe, optional L2.
  - Text: **SentencePiece** (not CLIP BPE) — pack usually ships `tokenizer.json` **and/or** `spiece.model`; load the pack’s tokenizer API correctly in Rust `tokenizers`.
  - Template: typically **no BOS**, **EOS at end**, **max length ~64** (confirm on export).
  - Languages: multilingual checkpoint covers Chinese + English (+ others); English smart-tag prompts remain valid.
- **EN-only SigLIP** (e.g. Xenova `siglip-base-patch16-256`): same `family=siglip` preprocess/token rules; weaker for Chinese free-text — keep as alternate pack id if needed.
- **BGE-VL:** conditional future only if a verified ONNX export appears (see Decision log).
- Prefer **hot reload**; prompt restart only if reload fails.
- `encode_text` / `encode_image` / `generate_embedding` always use the **active** full pack.

### Full-stack swap (not text-only)
Current multilingual option only swaps the **text** tower while keeping bundled CLIP vision. That is **invalid** for SigLIP (and any non-CLIP family): image embeds and text embeds must live in the **same** space.

| Rule | Detail |
|------|--------|
| Load unit | Always load **vision + text + tokenizer assets** from the same pack/`family` (e.g. `vision.onnx`, `text.onnx`, `tokenizer.json`, and `spiece.model` if present) |
| Unload unit | Drop the whole stack when switching family (do not leave CLIP vision + SigLIP text) |
| Legacy `model=1` multilingual CLIP text | Remains CLIP-family only; still paired with CLIP vision. Document as `clip-multilingual-text` — **not** a vision upgrade and **not** a substitute for multilingual SigLIP |
| Index | Any family / dim change → clear + rebuild embeds (CLIP **512-d** vs SigLIP often **768-d**; never mix rows) |

`generate_embedding` must use the **active** vision session, never a hard-coded resource path after multi-model lands.

### Tokenizer / special tokens (**highest footgun**)
**Baseline code truth (`t_ai.rs` today):**
- Loads HuggingFace `tokenizers` via `Tokenizer::from_file(tokenizer.json)`.
- Calls `tokenizer.encode(text, true)` — **does not** hand-append BOS/EOS in Rust.
- CLIP pack’s `tokenizer.json` uses `RobertaProcessing` with `<|startoftext|>` / `<|endoftext|>` (BOS/EOS for CLIP).
- **No** truncation/padding config is set in the JSON we ship (`truncation`/`padding` null); CLIP convention is **max 77**.

**CLIP vs SigLIP (multilingual pack same SigLIP text rules; confirm Phase 0):**

| | CLIP (bundled) | SigLIP (EN or multilingual) |
|--|----------------|-----------------------------|
| Special tokens | BOS + EOS | Often **no BOS**; **EOS at end** |
| Max length | 77 | Often **64** |
| Vocab / type | CLIP BPE (~49k) | **SentencePiece** (not CLIP BPE) — own tokenizer files |
| Files | `tokenizer.json` | `tokenizer.json` and often **`spiece.model`** — do not drop SP piece if the pack needs it |
| Reuse | — | **Never** reuse CLIP `tokenizer.json` with SigLIP ONNX |

**Implementation requirements:**
1. Each model pack carries **its own** tokenizer files; switching `modelId` reloads tokenizer with vision/text.
2. Prefer JSON post-processor / SP config for special tokens (`encode(..., add_special_tokens=true)`). If the export’s JSON does not apply SigLIP template, branch on `family` — **never** hard-code CLIP BOS/EOS for SigLIP.
3. Explicit **truncation + padding** per manifest (`max_length` 77 vs 64, pad id, attention_mask).
4. Phase 0 acceptance: encode fixed CN + EN strings; dump `ids[0]` / `ids[-1]` / `len`; match a Python/transformers reference for **that** checkpoint. Wrong template ⇒ garbage retrieval with “successful” ORT load.
5. Multilingual vs EN-only packs share `family=siglip` code paths; differ by **weights + tokenizer files + modelId**, not by inventing a second text stack.

### ORT / export compatibility
- App uses **Rust `ort`** (native ONNX Runtime), not `onnxruntime-web`.
- Community/Xenova ONNX is often aimed at transformers.js; **most** ops load, but opset / unsupported ops / dynamic axes can fail only at `Session::commit_from_file` or first `run`.
- **Trial load** (already in download flow) is mandatory: load vision + text + one dummy `encode_text` + one dummy `encode_image` before rename-to-final / activate.
- Prefer a **known-good export pipeline** (documented commit + opset) over “whatever HF file”; host on `picaipic-binaries` (or equivalent) with **sha256** (trust boundary; aligns with not shipping arbitrary remote weights without verification).

### Output tensors & pooling
- Do not assume CLIP names (`pooler_output` / `text_embeds` / `image_embeds` / `last_hidden_state` + first-token).
- Phase 0 probe lists input/output names and ranks; write them into manifest (or a small `family` table in code).
- If using `last_hidden_state`, define pooling (CLS / first token / mean over mask) per family — wrong pooling looks like “random” search.

### L2 normalize & cosine
- Document whether the ONNX already L2-normalizes. If not, normalize in Rust before store/search so cosine stays consistent.
- Dim mismatch (512 vs 768) already returns 0 from `cosine_similarity_blob`; still **clear embeds** on switch so UI does not show empty/weird partial hits.

### Thresholds
- SigLIP uses a different training objective (sigmoid / temperature-scaled cosine). **Score scale ≠ CLIP.**  
- Manifest `thresholds` / `smartTagThreshold` (any sample array like `[0.28,0.24,0.20,0.16]`) stay **placeholders until measured** on real albums after rebuild (same histogram log shape as Track A).

## Candidate pack (primary BETA)

| Field | Value |
|-------|--------|
| **modelId** (suggested) | `siglip-base-patch16-256-multilingual` |
| **Upstream weights** | Google multilingual SigLIP base patch16-256 (official research checkpoint lineage) |
| **Community ONNX pack (starting point)** | e.g. `ajaleksa/siglip-base-patch16-256-multilingual-onnx` (verify current files/sizes; **not** Google-official ONNX) |
| **Also check** | quantized variants (e.g. community qint8) if quality/size trade-off is acceptable |
| **Expected files** | vision ONNX + text ONNX + tokenizer (`tokenizer.json` / `spiece.model` as shipped) + our `manifest.json` |
| **embeddingDim** | often **768** (probe; write into manifest) |
| **Distribution** | Mirror to `picaipic-binaries` (or equivalent) with **pinned revision + sha256** — do not hotlink unpinned HF forever |

## End-to-end flows

### Enable multilingual SigLIP
1. User selects multilingual SigLIP BETA.
2. If not installed → confirm (size, rebuild required) → download to temp → sha256 → **trial load** (vision+text+dummy encode CN+EN) → rename into final dir.
3. `set_image_search_model` succeeds → persist `modelId`.
4. If library `embedding_model_id` mismatches → prompt rebuild now/later.
5. Rebuild: clear embeds → batch generate → write `embedding_model_id` + version.
6. Search / smart tags enabled (CN free-text + EN smart tags).

### Fallback
- Any load/download failure → do not persist new id; engine stays on last good model; toast error.
- Bundled CLIP always installable without network.

### Switch back to CLIP
- Load resource models; if library meta still SigLIP → rebuild again (spaces differ both ways).
- Keep downloaded SigLIP pack on disk for re-enable.

## Phased delivery
| Phase | Work | Notes |
|-------|------|--------|
| **0 Assets** | **Primary:** multilingual SigLIP community ONNX pack (e.g. ajaleksa multilingual export) — verify file layout, I/O names, SP tokenizer + no-BOS/EOS/max 64, preprocess 256/0.5, dim 768; dummy encode **CN+EN** on Rust `ort`; optional quantized variant survey; **self-host + sha256** | **Blocks UI** if trial load fails |
| **1 Engine** | Registry, CLIP refactor, `family=siglip` (image 256/0.5 + SentencePiece + token template), full-stack load, `set_image_search_model` | Dev flag before UI |
| **2 Download + Settings** | Generalize multilingual downloader; BETA dropdown (CLIP vs multilingual SigLIP); progress/cancel; `modelId` migration | Do not present legacy text-only as “upgrade vision” |
| **3 Index binding** | `app_meta` embedding ids, stale check, clear/rebuild, progress | Fail closed |
| **4 Polish** | Measured SigLIP thresholds, smart-tag + CN free-text regression, i18n | |
| **5 Optional** | EN-only SigLIP pack id; BGE-VL **only if** verified ONNX export appears | Same machinery |

Rough effort after Phase 0: **~4–8 engineer-days**.

## Risks
| Risk | Mitigation |
|------|------------|
| Pack fails Rust `ort` trial load | Phase 0 gate; do not ship UI |
| **SentencePiece / wrong special tokens / max 64** | Pack-local SP+JSON; family branch; CN+EN id dump vs reference |
| **Text-only swap** (CLIP vision + new text) | Forbidden for non-CLIP family; load/unload full stack |
| Community (not Google-official) ONNX | Pin revision; self-host; sha256; document provenance |
| FP32 pack size (hundreds MB–~1GB) | Honest UI size; later quantize if quality OK |
| Wrong output / pooling | Phase 0 probe |
| 768-d rebuild cost | Progress, cancel, semaphore; fail closed if skipped |
| Score scale ≠ CLIP | Histogram calibration after rebuild |
| User skips rebuild | `INDEX_STALE` |
| Disk / ORT memory | Delete pack option; unload old sessions |

## Acceptance
1. Fresh install: CLIP-only behavior unchanged.
2. After SigLIP download, files live under app-data; **resources/models untouched**.
3. Corrupt pack → load fails → still on CLIP.
4. Switch without rebuild → search shows rebuild requirement, not garbage ranking.
5. After rebuild: text search + smart tags + similar work; spot-check birds/plants better or equal subjectively.
6. Switch back to CLIP + rebuild restores baseline.
7. `pnpm --dir src-vite build` and `cargo check --manifest-path src-tauri/Cargo.toml` pass when code lands.

## Verify (when implementing)
```bash
cargo check --manifest-path src-tauri/Cargo.toml
pnpm --dir src-vite build
```
Manual: Settings model switch, cancel download, failed load fallback, stale search banner, full embed rebuild on a small album.

## Decision log (product)

### 2026-07-23 — B0: default bundled **CLIP B/16 int8** before SigLIP
**Decision:** Implement Plan B / Track **B0** next: replace Xenova B/32 quant with **B/16 quant/int8** as the only bundled default; hard-cut via `app_meta`; **no SigLIP** in this step. Multilingual text-only disabled (Rust reject + UI hide). Design: `docs/superpowers/specs/2026-07-23-clip-b16-default-bump-design.md`.  
**Reasoning:** Volume similar to current pack; finer patches improve detail/similar retrieval modestly; same CLIP preprocess/512-d → small engine delta; still does not fix CN free-text (Track B). Owner libraries can be wiped; product still fail-closed.  
**Consequences:** Full reindex on upgrade; slower encode than B/32; Track B deferred but reuses `app_meta`.

### 2026-07-23 — Primary BETA (Track B, deferred): **multilingual SigLIP ONNX**
Ranking table (why this wins the product basket):

| Candidate | ONNX ready | Chinese free-text | Stronger dual-tower than bundled CLIP | Verdict |
|-----------|------------|-------------------|----------------------------------------|---------|
| EN-only SigLIP (e.g. Xenova base) | Yes | No | Yes | Good EN upgrade; **misses CN** |
| BGE-VL-base | **No** known good public ONNX (export project) | Yes | Yes (retrieval-tuned) | Quality attractive; **blocked on export** |
| Legacy `model=1` multilingual **text only** | Yes (existing path) | Partial | **No** (vision still CLIP) | Does **not** upgrade image embeds |
| **Multilingual SigLIP ONNX** (e.g. ajaleksa pack) | **Yes** (community) | **Yes** | **Yes** | **Best trade-off if Phase 0 trial load passes** |

**One model gives:** stronger retrieval than weak B/32 **and** Chinese free-text **and** an existing dual-tower ONNX layout (verify files), without self-authoring BGE export. English smart tags stay valid (EN in language coverage).

**Honest costs (must not forget):**
1. Thresholds **re-measure** after rebuild (SigLIP scale ≠ CLIP; placeholders forbidden to ship).
2. Tokenizer is **SentencePiece** + SigLIP template (no BOS / EOS / max ~64) — family branch required; not CLIP BPE.
3. Pack may be **FP32 / large**; optional later quantize.
4. Community ONNX → **self-host + pin + sha256** (Google lineage for weights, not Google-official ONNX binary).
5. **768-d** → full reindex; fail closed if skipped.
6. **Does not fix empty grids today** — that remains Track A on bundled CLIP floors.

### 2026-07-22 — BGE demoted on export risk (still valid)
BGE-VL remains **conditional future** until a verified `torch.onnx.export` / official ONNX exists. Demotion is **export risk**, not denial of CN/retrieval quality.

### Product default (PicAiPic)
| Priority | Choice |
|----------|--------|
| **Primary BETA** | **`siglip-base-patch16-256-multilingual`** (community ONNX starting point e.g. `ajaleksa/siglip-base-patch16-256-multilingual-onnx`; Phase 0 must pass Rust `ort`) |
| Alternate pack | EN-only SigLIP if multilingual pack fails load or size is unacceptable |
| Conditional future | BGE-VL-base when ONNX is real |
| Always keep (after B0) | Bundled CLIP **B/16** int8/quant as required offline default |
| Historical | Bundled CLIP B/32 (pre-B0) |
| Active next implement | **B0** before Track B SigLIP |

### Still true for every sideload (do not skip)
- Full-stack vision+text+tokenizer; **no** CLIP-vision + foreign text.
- Clear + rebuild embeds; `app_meta.embedding_model_id` fail closed.
- Thresholds measured via histograms.
- Trial load + sha256-hosted pack.

### Not substitutes for search
EfficientNet, MobileNet, DINOv2-alone, LLaVA (VLM Q&A ≠ library cosine search).

### Install policy
Sideload only; never overwrite bundled CLIP.

### Tokenizer
Never share the wrong family’s tokenizer with another ONNX pack. CLIP JSON must not ride along with SigLIP; multilingual SigLIP must use **its** SentencePiece assets.

## Related
- Cosine floors / file-type filters: `change-ai-search-filters.md`
- Smart-tag prompts: `change-smart-tags.md`
- Large-library search perf: `change-library-perf.md`
- New IPC: `add-tauri-command.md`
