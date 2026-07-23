---
name: siglip2-phase0-probe
description: Phase 0 pack probe for onnx-community SigLIP2 base patch16-224 ONNX int8 dual-tower. No product default change.
last_updated: 2026-07-23
---

# SigLIP2 ONNX Phase 0 probe (PicAiPic)

## Goal
Verify whether **dual-tower int8** weights from  
`onnx-community/siglip2-base-patch16-224-ONNX`  
load and run under **ONNX Runtime** before any Track B product work.

**Not goals**
- Do **not** replace bundled CLIP B/32 under `src-tauri/resources/models/`.
- Do **not** ship Settings download UI until Phase 0 passes **Python + Rust** ort.
- Do **not** treat “HF has int8” as quality proof — need real-album CN/EN search later.

## Candidate pack

| Field | Value |
|-------|--------|
| HF repo | `onnx-community/siglip2-base-patch16-224-ONNX` |
| Base | `google/siglip2-base-patch16-224` |
| Preferred files | `onnx/vision_model_int8.onnx` (~90 MB), `onnx/text_model_int8.onnx` (~270 MB) |
| Alt | `*_quantized.onnx` (same order of size as int8 on this pack) |
| Avoid for dual-tower path | Combined `onnx/model_*.onnx` unless we later prove I/O matches |
| Tokenizer | `tokenizer.json` + `tokenizer.model` (SentencePiece, vocab ~256k) |
| Image size | **224** |
| Mean / std | **0.5 / 0.5 / 0.5** (not CLIP mean/std) |
| Export intent | transformers.js → **must** trial-load on desktop ORT |

Rough download: **~360 MB** dual-tower int8 + tokenizer.

## Prerequisites
```bash
pip install onnxruntime numpy pillow tokenizers
```

If Hugging Face is blocked (common on this machine):
```bash
# Git Bash / env
export HTTPS_PROXY=http://127.0.0.1:7897
export HTTP_PROXY=http://127.0.0.1:7897
# or set system proxy / HF_TOKEN if needed
```

## Run probe script
From repo root:

```bash
python scripts/probe_siglip2_onnx.py
```

Options:
```bash
python scripts/probe_siglip2_onnx.py --variant int8
python scripts/probe_siglip2_onnx.py --variant quantized
python scripts/probe_siglip2_onnx.py --skip-download   # reuse scripts/.probe-models/...
python scripts/probe_siglip2_onnx.py --json-out docs/guide/siglip2-phase0-report.json
```

Default download dir (gitignored via `scripts/.probe-models/`):
```
scripts/.probe-models/siglip2-base-patch16-224/
  vision_model_int8.onnx
  text_model_int8.onnx
  tokenizer.json
  tokenizer.model
  tokenizer_config.json
```

### Exit codes
| Code | Meaning |
|------|---------|
| 0 | Load + encode OK, no soft issues |
| 1 | Hard fail (deps/download/ORT/run) |
| 2 | Loaded but soft issues (odd dim, very low en/zh text cosine, …) |

## Checklist

### A. Python ORT (this script)
- [ ] Download vision+text int8 + tokenizer succeeds
- [ ] Both sessions open with `CPUExecutionProvider`
- [ ] Record **input names + shapes** (vision: usually `pixel_values` NCHW 1×3×224×224)
- [ ] Record **output names** (pooler / embeds / last_hidden_state?)
- [ ] Image encode → embedding **dim** (expect often 768; write measured)
- [ ] Text encode EN: `a photo of a bird`
- [ ] Text encode ZH: `一只鸟` / `一株植物`
- [ ] Print raw L2 norms (document if graph already normalizes)
- [ ] Cosine image↔text for EN/ZH probes (scale may differ from CLIP)
- [ ] en_bird vs zh_bird **text** cosine not near zero (weak multilingual smoke)

### B. Rust `ort` (required before product)
Python ORT ≠ app ORT. Before Track B engine work:
- [ ] Load same three files with PicAiPic’s Rust `ort` crate (small bin or temporary command)
- [ ] One `encode_image` + one `encode_text` (CN+EN)
- [ ] Note any opset / unsupported op errors

### C. Product policy (always)
- [ ] Bundled CLIP B/32 untouched
- [ ] Future install under app-data pack dir only
- [ ] Full-stack vision+text+tokenizer; never mix with CLIP tower
- [ ] Library embeds rebuild + threshold remeasure after activate
- [ ] Self-host + pin + sha256 for shipped downloads

### D. Owner quality smoke (after A+B pass)
- [ ] Small real album: EN queries (birds/plants/insects/architecture)
- [ ] Same album: CN free-text queries
- [ ] Compare subjectively to CLIP B/32 (expect clearer win than B/16)

## How to interpret results

| Result | Action |
|--------|--------|
| Python fail load | Try `quantized` / `fp16`; if all fail, pack not usable |
| Python OK, Rust fail | Export/opset issue — fix export or pick another pack |
| Load OK, CN text garbage | Prefer multilingual SigLIP pack as primary; keep SigLIP2 as EN-strong alt |
| Load OK, quality win | Promote to Track B Phase 1 engine `family=siglip` (or `siglip2`) |

## Measured result (Python ORT, 2026-07-23)

Host run of `python scripts/probe_siglip2_onnx.py` with `HTTPS_PROXY=http://127.0.0.1:7897`:

| Item | Value |
|------|--------|
| Exit | **0 (PASS)** soft_issues=0 |
| Variant | int8 dual-tower |
| Vision | `vision_model_int8.onnx` **94,553,333** bytes |
| Text | `text_model_int8.onnx` **283,438,275** bytes |
| Tokenizer | `tokenizer.json` ~34 MB + `tokenizer.model` ~4 MB |
| Vision in | `pixel_values` float, dynamic NCHW |
| Vision out | prefer **`pooler_output`** `[batch, 768]`; also `last_hidden_state` |
| Text in | **`input_ids` only** (no `attention_mask` on this export) |
| Text out | prefer **`pooler_output`** `[batch, 768]` |
| Embedding dim | **768** |
| Raw L2 | image ~11.6, text ~24–27 → **not unit-norm in graph**; L2 before cosine required |
| Token length | encode length **64** with pad id **0** at end |
| en_bird vs zh_bird text cosine | **0.9375** (strong multilingual alignment smoke) |
| Dummy image↔text cos | ~0.05 (dummy green blob, not a real retrieval test) |

### Manifest draft (for future Track B)

```json
{
  "id": "siglip2-base-patch16-224-int8",
  "family": "siglip2",
  "embeddingDim": 768,
  "imageSize": 224,
  "mean": [0.5, 0.5, 0.5],
  "std": [0.5, 0.5, 0.5],
  "normalizeEmbeddings": true,
  "files": {
    "vision": "vision_model_int8.onnx",
    "text": "text_model_int8.onnx",
    "tokenizer": "tokenizer.json"
  },
  "textInputs": ["input_ids"],
  "visionInputs": ["pixel_values"],
  "preferredOutputs": {
    "vision": "pooler_output",
    "text": "pooler_output"
  },
  "maxTextLength": 64,
  "padId": 0
}
```

Threshold arrays intentionally **omitted** until measured on real library embeds.

### Still open
- [ ] Rust `ort` load of the same three files
- [ ] Real-album EN/CN subjective search vs CLIP B/32
- [ ] Product sideload + rebuild (only after Rust pass)

## Relation to abandoned B0
B0 (CLIP B/16 default) was abandoned after owner trial (≈ B/32).  
This Phase 0 is **Track B pack verification**, not a default model swap.

## Related
- Pattern: `.mex/patterns/change-image-search-model.md`
- Script: `scripts/probe_siglip2_onnx.py`
- Historical B0 design: `docs/superpowers/specs/2026-07-23-clip-b16-default-bump-design.md`
