---
name: altclip-phase0-probe
description: Track C bilingual text (CLIP-aligned, no reindex). Phase 0 probe + product C default (bundled int8 EN+CN text).
last_updated: 2026-07-24
---

# Track C — CLIP-aligned bilingual text (no reindex)

## Goal

Prove whether a **text tower** (candidate: canavar / AltCLIP-class CLIP-B/32-aligned) can drive **Chinese free-text search** against **existing** PicAiPic library embeds written by **bundled CLIP ViT-B/32 vision**, **without re-embedding the library** — then ship it as the product default text tower.

**Not goals**
- Do **not** replace `src-tauri/resources/models/vision_model.onnx`.
- Do **not** re-enable the legacy **sentence-embedding** pack under `{app_data}/models/multilingual/` (space mismatch).
- Do **not** confuse this with SigLIP2 dual-tower (different vision space, dim often 768, **requires reindex**).

**Product status (2026-07-24 option C):** Phase 0 **PASS** and **shipped** as bundled `text_model.onnx` (int8). See decision log at bottom.

## Why this can skip reindex

| Piece | Stays fixed | May change |
|-------|-------------|------------|
| `afiles.embeds` | CLIP B/32 **vision** 512-d | — |
| Image encode path | CLIP preprocess 224 + CLIP mean/std | — |
| Text encode | Default CLIP text (EN) | Aligned bilingual text (CN+EN) |

If the candidate text tower is **aligned to the same OpenAI CLIP B/32 image space**, Chinese queries live in the same cosine space as stored image vectors.

## Red lines (fail closed)

1. Output **dim must be 512** (match product CLIP vision).
2. **Same CLIP vision** for all image features in the probe (resources pack only).
3. Smoke: EN CLIP-text ranking vs CN candidate-text ranking must both put **relevant** images in top ranks on the same album.
4. Legacy multilingual sentence pack is a **negative control** (should look worse / random), not a candidate.
5. Download-complete ≠ usable. Product activation later must re-run trial+smoke.

## Prerequisites

```bash
pip install onnxruntime numpy pillow tokenizers
```

Place candidate text tower + tokenizer under (example):

```
scripts/.probe-models/altclip-m9-text/
  text_model.onnx      # or whatever filename; pass --text
  tokenizer.json
```

**You must supply a real CLIP-aligned ONNX export.** There is no guaranteed public dual-file pack identical to product ORT; Phase 0 is exactly to validate *your* export.

Bundled CLIP (read-only):

```
src-tauri/resources/models/
  vision_model.onnx
  text_model.onnx
  tokenizer.json
```

## Scripts

| Script | Role |
|--------|------|
| `scripts/compare_clip_en_vs_altclip_cn.py` | Album: fixed CLIP vision; EN via CLIP text vs CN via candidate text; rank overlap report |
| `scripts/probe_altclip_ort/` | Rust `ort` load: CLIP vision (resources) + candidate text (pack dir); dim/cosine smoke |

### Python album compare

```bash
# From repo root — candidate pack must exist
python scripts/compare_clip_en_vs_altclip_cn.py --images path/to/album
python scripts/compare_clip_en_vs_altclip_cn.py --images ./album --altclip-dir scripts/.probe-models/altclip-m9-text
python scripts/compare_clip_en_vs_altclip_cn.py --images ./album --json-out docs/guide/clip-vs-altclip-compare-report.json
```

### Rust ort smoke

```bash
cargo run --manifest-path scripts/probe_altclip_ort/Cargo.toml --release -- \
  --text-dir scripts/.probe-models/altclip-m9-text
```

## Pass / fail criteria (owner album)

Use ≥30 images with clear subjects (bird/cat/plant/building/people if available).

| Check | Pass |
|-------|------|
| Candidate text load on **Python ORT** and **Rust ort** | Both OK |
| Candidate text embed dim | **512** |
| CLIP vision dim | **512** |
| EN: CLIP text vs image | Strong queries (e.g. bird) max cosine in roughly product band (~0.18–0.30 raw cosine); relevant image in top ranks |
| CN: candidate text vs **same** image embeds | Relevant images in top-k for 「鸟」「猫」「风景」等; not random |
| EN regression | With candidate text on English queries, ranking not collapsed vs CLIP text (allow mild shift) |
| Negative control (optional) | Old app-data multilingual sentence ONNX ranks poorly on CN vs candidate |

**Fail → stop Track C** for that pack. Options: another aligned export, Track B full-stack + reindex, or English-only.

## After pass (implementation order — not Phase 0)

1. **C0** Host pack + `manifest.json` (`alignsTo=clip-b32`, `embeddingDim=512`, sha256).  
2. **C1** `ImageSearchTextModel::AlignedCn` (name TBD); still reject legacy `Multilingual`.  
3. **C2** Download → trial_load + smoke → only then activate.  
4. **C3** Settings: honest “中英文本增强 · 不重建索引”.  
5. **C4** Re-calibrate thresholds if CN score scale drifts.  
6. **C5** Optional `app_meta` vision_id/text_id.

## Relation to prior work

- SigLIP2 Phase 0: `docs/guide/siglip2-phase0-probe.md` — **different problem** (new vision space).  
- Stop-bleed: legacy multilingual **disabled** — do not re-enable that pack.  
- Product search path (matrix/ANN) stays 512 cosine; no space change if Phase 0 passes.

## Decision log

### 2026-07-24 — Track C Phase 0 opened
**Decision:** Pursue CLIP-B/32-aligned bilingual **text-only** sideload as the no-reindex Chinese path; Phase 0 must pass before any product UI.  
**Reasoning:** Library embeds are CLIP vision; AltCLIP-class text is the only low-cost path that can preserve them. SigLIP2-style full-stack always reindexes.  
**Consequences:** Probe scripts fork SigLIP infrastructure but **fix vision** and expect **dim 512**.

### 2026-07-24 — Candidate pack measured: `canavar/clip-ViT-B-32-multilingual-v1-ONNX`
**Local path:** `scripts/.probe-models/clip-vit-b32-multilingual-v1-text/` (`model.onnx` → `text_model.onnx`, ~516 MB).  
**sha256 (model.onnx):** `7f3129e76a60a33aa42941a369d54266151f9ca2e7e4d52295300edb85c5bbeb`

| Check | Result |
|-------|--------|
| Rust ort load | PASS |
| I/O | `input_ids` + `attention_mask` → `token_embeddings` [B,S,768] + **`sentence_embedding` [B,512]** |
| Must use | **`sentence_embedding` only** (not token_embeddings) |
| CLIP vision dim | 512 |
| Candidate sentence dim | **512** |
| Album compare (43 imgs, backend/results) | EN CLIP-text vs CN candidate top-5 overlap: bird 4/5, cat 4/5, plant **5/5**, arch 3/5, landscape 3/5, insects **5/5** |
| CN max cosine (raw) | ~0.24–0.28 (same band as EN) |
| Report | `docs/guide/clip-vs-multilingual-v1-compare-report.json` |

**Verdict:** **Phase 0 technical + rank-alignment gate PASS on this pack** (synthetic + 43-image set). Still do an owner smoke on a real personal photo library before C3 Settings UI. Product must bind output name `sentence_embedding` and require attention_mask.

### 2026-07-24 — Dynamic int8 + product C1–C3
| Item | Result |
|------|--------|
| Dynamic QInt8 | `…-text-int8/text_model.onnx` ~**136 MB** (ratio **0.25**) |
| Rust ort int8 smoke | **PASS** (sentence_embedding 512) |
| fp32 vs int8 retrieval | top1 **6/6** same; top5 overlap avg **4.67/5**; emb cos **~0.998** |
| Product | `Multilingual` re-enabled for **CLIP-aligned** pack only; `encode_text` prefers `sentence_embedding`; Settings bilingual option + download; search activates model 0/1; smoke EN+ZH on activate |
| Install dirs | `{app_data}/models/image-search/clip-vit-b32-multilingual-v1-text[-int8]/` (int8 preferred when both exist) |
| **Self-hosted download** | `big2cater/picaipic-binaries` release tag **`models`**: `clip-vit-b32-multilingual-v1-text-int8.onnx` (~129.5 MiB) + tokenizer + manifest. App download URLs point here; SHA-256 verified before install. |
| **Product default (C, 2026-07-24)** | Installer **ships** bilingual int8 as `resources/models/text_model.onnx` + tokenizer (EN CLIP text removed from bundle). No Settings model switch. Optional “re-download cloud pack” kept for observation. Legacy EN CLIP text URLs kept commented in `download_models.*` + backup under `scripts/.probe-models/bundled-clip-en-text-backup/`. Vision remains CLIP B/32. |

### 2026-07-24 — Similar-from-file ranking (related, not Track C model)
Settings “图片相似度” Low≈VH was an **image→image score-band** bug (text floors applied to similar-from-file). Fixed separately: image floors 0.88/0.82/0.74/0.62, thr_cap 12/24/40/100, exclude self. See `change-ai-search-filters.md` / progress docs.
