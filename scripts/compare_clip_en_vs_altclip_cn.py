#!/usr/bin/env python3
"""
Track C Phase 0: fixed CLIP B/32 vision; compare EN (CLIP text) vs CN (candidate text).

Does NOT change product defaults. Does NOT overwrite src-tauri/resources/models/.
Does NOT download models (offline). Place candidate text ONNX + tokenizer yourself.

Protocol (different from compare_clip_vs_siglip2.py):
  - Image embeds: ALWAYS bundled CLIP vision (512-d).
  - Left ranking: CLIP text + English queries.
  - Right ranking: candidate (AltCLIP-class) text + Chinese queries.
  - Score: raw cosine (product style) for both.

Usage (repo root):
  python scripts/compare_clip_en_vs_altclip_cn.py --images path/to/album
  python scripts/compare_clip_en_vs_altclip_cn.py --images a.jpg,b.png \\
      --altclip-dir scripts/.probe-models/altclip-m9-text \\
      --json-out docs/guide/clip-vs-altclip-compare-report.json

Exit:
  0 = ran OK (owner still judges quality)
  1 = hard error
  2 = soft issues (dim != 512, some images failed, etc.)
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
CLIP_DIR = REPO_ROOT / "src-tauri" / "resources" / "models"
DEFAULT_ALT_DIR = (
    Path(__file__).resolve().parent / ".probe-models" / "altclip-m9-text"
)

CLIP_VISION = "vision_model.onnx"
CLIP_TEXT = "text_model.onnx"
CLIP_TOKENIZER = "tokenizer.json"

IMAGE_SIZE = 224
IMAGE_EXTS = {".jpg", ".jpeg", ".png", ".webp", ".bmp", ".gif"}
CLIP_MEAN = (0.48145466, 0.4578275, 0.40821073)
CLIP_STD = (0.26862954, 0.26130258, 0.27577711)
CLIP_MAX_LEN = 77
# Candidate max length — many CLIP-aligned towers still use 77; override with --alt-max-len.
DEFAULT_ALT_MAX_LEN = 77
EXPECTED_DIM = 512

# Paired EN/CN queries for rank comparison (same visual intent).
PAIRED_QUERIES = [
    ("a bird", "一只鸟"),
    ("a cat", "一只猫"),
    ("a plant", "植物"),
    ("architecture", "建筑"),
    ("landscape", "风景"),
    ("insects", "昆虫"),
]

DEFAULT_JSON = REPO_ROOT / "docs" / "guide" / "clip-vs-altclip-compare-report.json"


def die(msg: str, code: int = 1) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(code)


def ensure_deps() -> None:
    try:
        import numpy  # noqa: F401
        import onnxruntime  # noqa: F401
        from PIL import Image  # noqa: F401
        from tokenizers import Tokenizer  # noqa: F401
    except ImportError as e:
        die(f"missing Python dep: {e}\n  pip install onnxruntime numpy pillow tokenizers")


def list_images(spec: str) -> list[Path]:
    p = Path(spec)
    if p.is_dir():
        files = sorted(
            x for x in p.rglob("*") if x.is_file() and x.suffix.lower() in IMAGE_EXTS
        )
        return files
    parts = [Path(x.strip()) for x in spec.split(",") if x.strip()]
    out = [x for x in parts if x.is_file()]
    if not out:
        die(f"no images from --images {spec!r}")
    return out


def pick_output(outs, out_names, prefer_keys: tuple[str, ...]):
    import numpy as np

    # Prefer projected sentence/text vectors over token sequences (e.g. 768-d DistilBERT).
    # Match order: exact name → prefer_key as full path segment → avoid "token_*".
    pick = None
    lowers = [n.lower() for n in out_names]

    for want in prefer_keys:
        w = want.lower()
        for i, low in enumerate(lowers):
            if low == w or low.endswith("." + w) or low.endswith("/" + w):
                pick = i
                break
        if pick is not None:
            break

    if pick is None:
        for want in prefer_keys:
            w = want.lower()
            for i, low in enumerate(lowers):
                if "token" in low:
                    continue
                if w in low:
                    pick = i
                    break
            if pick is not None:
                break

    if pick is None:
        for i, arr in enumerate(outs):
            a = np.asarray(arr)
            # Prefer rank-2 [batch, dim] over [batch, seq, dim]
            if a.ndim == 2:
                pick = i
                break
    if pick is None:
        pick = 0

    emb = np.asarray(outs[pick], dtype="float32")
    if emb.ndim >= 2 and emb.shape[0] == 1:
        if emb.ndim == 3:
            emb = emb[0, 0, :]
        else:
            emb = emb[0]
    return emb.astype("float32").reshape(-1), out_names[pick]


def cosine_raw(a, b) -> float:
    import numpy as np

    a = np.asarray(a, dtype=np.float32).reshape(-1)
    b = np.asarray(b, dtype=np.float32).reshape(-1)
    na = float(np.linalg.norm(a))
    nb = float(np.linalg.norm(b))
    if na <= 0 or nb <= 0:
        return 0.0
    return float(np.dot(a, b) / (na * nb))


def preprocess_clip_image(path: Path):
    import numpy as np
    from PIL import Image

    with Image.open(path) as im:
        im = im.convert("RGB")
        im = im.resize((IMAGE_SIZE, IMAGE_SIZE), Image.BILINEAR)
        arr = np.asarray(im).astype("float32") / 255.0
    for c in range(3):
        arr[:, :, c] = (arr[:, :, c] - CLIP_MEAN[c]) / CLIP_STD[c]
    return np.transpose(arr, (2, 0, 1))[None, ...].astype("float32")


def load_session(path: Path):
    import onnxruntime as ort

    if not path.is_file():
        die(f"missing model: {path}")
    so = ort.SessionOptions()
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    return ort.InferenceSession(str(path), so, providers=["CPUExecutionProvider"])


def encode_clip_image(sess, path: Path):
    import numpy as np

    pv = preprocess_clip_image(path)
    feeds = {}
    for inp in sess.get_inputs():
        if "pixel" in inp.name.lower() or inp.name == "pixel_values":
            feeds[inp.name] = pv
    if not feeds:
        feeds[sess.get_inputs()[0].name] = pv
    outs = sess.run(None, feeds)
    names = [o.name for o in sess.get_outputs()]
    emb, oname = pick_output(outs, names, ("image_embeds", "pooler", "embed"))
    return emb, float(np.linalg.norm(emb)), oname


def encode_text_generic(sess, tokenizer, text: str, max_len: int, pad_id: int = 0):
    import numpy as np

    enc = tokenizer.encode(text, add_special_tokens=True)
    ids = list(enc.ids)
    truncated = len(ids) > max_len
    if len(ids) > max_len:
        ids = ids[:max_len]
    # attention_mask before pad if available from tokenizer
    if hasattr(enc, "attention_mask") and enc.attention_mask is not None:
        mask = list(enc.attention_mask)[:max_len]
    else:
        mask = [1] * len(ids)
    while len(ids) < max_len:
        ids.append(pad_id)
        mask.append(0)

    feeds = {}
    for inp in sess.get_inputs():
        name = inp.name
        if "input_ids" in name or name == "input_ids":
            feeds[name] = np.asarray([ids], dtype=np.int64)
        elif "attention_mask" in name or name == "attention_mask":
            feeds[name] = np.asarray([mask], dtype=np.int64)
    if not feeds:
        die("text session has no input_ids")
    outs = sess.run(None, feeds)
    names = [o.name for o in sess.get_outputs()]
    emb, oname = pick_output(
        outs,
        names,
        (
            "sentence_embedding",  # singular (canavar multilingual ONNX)
            "sentence_embeddings",
            "text_embeds",
            "pooler",
            "embed",
            "text",
        ),
    )
    return emb, float(np.linalg.norm(emb)), oname, truncated, len([i for i in ids if i != pad_id])


def rank(images: list[dict], text_emb) -> list[tuple[str, float]]:
    import numpy as np

    text_emb = np.asarray(text_emb, dtype=np.float32).reshape(-1)
    scored = []
    for im in images:
        img = np.asarray(im["emb"], dtype=np.float32).reshape(-1)
        if img.shape[0] != text_emb.shape[0]:
            scored.append((im["name"], float("nan")))
            continue
        scored.append((im["name"], cosine_raw(img, text_emb)))
    scored.sort(key=lambda x: (-1e9 if x[1] != x[1] else -x[1]))  # nan last
    return scored


def topk_names(ranked: list[tuple[str, float]], k: int) -> list[str]:
    return [n for n, _ in ranked[:k]]


def main() -> None:
    ensure_deps()
    from tokenizers import Tokenizer

    ap = argparse.ArgumentParser(description="CLIP vision fixed: EN CLIP-text vs CN alt text")
    ap.add_argument("--images", required=True, help="album dir or comma-separated files")
    ap.add_argument("--clip-dir", type=Path, default=CLIP_DIR)
    ap.add_argument("--altclip-dir", type=Path, default=DEFAULT_ALT_DIR)
    ap.add_argument("--alt-text", default="text_model.onnx", help="filename inside altclip-dir")
    ap.add_argument("--alt-tokenizer", default="tokenizer.json")
    ap.add_argument("--alt-max-len", type=int, default=DEFAULT_ALT_MAX_LEN)
    ap.add_argument("--topk", type=int, default=5)
    ap.add_argument("--json-out", type=Path, default=None)
    args = ap.parse_args()

    soft = 0
    clip_dir: Path = args.clip_dir
    alt_dir: Path = args.altclip_dir

    for p in (
        clip_dir / CLIP_VISION,
        clip_dir / CLIP_TEXT,
        clip_dir / CLIP_TOKENIZER,
        alt_dir / args.alt_text,
        alt_dir / args.alt_tokenizer,
    ):
        if not p.is_file():
            die(f"missing {p}\n  Place AltCLIP-class text ONNX + tokenizer under {alt_dir}")

    print("Track C Phase 0 — CLIP vision fixed; EN CLIP-text vs CN candidate-text")
    print(f"  clip_dir:    {clip_dir}")
    print(f"  altclip_dir: {alt_dir}")
    print(f"  expected_dim: {EXPECTED_DIM}")

    clip_v = load_session(clip_dir / CLIP_VISION)
    clip_t = load_session(clip_dir / CLIP_TEXT)
    alt_t = load_session(alt_dir / args.alt_text)
    clip_tok = Tokenizer.from_file(str(clip_dir / CLIP_TOKENIZER))
    # CLIP JSON often has truncation=null; set max length like product.
    try:
        clip_tok.enable_truncation(max_length=CLIP_MAX_LEN)
    except Exception:
        pass
    alt_tok = Tokenizer.from_file(str(alt_dir / args.alt_tokenizer))
    try:
        alt_tok.enable_truncation(max_length=args.alt_max_len)
    except Exception:
        pass

    paths = list_images(args.images)
    print(f"  images: {len(paths)}")
    images: list[dict] = []
    for path in paths:
        try:
            emb, nrm, oname = encode_clip_image(clip_v, path)
            if emb.shape[0] != EXPECTED_DIM:
                print(f"  WARN dim={emb.shape[0]} for {path.name} (want {EXPECTED_DIM})")
                soft = 2
            images.append(
                {"name": path.name, "path": str(path), "emb": emb, "norm": nrm, "out": oname}
            )
        except Exception as e:
            print(f"  SKIP {path}: {e}")
            soft = 2
    if not images:
        die("no images encoded")

    report = {
        "protocol": "clip_vision_fixed_en_clip_text_vs_cn_alt_text",
        "clip_dir": str(clip_dir),
        "altclip_dir": str(alt_dir),
        "n_images": len(images),
        "pairs": [],
        "notes": [],
    }

    print("\n== paired EN (CLIP text) vs CN (candidate text) ==")
    for en_q, zh_q in PAIRED_QUERIES:
        en_emb, en_n, en_out, en_tr, en_len = encode_text_generic(
            clip_t, clip_tok, en_q if en_q.startswith("a ") else f"a photo of {en_q}", CLIP_MAX_LEN
        )
        # Product leaves CJK unwrapped — pass Chinese as-is.
        zh_emb, zh_n, zh_out, zh_tr, zh_len = encode_text_generic(
            alt_t, alt_tok, zh_q, args.alt_max_len
        )

        if en_emb.shape[0] != EXPECTED_DIM or zh_emb.shape[0] != EXPECTED_DIM:
            print(
                f"  FAIL dim en={en_emb.shape[0]} zh={zh_emb.shape[0]} (need {EXPECTED_DIM})"
            )
            soft = 2
            report["notes"].append(f"dim_mismatch for {en_q!r}/{zh_q!r}")

        en_rank = rank(images, en_emb)
        zh_rank = rank(images, zh_emb)
        k = min(args.topk, len(images))
        en_top = topk_names(en_rank, k)
        zh_top = topk_names(zh_rank, k)
        overlap = len(set(en_top) & set(zh_top))

        print(f"\n  EN {en_q!r}  max={en_rank[0][1]:.4f} top{k}={en_top}")
        print(f"  CN {zh_q!r}  max={zh_rank[0][1]:.4f} top{k}={zh_top}")
        print(f"  top{k} name overlap: {overlap}/{k}")
        print(
            f"    en: dim={en_emb.shape[0]} out={en_out} trunc={en_tr} nonpad≈{en_len} l2={en_n:.3f}"
        )
        print(
            f"    zh: dim={zh_emb.shape[0]} out={zh_out} trunc={zh_tr} nonpad≈{zh_len} l2={zh_n:.3f}"
        )

        report["pairs"].append(
            {
                "en_query": en_q,
                "zh_query": zh_q,
                "en_max": en_rank[0][1],
                "zh_max": zh_rank[0][1],
                "en_top": [{"name": n, "score": s} for n, s in en_rank[:k]],
                "zh_top": [{"name": n, "score": s} for n, s in zh_rank[:k]],
                "topk_overlap": overlap,
            }
        )

    report["notes"].append(
        "Owner judges: CN top-k should contain the same subjects as EN for paired queries; "
        "absolute scores may differ. dim must be 512. Fail Track C if CN ranks look random."
    )

    out_path = args.json_out or DEFAULT_JSON
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\nWrote {out_path}")
    if soft:
        print("SOFT issues present (exit 2) — inspect dims / skips")
        sys.exit(2)
    print("OK (exit 0) — still requires owner quality gate before product UI")


if __name__ == "__main__":
    main()
