#!/usr/bin/env python3
"""
PicAiPic offline album compare: bundled CLIP B/32 vs SigLIP2 quantized dual-tower.

Red lines / non-goals (read before changing):
  - Offline only: does NOT download models or talk to the network by default.
  - Does NOT touch product defaults, Tauri/Rust/Vue, or src-tauri/resources/models/.
  - Does NOT modify probe scripts or write into persistent product model dirs.
  - Read-only inputs: src-tauri/resources/models/ (CLIP) and
    scripts/.probe-models/siglip2-base-patch16-224-quant/ (SigLIP2 quantized).
  - Third-party deps: onnxruntime, numpy, pillow, tokenizers only (same as probe).
  - Optional --json-out is for local reports; script never git-adds.

Preprocess notes (deliberate):
  CLIP: RGB → BILINEAR 224 (mirrors product Triangle) → /255 → CLIP mean/std → NCHW.
        Cosine on raw embeds: dot / (||a||·||b||), matching product.
  SigLIP2: RGB → BILINEAR 224 (HF often uses bicubic; BILINEAR here matches Phase 0
           probe and reduces cross-model resize variables) → /255 → (x-0.5)/0.5 → NCHW.
           L2-normalize embeds then dot (raw L2 ≫ 1 on this pack).

Usage (from repo root):
  python scripts/compare_clip_vs_siglip2.py --images path/to/album
  python scripts/compare_clip_vs_siglip2.py --images a.jpg,b.png --queries "a bird,一只鸟"
  python scripts/compare_clip_vs_siglip2.py --images ./album --json-out docs/guide/clip-vs-siglip2-compare-report.json

If SigLIP2 pack is missing:
  python scripts/probe_siglip2_onnx.py --variant quantized
  # or, if already downloaded elsewhere:
  python scripts/probe_siglip2_onnx.py --variant quantized --skip-download

Exit codes:
  0 = OK
  1 = hard error (deps, missing models, empty inputs, encode hard-fail)
  2 = soft issues (some images unreadable, query truncation, etc.)
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parents[1]
CLIP_DIR = REPO_ROOT / "src-tauri" / "resources" / "models"
SIGLIP2_DIR = (
    Path(__file__).resolve().parent
    / ".probe-models"
    / "siglip2-base-patch16-224-quant"
)

CLIP_VISION = "vision_model.onnx"
CLIP_TEXT = "text_model.onnx"
CLIP_TOKENIZER = "tokenizer.json"

SIGLIP2_VISION = "vision_model_quantized.onnx"
SIGLIP2_TEXT = "text_model_quantized.onnx"
SIGLIP2_TOKENIZER = "tokenizer.json"

IMAGE_SIZE = 224
IMAGE_EXTS = {".jpg", ".jpeg", ".png", ".webp", ".bmp", ".gif"}

CLIP_MEAN = (0.48145466, 0.4578275, 0.40821073)
CLIP_STD = (0.26862954, 0.26130258, 0.27577711)
SIGLIP2_MEAN = (0.5, 0.5, 0.5)
SIGLIP2_STD = (0.5, 0.5, 0.5)

CLIP_MAX_LEN = 77
SIGLIP2_MAX_LEN = 64

DEFAULT_QUERIES = [
    "a bird",
    "a plant",
    "insects",
    "architecture",
    "风景",
    "一只鸟",
    "一株植物",
    "建筑",
    "昆虫",
]

DEFAULT_JSON_OUT = REPO_ROOT / "docs" / "guide" / "clip-vs-siglip2-compare-report.json"


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
        die(
            f"missing Python dep: {e}\n"
            "  pip install onnxruntime numpy pillow tokenizers"
        )


def require_files(label: str, paths: Iterable[Path], hint: str) -> None:
    missing = [p for p in paths if not p.is_file()]
    if not missing:
        return
    lines = [f"missing {label} files:"]
    for p in missing:
        lines.append(f"  - {p}")
    lines.append(hint)
    die("\n".join(lines))


def parse_images_arg(raw: str) -> list[Path]:
    """Accept a directory or comma-separated file list."""
    raw = raw.strip()
    if not raw:
        die("--images is required (dir or comma-separated files)")

    # Prefer directory if the whole string is an existing dir (paths may contain commas rarely).
    as_path = Path(raw)
    if as_path.is_dir():
        return collect_images_from_dir(as_path)

    parts = [p.strip() for p in raw.split(",") if p.strip()]
    if len(parts) == 1:
        p = Path(parts[0])
        if p.is_dir():
            return collect_images_from_dir(p)
        if p.is_file():
            return [p]
        die(f"--images path not found: {p}")

    files: list[Path] = []
    for s in parts:
        p = Path(s)
        if p.is_dir():
            files.extend(collect_images_from_dir(p))
        elif p.is_file():
            files.append(p)
        else:
            print(f"  warn: image path not found, skip: {p}", file=sys.stderr)
    return files


def collect_images_from_dir(root: Path) -> list[Path]:
    found: list[Path] = []
    for p in sorted(root.rglob("*")):
        if p.is_file() and p.suffix.lower() in IMAGE_EXTS:
            found.append(p)
    return found


def parse_queries(args) -> list[str]:
    if args.queries_file is not None:
        path = Path(args.queries_file)
        if not path.is_file():
            die(f"--queries-file not found: {path}")
        lines = path.read_text(encoding="utf-8").splitlines()
        qs = [ln.strip() for ln in lines if ln.strip() and not ln.strip().startswith("#")]
        if not qs:
            die(f"--queries-file empty: {path}")
        return qs
    if args.queries is not None:
        qs = [q.strip() for q in args.queries.split(",") if q.strip()]
        if not qs:
            die("--queries produced an empty list")
        return qs
    return list(DEFAULT_QUERIES)


def make_session(onnx_path: Path):
    import onnxruntime as ort

    so = ort.SessionOptions()
    so.intra_op_num_threads = 2
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    try:
        return ort.InferenceSession(
            str(onnx_path), so, providers=["CPUExecutionProvider"]
        )
    except Exception as e:
        die(f"ORT Session load failed for {onnx_path}: {e}")


def pick_output(outs, out_names: list[str], prefer_keys: tuple[str, ...]):
    """Prefer named embeds/pooler; fall back to first output (probe-style)."""
    import numpy as np

    pick = 0
    for i, n in enumerate(out_names):
        low = n.lower()
        if any(k in low for k in prefer_keys):
            pick = i
            break
    emb = outs[pick]
    emb = np.asarray(emb)
    if emb.ndim >= 2 and emb.shape[0] == 1:
        if emb.ndim == 3:
            emb = emb[0, 0, :]
        else:
            emb = emb[0]
    return emb.astype("float32").reshape(-1), out_names[pick]


def l2_normalize(v):
    import numpy as np

    v = np.asarray(v, dtype=np.float32).reshape(-1)
    n = float(np.linalg.norm(v))
    if n <= 0:
        return v, 0.0
    return v / n, n


def cosine_raw(a, b) -> float:
    """CLIP-style: keep raw vectors, cosine = dot/(||a||·||b||)."""
    import numpy as np

    a = np.asarray(a, dtype=np.float32).reshape(-1)
    b = np.asarray(b, dtype=np.float32).reshape(-1)
    na = float(np.linalg.norm(a))
    nb = float(np.linalg.norm(b))
    if na <= 0 or nb <= 0:
        return 0.0
    return float(np.dot(a, b) / (na * nb))


def cosine_unit(a, b) -> float:
    """SigLIP2-style: vectors already L2-normalized → pure dot."""
    import numpy as np

    return float(np.dot(np.asarray(a, dtype=np.float32), np.asarray(b, dtype=np.float32)))


def preprocess_image(path: Path, mean, std):
    import numpy as np
    from PIL import Image

    with Image.open(path) as im:
        im = im.convert("RGB")
        # BILINEAR ≈ product Triangle for CLIP; also used for SigLIP2 here (see module docstring).
        im = im.resize((IMAGE_SIZE, IMAGE_SIZE), Image.BILINEAR)
        arr = np.asarray(im).astype("float32") / 255.0
    for c in range(3):
        arr[:, :, c] = (arr[:, :, c] - mean[c]) / std[c]
    return np.transpose(arr, (2, 0, 1))[None, ...].astype("float32")


def encode_image(sess, pixel_values, prefer_keys: tuple[str, ...], l2: bool):
    feeds = {}
    for inp in sess.get_inputs():
        name = inp.name
        if "pixel" in name.lower() or name == "pixel_values":
            feeds[name] = pixel_values
    if not feeds:
        feeds[sess.get_inputs()[0].name] = pixel_values
    outs = sess.run(None, feeds)
    out_names = [o.name for o in sess.get_outputs()]
    emb, out_name = pick_output(outs, out_names, prefer_keys)
    if l2:
        emb, raw_norm = l2_normalize(emb)
    else:
        raw_norm = float(__import__("numpy").linalg.norm(emb))
    return emb, raw_norm, out_name


def encode_text_clip(sess, tokenizer, text: str):
    import numpy as np

    enc = tokenizer.encode(text, add_special_tokens=True)
    ids = enc.ids
    # With truncation on, ids never exceed max; hitting max_length is a soft signal.
    truncated = len(ids) >= CLIP_MAX_LEN
    feeds = {}
    for inp in sess.get_inputs():
        name = inp.name
        if "input_ids" in name or name == "input_ids":
            feeds[name] = np.asarray([ids], dtype=np.int64)
        # bundled CLIP has no attention_mask
    if not feeds:
        die("CLIP text session has no input_ids")
    outs = sess.run(None, feeds)
    out_names = [o.name for o in sess.get_outputs()]
    emb, out_name = pick_output(
        outs, out_names, ("text_embeds", "pooler", "embed", "text")
    )
    raw_norm = float(__import__("numpy").linalg.norm(emb))
    return emb, raw_norm, out_name, truncated, len(ids)


def encode_text_siglip2(sess, tokenizer, text: str):
    import numpy as np

    # HF SigLIP lowercases; bare tokenizers JSON may not. Chinese unaffected.
    text_lc = text.lower()
    enc = tokenizer.encode(text_lc, add_special_tokens=True)
    ids = enc.ids
    mask = enc.attention_mask
    # Fixed(64) pad → len(ids) always 64; non-pad count at cap ≈ truncated/full.
    non_pad = int(sum(1 for t in ids if t != 0))
    truncated = non_pad >= SIGLIP2_MAX_LEN

    feeds = {}
    for inp in sess.get_inputs():
        name = inp.name
        if "input_ids" in name or name == "input_ids":
            feeds[name] = np.asarray([ids], dtype=np.int64)
        elif "attention_mask" in name or name == "attention_mask":
            feeds[name] = np.asarray([mask], dtype=np.int64)
    if not feeds:
        die("SigLIP2 text session has no feedable inputs")
    outs = sess.run(None, feeds)
    out_names = [o.name for o in sess.get_outputs()]
    emb, out_name = pick_output(
        outs, out_names, ("pooler", "embed", "text")
    )
    emb, raw_norm = l2_normalize(emb)
    return emb, raw_norm, out_name, truncated, non_pad


def top_k(scores: list[tuple[str, float]], k: int) -> list[tuple[str, float]]:
    return sorted(scores, key=lambda x: x[1], reverse=True)[:k]


def jaccard(a: set[str], b: set[str]) -> float:
    if not a and not b:
        return 1.0
    u = a | b
    if not u:
        return 0.0
    return len(a & b) / len(u)


def rel_name(path: Path, roots: list[Path]) -> str:
    for root in roots:
        try:
            return str(path.resolve().relative_to(root.resolve()))
        except ValueError:
            continue
    return path.name


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Offline CLIP B/32 vs SigLIP2 quantized album compare (PicAiPic)"
    )
    parser.add_argument(
        "--images",
        required=True,
        help="Image directory (recursive) or comma-separated image files",
    )
    parser.add_argument(
        "--queries",
        default=None,
        help='Comma-separated queries (default: EN/CN bird/plant/insect/architecture set)',
    )
    parser.add_argument(
        "--queries-file",
        type=Path,
        default=None,
        help="Text file with one query per line (overrides --queries)",
    )
    parser.add_argument("--top-k", type=int, default=5, help="Top-k images per query (default 5)")
    parser.add_argument(
        "--clip-dir",
        type=Path,
        default=CLIP_DIR,
        help="CLIP model dir (default: src-tauri/resources/models)",
    )
    parser.add_argument(
        "--siglip2-dir",
        type=Path,
        default=SIGLIP2_DIR,
        help="SigLIP2 quantized pack dir",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        nargs="?",
        const=DEFAULT_JSON_OUT,
        default=None,
        help=(
            "Write JSON report (optional path; "
            f"flag alone → {DEFAULT_JSON_OUT.as_posix()})"
        ),
    )
    args = parser.parse_args()
    ensure_deps()

    import numpy as np
    from tokenizers import Tokenizer

    if args.top_k < 1:
        die("--top-k must be >= 1")

    clip_dir = args.clip_dir
    sig_dir = args.siglip2_dir
    clip_files = [
        clip_dir / CLIP_VISION,
        clip_dir / CLIP_TEXT,
        clip_dir / CLIP_TOKENIZER,
    ]
    sig_files = [
        sig_dir / SIGLIP2_VISION,
        sig_dir / SIGLIP2_TEXT,
        sig_dir / SIGLIP2_TOKENIZER,
    ]
    require_files(
        "CLIP",
        clip_files,
        "  expected bundled CLIP under src-tauri/resources/models/\n"
        "  (vision_model.onnx, text_model.onnx, tokenizer.json)",
    )
    require_files(
        "SigLIP2 quantized",
        sig_files,
        "  expected scripts/.probe-models/siglip2-base-patch16-224-quant/\n"
        "  fix: python scripts/probe_siglip2_onnx.py --variant quantized\n"
        "  or:  python scripts/probe_siglip2_onnx.py --variant quantized --skip-download\n"
        "  (does not modify product defaults or resources/models)",
    )

    image_paths = parse_images_arg(args.images)
    if not image_paths:
        die("no images found (supported: jpg/jpeg/png/webp/bmp/gif)")

    queries = parse_queries(args)
    top_k_n = args.top_k
    soft_issues: list[str] = []

    print("PicAiPic CLIP B/32 vs SigLIP2 quantized — local album compare")
    print(f"  clip:    {clip_dir} (dim 512, mean={list(CLIP_MEAN)})")
    print(f"  siglip2: {sig_dir} (dim 768, mean={list(SIGLIP2_MEAN)})")
    print(f"  images:  {len(image_paths)}   queries: {len(queries)}")
    print(f"  top-k:   {top_k_n}")
    print(f"  note:    offline read-only; no product default change")

    print("\n-- load sessions --")
    clip_v = make_session(clip_dir / CLIP_VISION)
    clip_t = make_session(clip_dir / CLIP_TEXT)
    sig_v = make_session(sig_dir / SIGLIP2_VISION)
    sig_t = make_session(sig_dir / SIGLIP2_TEXT)

    print("-- load tokenizers --")
    try:
        clip_tok = Tokenizer.from_file(str(clip_dir / CLIP_TOKENIZER))
    except Exception as e:
        die(f"CLIP tokenizer load failed: {e}")
    # JSON has truncation=null; enable CLIP max length (product convention).
    clip_tok.enable_truncation(max_length=CLIP_MAX_LEN)

    try:
        sig_tok = Tokenizer.from_file(str(sig_dir / SIGLIP2_TOKENIZER))
    except Exception as e:
        die(f"SigLIP2 tokenizer load failed: {e}")
    # JSON padding is Fixed(64); re-apply explicitly so truncation+pad are both on.
    sig_tok.enable_truncation(max_length=SIGLIP2_MAX_LEN)
    sig_tok.enable_padding(
        length=SIGLIP2_MAX_LEN, pad_id=0, pad_token="<pad>"
    )

    # Resolve common roots for shorter display names.
    name_roots: list[Path] = []
    as_path = Path(args.images.strip())
    if as_path.is_dir():
        name_roots.append(as_path)
    else:
        parents = {p.resolve().parent for p in image_paths}
        if len(parents) == 1:
            name_roots.append(next(iter(parents)))

    print("\n-- encode images --")
    images: list[dict] = []
    for path in image_paths:
        label = rel_name(path, name_roots)
        try:
            clip_px = preprocess_image(path, CLIP_MEAN, CLIP_STD)
            sig_px = preprocess_image(path, SIGLIP2_MEAN, SIGLIP2_STD)
            clip_emb, clip_l2, clip_out = encode_image(
                clip_v, clip_px, ("image_embeds", "pooler", "embed", "image", "vision"), l2=False
            )
            sig_emb, sig_l2, sig_out = encode_image(
                sig_v, sig_px, ("pooler", "embed", "image", "vision"), l2=True
            )
        except Exception as e:
            msg = f"image read/encode failed: {label}: {e}"
            print(f"  warn: {msg}", file=sys.stderr)
            soft_issues.append(msg)
            continue
        if clip_emb.size != 512:
            soft_issues.append(f"CLIP dim unexpected for {label}: {clip_emb.size}")
        if sig_emb.size != 768:
            soft_issues.append(f"SigLIP2 dim unexpected for {label}: {sig_emb.size}")
        images.append(
            {
                "path": str(path),
                "name": label,
                "clip": clip_emb,
                "siglip2": sig_emb,
                "clip_raw_l2": clip_l2,
                "siglip2_raw_l2": sig_l2,
                "clip_out": clip_out,
                "siglip2_out": sig_out,
            }
        )
        print(
            f"  ok {label}: clip_dim={clip_emb.size} raw_l2={clip_l2:.4f} | "
            f"sig_dim={sig_emb.size} raw_l2_pre_norm={sig_l2:.4f}"
        )

    if not images:
        die("all images failed to load/encode")

    print("\n-- encode queries + score --")
    per_query: list[dict] = []
    truncated_queries = 0
    argmax_agree = 0
    overlaps: list[float] = []

    for q in queries:
        try:
            c_emb, c_l2, c_out, c_trunc, c_len = encode_text_clip(clip_t, clip_tok, q)
            s_emb, s_l2, s_out, s_trunc, s_len = encode_text_siglip2(sig_t, sig_tok, q)
        except Exception as e:
            die(f"text encode failed for {q!r}: {e}")

        if c_trunc or s_trunc:
            truncated_queries += 1
            soft_issues.append(
                f"query truncated: {q!r} (clip_len={c_len}, sig_len={s_len})"
            )

        clip_scores = [
            (im["name"], cosine_raw(im["clip"], c_emb)) for im in images
        ]
        sig_scores = [
            (im["name"], cosine_unit(im["siglip2"], s_emb)) for im in images
        ]
        clip_top = top_k(clip_scores, top_k_n)
        sig_top = top_k(sig_scores, top_k_n)
        clip_set = {n for n, _ in clip_top}
        sig_set = {n for n, _ in sig_top}
        ov = jaccard(clip_set, sig_set)
        overlaps.append(ov)
        agree = clip_top[0][0] == sig_top[0][0]
        if agree:
            argmax_agree += 1

        def fmt_top(rows: list[tuple[str, float]]) -> str:
            return " ".join(f"{n}({s:.3f})" for n, s in rows)

        print(f'\nQuery: "{q}"')
        print(f"  CLIP    top-{top_k_n}: {fmt_top(clip_top)}")
        print(f"  SigLIP2 top-{top_k_n}: {fmt_top(sig_top)}")
        print(
            f"  overlap@{top_k_n}: {ov:.2f}  ;  argmax agree: {'yes' if agree else 'no'}"
        )

        per_query.append(
            {
                "query": q,
                "clip": {
                    "top": [{"name": n, "score": s} for n, s in clip_top],
                    "argmax": clip_top[0][0],
                    "argmax_score": clip_top[0][1],
                    "text_raw_l2": c_l2,
                    "output": c_out,
                    "token_len": c_len,
                    "truncated": c_trunc,
                },
                "siglip2": {
                    "top": [{"name": n, "score": s} for n, s in sig_top],
                    "argmax": sig_top[0][0],
                    "argmax_score": sig_top[0][1],
                    "text_raw_l2": s_l2,
                    "output": s_out,
                    "token_len": s_len,
                    "truncated": s_trunc,
                },
                f"overlap@{top_k_n}": ov,
                "argmax_agree": agree,
            }
        )

    nq = len(queries)
    mean_ov = float(np.mean(overlaps)) if overlaps else 0.0

    print("\n== Side-by-side (top-1) ==")
    print(f"{'query':<16} {'CLIP top-1':<28} {'SigLIP2 top-1':<28} {'J@k':>6} {'agr':>4}")
    for row in per_query:
        q = row["query"]
        if len(q) > 14:
            qdisp = q[:13] + "…"
        else:
            qdisp = q
        c1 = f"{row['clip']['argmax']}({row['clip']['argmax_score']:.3f})"
        s1 = f"{row['siglip2']['argmax']}({row['siglip2']['argmax_score']:.3f})"
        ov = row[f"overlap@{top_k_n}"]
        agr = "yes" if row["argmax_agree"] else "no"
        print(f"{qdisp:<16} {c1:<28} {s1:<28} {ov:6.2f} {agr:>4}")

    print("\n== Summary ==")
    print(f"  images_ok: {len(images)}/{len(image_paths)}")
    print(f"  queries: {nq}  (truncated_queries: {truncated_queries})")
    print(f"  argmax agree: {argmax_agree}/{nq} queries")
    print(f"  mean_overlap@{top_k_n}: {mean_ov:.2f}")
    print(
        "  (subjective; SigLIP scale ≠ CLIP — compare rankings, not absolute scores)"
    )
    if soft_issues:
        print(f"  soft_issues: {len(soft_issues)}")
        for s in soft_issues[:12]:
            print(f"    - {s}")
        if len(soft_issues) > 12:
            print(f"    … +{len(soft_issues) - 12} more")

    report = {
        "ok": True,
        "clip_dir": str(clip_dir),
        "siglip2_dir": str(sig_dir),
        "clip_dim": 512,
        "siglip2_dim": 768,
        "image_size": IMAGE_SIZE,
        "top_k": top_k_n,
        "images_requested": len(image_paths),
        "images_ok": len(images),
        "queries": queries,
        "truncated_queries": truncated_queries,
        "argmax_agree": argmax_agree,
        "argmax_agree_ratio": argmax_agree / nq if nq else 0.0,
        f"mean_overlap@{top_k_n}": mean_ov,
        "per_query": per_query,
        "soft_issues": soft_issues,
        "images": [
            {
                "name": im["name"],
                "path": im["path"],
                "clip_raw_l2": im["clip_raw_l2"],
                "siglip2_raw_l2": im["siglip2_raw_l2"],
                "clip_out": im["clip_out"],
                "siglip2_out": im["siglip2_out"],
            }
            for im in images
        ],
        "note": "SigLIP scores are L2-normalized dots; CLIP scores are raw cosine. Rankings, not absolute scores.",
    }

    if args.json_out is not None:
        out = args.json_out
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
        print(f"\nWrote {out}")
        print("  (not git-added; decide whether to commit the report yourself)")

    if soft_issues:
        print("\nCompare finished with soft issues (exit 2).")
        sys.exit(2)
    print("\nCompare finished OK (exit 0).")
    sys.exit(0)


if __name__ == "__main__":
    main()
