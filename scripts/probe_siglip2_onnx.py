#!/usr/bin/env python3
"""
Phase 0 probe for onnx-community/siglip2-base-patch16-224-ONNX (int8 dual-tower).

Does NOT touch src-tauri/resources/models (bundled CLIP stays B/32).
Downloads into scripts/.probe-models/siglip2-base-patch16-224/ by default.

Usage (from repo root):
  python scripts/probe_siglip2_onnx.py
  python scripts/probe_siglip2_onnx.py --skip-download
  python scripts/probe_siglip2_onnx.py --variant quantized
  set HTTPS_PROXY=http://127.0.0.1:7897   # if HF is blocked

Exit codes:
  0 = load + encode OK (print report)
  1 = hard failure (missing deps, download fail, ORT load/run fail)
  2 = soft fail (loaded but shape/token heuristics look wrong)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DIR = Path(__file__).resolve().parent / ".probe-models" / "siglip2-base-patch16-224"
HF_REPO = "onnx-community/siglip2-base-patch16-224-ONNX"
HF_BASE = f"https://huggingface.co/{HF_REPO}/resolve/main"

# Dual-tower only (not the combined model_*.onnx).
VARIANTS = {
    "int8": {
        "vision": "onnx/vision_model_int8.onnx",
        "text": "onnx/text_model_int8.onnx",
    },
    "quantized": {
        "vision": "onnx/vision_model_quantized.onnx",
        "text": "onnx/text_model_quantized.onnx",
    },
    "fp16": {
        "vision": "onnx/vision_model_fp16.onnx",
        "text": "onnx/text_model_fp16.onnx",
    },
    "fp32": {
        "vision": "onnx/vision_model.onnx",
        "text": "onnx/text_model.onnx",
    },
}

TOKENIZER_FILES = ("tokenizer.json", "tokenizer.model", "tokenizer_config.json")

# SigLIP / SigLIP2 typical preprocess (confirmed on this pack's preprocessor_config.json)
IMAGE_SIZE = 224
IMAGE_MEAN = (0.5, 0.5, 0.5)
IMAGE_STD = (0.5, 0.5, 0.5)

PROBE_TEXTS = [
    ("en_bird", "a photo of a bird"),
    ("en_plant", "a photo of a plant"),
    ("zh_bird", "一只鸟"),
    ("zh_plant", "一株植物"),
    ("zh_empty_risk", "风景"),
]


def die(msg: str, code: int = 1) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(code)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists() and dest.stat().st_size > 0:
        print(f"  skip (exists): {dest.name} ({dest.stat().st_size:,} bytes)")
        return
    print(f"  downloading {url}")
    tmp = dest.with_suffix(dest.suffix + ".partial")
    req = urllib.request.Request(url, headers={"User-Agent": "PicAiPic-Phase0-Probe/1.0"})
    if token := os.environ.get("HF_TOKEN"):
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=600) as resp, tmp.open("wb") as out:
            total = resp.headers.get("Content-Length")
            total_n = int(total) if total and total.isdigit() else None
            n = 0
            while True:
                buf = resp.read(1024 * 1024)
                if not buf:
                    break
                out.write(buf)
                n += len(buf)
                if total_n:
                    pct = 100.0 * n / total_n
                    print(f"\r  {dest.name}: {n:,}/{total_n:,} ({pct:.1f}%)", end="", flush=True)
            print()
    except urllib.error.HTTPError as e:
        if tmp.exists():
            tmp.unlink(missing_ok=True)
        die(f"HTTP {e.code} for {url}")
    except Exception as e:
        if tmp.exists():
            tmp.unlink(missing_ok=True)
        die(f"download failed for {url}: {e}")
    tmp.replace(dest)
    print(f"  saved {dest} ({dest.stat().st_size:,} bytes)")


def ensure_deps() -> None:
    try:
        import numpy  # noqa: F401
        import onnxruntime  # noqa: F401
        from PIL import Image  # noqa: F401
        from tokenizers import Tokenizer  # noqa: F401
    except ImportError as e:
        die(
            f"missing Python dep: {e}\n"
            "  pip install onnxruntime numpy pillow tokenizers"
        )


def describe_session(label: str, sess) -> dict:
    def fmt_io(items):
        out = []
        for i in items:
            shape = []
            for d in i.shape:
                shape.append(str(d) if isinstance(d, int) else (d if d is not None else "?"))
            out.append(
                {
                    "name": i.name,
                    "type": i.type,
                    "shape": shape,
                }
            )
        return out

    info = {
        "label": label,
        "providers": sess.get_providers(),
        "inputs": fmt_io(sess.get_inputs()),
        "outputs": fmt_io(sess.get_outputs()),
    }
    print(f"\n== {label} ==")
    print(f"  providers: {info['providers']}")
    for side, rows in (("inputs", info["inputs"]), ("outputs", info["outputs"])):
        print(f"  {side}:")
        for r in rows:
            print(f"    - {r['name']}: {r['type']} {r['shape']}")
    return info


def l2_normalize(v):
    import numpy as np

    v = np.asarray(v, dtype=np.float32).reshape(-1)
    n = float(np.linalg.norm(v))
    if n <= 0:
        return v, 0.0
    return v / n, n


def cosine(a, b) -> float:
    import numpy as np

    a, _ = l2_normalize(a)
    b, _ = l2_normalize(b)
    return float(np.dot(a, b))


def make_dummy_image_nchw():
    import numpy as np
    from PIL import Image, ImageDraw

    img = Image.new("RGB", (IMAGE_SIZE, IMAGE_SIZE), (30, 120, 40))
    d = ImageDraw.Draw(img)
    # crude "bird-ish" blob for non-constant pixels
    d.ellipse((70, 70, 150, 140), fill=(200, 60, 40))
    d.ellipse((100, 50, 130, 80), fill=(240, 220, 80))
    arr = np.asarray(img).astype("float32") / 255.0
    for c in range(3):
        arr[:, :, c] = (arr[:, :, c] - IMAGE_MEAN[c]) / IMAGE_STD[c]
    # NCHW
    return np.transpose(arr, (2, 0, 1))[None, ...].astype("float32")


def encode_text(sess, tokenizer, text: str) -> tuple:
    import numpy as np

    enc = tokenizer.encode(text, add_special_tokens=True)
    ids = enc.ids
    mask = enc.attention_mask
    print(f"  text[{text!r}] ids_len={len(ids)} first3={ids[:3]} last3={ids[-3:]}")

    feeds = {}
    for inp in sess.get_inputs():
        name = inp.name
        if "input_ids" in name or name == "input_ids":
            feeds[name] = np.asarray([ids], dtype=np.int64)
        elif "attention_mask" in name or name == "attention_mask":
            feeds[name] = np.asarray([mask], dtype=np.int64)
        else:
            # some exports only need input_ids
            print(f"  warn: unknown text input {name}, skipping")

    if not feeds:
        die("no feedable text inputs found")

    outs = sess.run(None, feeds)
    out_names = [o.name for o in sess.get_outputs()]
    # Prefer known embed names
    pick = 0
    for i, n in enumerate(out_names):
        if any(k in n.lower() for k in ("pooler", "embed", "text")):
            pick = i
            break
    emb = outs[pick]
    emb = emb.reshape(-1) if hasattr(emb, "reshape") else emb
    if hasattr(emb, "shape") and len(emb.shape) >= 2 and emb.shape[0] == 1:
        # [1, dim] or [1, seq, dim]
        if len(emb.shape) == 3:
            emb = emb[0, 0, :]  # first token fallback
        else:
            emb = emb[0]
    vec, raw_norm = l2_normalize(emb)
    return vec, raw_norm, out_names[pick], list(emb.shape) if hasattr(emb, "shape") else []


def encode_image(sess, pixel_values):
    import numpy as np

    feeds = {}
    for inp in sess.get_inputs():
        name = inp.name
        if "pixel" in name.lower() or name == "pixel_values":
            feeds[name] = pixel_values
        else:
            print(f"  warn: unknown vision input {name}")

    if not feeds:
        # take first input
        feeds[sess.get_inputs()[0].name] = pixel_values

    outs = sess.run(None, feeds)
    out_names = [o.name for o in sess.get_outputs()]
    pick = 0
    for i, n in enumerate(out_names):
        if any(k in n.lower() for k in ("pooler", "embed", "image", "vision")):
            pick = i
            break
    emb = outs[pick]
    if hasattr(emb, "shape") and len(emb.shape) >= 2 and emb.shape[0] == 1:
        if len(emb.shape) == 3:
            emb = emb[0, 0, :]
        else:
            emb = emb[0]
    vec, raw_norm = l2_normalize(emb)
    return vec, raw_norm, out_names[pick], list(np.asarray(emb).shape)


def main() -> None:
    parser = argparse.ArgumentParser(description="Phase 0 SigLIP2 ONNX probe for PicAiPic")
    parser.add_argument("--dir", type=Path, default=DEFAULT_DIR, help="local pack directory")
    parser.add_argument(
        "--variant",
        choices=sorted(VARIANTS.keys()),
        default="int8",
        help="ONNX quant flavor (default int8)",
    )
    parser.add_argument("--skip-download", action="store_true")
    parser.add_argument(
        "--json-out",
        type=Path,
        default=None,
        help="write machine-readable report JSON",
    )
    args = parser.parse_args()
    ensure_deps()

    import onnxruntime as ort
    from tokenizers import Tokenizer

    pack = args.dir
    pack.mkdir(parents=True, exist_ok=True)
    vpaths = VARIANTS[args.variant]
    vision_rel, text_rel = vpaths["vision"], vpaths["text"]
    vision_path = pack / Path(vision_rel).name
    text_path = pack / Path(text_rel).name
    tok_path = pack / "tokenizer.json"

    print("PicAiPic Phase 0 — SigLIP2 ONNX probe")
    print(f"  repo:    {HF_REPO}")
    print(f"  variant: {args.variant}")
    print(f"  dir:     {pack}")
    print(f"  note:    does NOT modify bundled CLIP under src-tauri/resources/models")

    if not args.skip_download:
        print("\n-- download --")
        download(f"{HF_BASE}/{vision_rel}", vision_path)
        download(f"{HF_BASE}/{text_rel}", text_path)
        for name in TOKENIZER_FILES:
            download(f"{HF_BASE}/{name}", pack / name)
    else:
        print("\n-- skip-download --")

    for p in (vision_path, text_path, tok_path):
        if not p.is_file():
            die(f"missing required file: {p}")

    report: dict = {
        "repo": HF_REPO,
        "variant": args.variant,
        "dir": str(pack),
        "files": {},
        "vision": {},
        "text": {},
        "encodes": {},
        "ok": False,
        "soft_issues": [],
    }

    for label, p in (("vision", vision_path), ("text", text_path), ("tokenizer", tok_path)):
        report["files"][label] = {
            "path": str(p),
            "bytes": p.stat().st_size,
            "sha256": sha256_file(p),
        }
        print(f"  {label}: {p.stat().st_size:,} bytes  sha256={report['files'][label]['sha256'][:16]}…")

    print("\n-- ORT load --")
    so = ort.SessionOptions()
    so.intra_op_num_threads = 2
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    try:
        vision_sess = ort.InferenceSession(str(vision_path), so, providers=["CPUExecutionProvider"])
        text_sess = ort.InferenceSession(str(text_path), so, providers=["CPUExecutionProvider"])
    except Exception as e:
        die(f"ORT Session load failed: {e}")

    report["vision"]["io"] = describe_session("vision", vision_sess)
    report["text"]["io"] = describe_session("text", text_sess)

    print("\n-- tokenizer --")
    try:
        tokenizer = Tokenizer.from_file(str(tok_path))
    except Exception as e:
        die(f"Tokenizer load failed: {e}")

    # Encode probes
    print("\n-- encode image --")
    pixels = make_dummy_image_nchw()
    try:
        img_vec, img_raw_norm, img_out_name, img_shape = encode_image(vision_sess, pixels)
    except Exception as e:
        die(f"vision encode failed: {e}")
    print(
        f"  out={img_out_name} shape={img_shape} dim={img_vec.size} "
        f"raw_l2={img_raw_norm:.4f} unit_l2={float((img_vec**2).sum()**0.5):.4f}"
    )
    report["encodes"]["image"] = {
        "output": img_out_name,
        "dim": int(img_vec.size),
        "raw_l2": img_raw_norm,
        "shape": img_shape,
    }

    print("\n-- encode texts --")
    text_vecs = {}
    for key, text in PROBE_TEXTS:
        try:
            vec, raw_norm, out_name, shape = encode_text(text_sess, tokenizer, text)
        except Exception as e:
            die(f"text encode failed for {key!r}: {e}")
        text_vecs[key] = vec
        print(
            f"  {key}: out={out_name} dim={vec.size} raw_l2={raw_norm:.4f} "
            f"cos_to_image={cosine(img_vec, vec):.4f}"
        )
        report["encodes"][key] = {
            "text": text,
            "output": out_name,
            "dim": int(vec.size),
            "raw_l2": raw_norm,
            "cos_to_image": cosine(img_vec, vec),
            "shape": shape,
        }

    # Heuristics
    soft = report["soft_issues"]
    dims = {k: report["encodes"][k]["dim"] for k in report["encodes"] if k != "image"}
    dims["image"] = report["encodes"]["image"]["dim"]
    if len(set(dims.values())) != 1:
        soft.append(f"embedding dims disagree: {dims}")
    dim = report["encodes"]["image"]["dim"]
    if dim not in (512, 768, 1024, 1152):
        soft.append(f"unusual embedding dim {dim} (not blocking)")

    # CN vs EN bird prompts should not be pure noise relative to each other
    if "en_bird" in text_vecs and "zh_bird" in text_vecs:
        c = cosine(text_vecs["en_bird"], text_vecs["zh_bird"])
        report["encodes"]["en_zh_bird_text_cos"] = c
        print(f"\n  en_bird vs zh_bird text cosine (L2): {c:.4f}")
        if c < 0.05:
            soft.append(
                f"en/zh bird text cosine very low ({c:.4f}) — possible weak multilingual or wrong tokens"
            )

    print("\n-- Phase 0 checklist (manual next) --")
    checks = [
        "ORT CPU load vision+text int8 OK (this script)",
        "Dummy encode image + CN/EN text OK (this script)",
        "Record I/O names + dim into future manifest",
        "Confirm Rust ort also loads (not only Python ORT) before product UI",
        "Self-host + sha256 if shipping download (do not hotlink unpinned HF forever)",
        "Do NOT replace bundled CLIP resources as default",
        "Full library rebuild + remeasure thresholds if activated later",
        "Subjective CN free-text on real album (owner)",
    ]
    for c in checks:
        print(f"  [ ] {c}")

    report["ok"] = True
    report["soft_issues"] = soft
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"\nWrote {args.json_out}")

    print("\n== SUMMARY ==")
    print(f"  variant={args.variant} dim={dim}")
    print(f"  vision_bytes={report['files']['vision']['bytes']:,}")
    print(f"  text_bytes={report['files']['text']['bytes']:,}")
    print(f"  soft_issues={len(soft)}")
    for s in soft:
        print(f"    - {s}")

    if soft:
        print("\nPhase 0: LOADED with soft issues (exit 2). Inspect before product work.")
        sys.exit(2)
    print("\nPhase 0: Python ORT probe PASSED (exit 0). Still need Rust ort confirmation.")
    sys.exit(0)


if __name__ == "__main__":
    main()
