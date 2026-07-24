---
name: siglip2-phase0-probe
description: Phase 0 pack probe for onnx-community SigLIP2 base patch16-224 ONNX int8 dual-tower. No product default change.
last_updated: 2026-07-24
---

# SigLIP2 ONNX Phase 0 probe (PicAiPic)

## Goal
Verify whether **dual-tower** weights from  
`onnx-community/siglip2-base-patch16-224-ONNX`  
load and run under **ONNX Runtime** before any Track B product work.

**Phase 0 outcome (2026-07-23):** load OK on **quantized/fp16** (Rust **int8** fail). Real-album subjective: **no clear quality win vs CLIP B/32** (small-bird→insect). **Do not ship product UI on this pack alone.** Default remains CLIP B/32.

**Not goals**
- Do **not** replace bundled CLIP B/32 under `src-tauri/resources/models/`.
- Do **not** ship Settings download UI for this pack without a clearer quality gate (or explicit optional BETA with documented 小主体 limits).
- Do **not** treat “HF has int8” as quality proof — offline compare + owner smoke required (done for ~96-image album).

## Candidate pack

| Field | Value |
|-------|--------|
| HF repo | `onnx-community/siglip2-base-patch16-224-ONNX` |
| Base | `google/siglip2-base-patch16-224` |
| Preferred for Rust ort | `*_quantized.onnx` (~same size band as int8) — **product candidate if quality gate ever passes** |
| Python OK / Rust fail | `onnx/vision_model_int8.onnx` (~90 MB), `onnx/text_model_int8.onnx` (~270 MB) — **do not ship int8 for desktop ort without EP upgrade** |
| Alt | `*_fp16.onnx` (larger; also loads on Rust ort) |
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
Offline compare script is ready (does **not** change product defaults):

```bash
# Requires quantized pack under scripts/.probe-models/siglip2-base-patch16-224-quant/
# (python scripts/probe_siglip2_onnx.py --variant quantized once if missing)
python scripts/compare_clip_vs_siglip2.py --images path/to/small_album
python scripts/compare_clip_vs_siglip2.py --images path/to/album \
  --queries "a bird,一只鸟,风景,建筑" \
  --json-out docs/guide/clip-vs-siglip2-compare-report.json
```

- [x] Compare script landed: `scripts/compare_clip_vs_siglip2.py` (CLIP B/32 bundled vs SigLIP2 quantized; rankings + Jaccard@k)
- [x] Small real album (~96 imgs): script run OK (exit 0); mean_overlap@5 ≈ 0.11, argmax agree 0/9 — rankings diverge (expected across families)
- [x] CN free-text included in same run (`风景`/`一只鸟`/`一株植物`/`建筑`/`昆虫`)
- [~] Owner subjective (2026-07-23): **CLIP 昆虫、植物不准；其余类别主观可接受**。**SigLIP2：** 图里鸟很小时也会被当成昆虫（小主体 / 远景细粒度混淆）——与 CLIP 的 植物/昆虫 短板叠加后，**未形成明确质量胜出**；不宜仅凭此包上产品 UI

## How to interpret results

| Result | Action |
|--------|--------|
| Python fail load | Try `quantized` / `fp16`; if all fail, pack not usable |
| Python OK, Rust fail | Export/opset issue — fix export or pick another pack |
| Load OK, CN text garbage | Prefer multilingual SigLIP pack as primary; keep SigLIP2 as EN-strong alt |
| Load OK, quality win | Promote to Track B Phase 1 engine `family=siglip` (or `siglip2`) |

## Measured results (2026-07-23)

### Python ORT (`scripts/probe_siglip2_onnx.py`)
| Variant | Load+encode | Notes |
|---------|-------------|--------|
| **int8** | **PASS** | dim 768; en/zh bird text cos **0.9375** |
| **quantized** | **PASS** | same I/O; sizes match int8 band (~90+270 MB) |
| **fp16** | **PASS** | larger (~186+565 MB) |

### Rust `ort` (same crate as PicAiPic: `scripts/probe_siglip2_ort`)
| Variant | Result |
|---------|--------|
| **int8** (`vision_model_int8` / `text_model_int8`) | **FAIL** — `Could not find an implementation for ConvInteger(10)` on patch embed |
| **quantized** | **PASS** — load + encode; dim 768; en/zh bird text cos **0.9375** |
| **fp16** | **PASS** — load + encode; dim 768; en/zh bird text cos **~0.89** |

**Product implication:** Prefer **`*_quantized.onnx` dual-tower** (or fp16 if quality needs it). Do **not** ship the `*_int8.onnx` files from this pack for Rust ort without a newer EP / different export.

### Shared I/O (quantized / fp16 / Python int8)
| Item | Value |
|------|--------|
| Vision in | `pixel_values` float NCHW |
| Vision out | **`pooler_output`** → 768-d |
| Text in | **`input_ids` only** |
| Text out | **`pooler_output`** → 768-d |
| Image size / mean / std | 224 / 0.5 / 0.5 |
| Text length | 64, pad id 0 |
| Normalize | **must L2 in app** (raw L2 ≫ 1) |

### Manifest draft (Track B candidate — quantized)

```json
{
  "id": "siglip2-base-patch16-224-quantized",
  "family": "siglip2",
  "embeddingDim": 768,
  "imageSize": 224,
  "mean": [0.5, 0.5, 0.5],
  "std": [0.5, 0.5, 0.5],
  "normalizeEmbeddings": true,
  "files": {
    "vision": "vision_model_quantized.onnx",
    "text": "text_model_quantized.onnx",
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
- [x] Python ORT probe
- [x] Rust `ort` probe (**quantized / fp16** pass; **int8** fail)
- [x] Offline album compare script + real ~96-image run
- [x] Real-album subjective (owner): CLIP **insects/plants weak**; other OK. SigLIP2 **small-bird → insect** confusion. **No clear quality win → do not promote product UI on this pack alone**
- [ ] Product sideload + rebuild (blocked until a pack shows clearer win on 植物/昆虫/小主体, or accept optional BETA with honest limits)

### Rust probe commands
```bash
# after python download into scripts/.probe-models/...
cargo run --manifest-path scripts/probe_siglip2_ort/Cargo.toml --release -- \
  --dir scripts/.probe-models/siglip2-base-patch16-224-quant --variant quantized

cargo run --manifest-path scripts/probe_siglip2_ort/Cargo.toml --release -- \
  --dir scripts/.probe-models/siglip2-base-patch16-224-fp16 --variant fp16
```

## Relation to abandoned B0
B0 (CLIP B/16 default) was abandoned after owner trial (≈ B/32).  
This Phase 0 is **Track B pack verification**, not a default model swap.

## Related
- Pattern: `.mex/patterns/change-image-search-model.md`
- Script: `scripts/probe_siglip2_onnx.py`
- Offline compare: `scripts/compare_clip_vs_siglip2.py`
- Historical B0 design: `docs/superpowers/specs/2026-07-23-clip-b16-default-bump-design.md`
