#!/usr/bin/env python3
"""
Calibrate PicAiPic image-search slider floors from search_similar console logs.

Does NOT change product defaults by itself — prints a recommended
[VeryHigh, High, Medium, Low] table from observed per-query max scores.

Usage (from repo root):
  # Paste log lines, then Ctrl-Z (Windows) / Ctrl-D (Unix):
  python scripts/calibrate_search_thresholds.py

  python scripts/calibrate_search_thresholds.py --log path/to/search.log
  python scripts/calibrate_search_thresholds.py --log search.log --json-out docs/guide/search-threshold-calibration.json

Recognized line shape (host debug print):
  search_similar text_chars=… max=0.2770 >0.18=97 >0.22=50 >0.28=0 …

Exit: 0 ok, 1 no usable lines.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

LINE_RE = re.compile(
    r"search_similar\s+"
    r"(?:.*?)"
    r"max=(?P<max>[0-9.]+)\s+"
    r">0\.18=(?P<b18>\d+)\s+"
    r">0\.22=(?P<b22>\d+)\s+"
    r">0\.28=(?P<b28>\d+)\s+"
    r">0\.34=(?P<b34>\d+)\s+"
    r">0\.40=(?P<b40>\d+)",
    re.IGNORECASE,
)

PREVIEW_RE = re.compile(r"preview=(?P<pre>\"[^\"]*\"|'[^']*')")


def parse_lines(text: str) -> list[dict]:
    rows: list[dict] = []
    for line in text.splitlines():
        if "search_similar" not in line or "max=" not in line:
            continue
        m = LINE_RE.search(line)
        if not m:
            continue
        preview = None
        pm = PREVIEW_RE.search(line)
        if pm:
            preview = pm.group("pre").strip("\"'")
        rows.append(
            {
                "preview": preview,
                "max": float(m.group("max")),
                "gt_018": int(m.group("b18")),
                "gt_022": int(m.group("b22")),
                "gt_028": int(m.group("b28")),
                "gt_034": int(m.group("b34")),
                "gt_040": int(m.group("b40")),
                "raw": line.strip(),
            }
        )
    return rows


def percentile(sorted_vals: list[float], p: float) -> float:
    if not sorted_vals:
        return 0.0
    if len(sorted_vals) == 1:
        return sorted_vals[0]
    k = (len(sorted_vals) - 1) * p
    f = int(k)
    c = min(f + 1, len(sorted_vals) - 1)
    if f == c:
        return sorted_vals[f]
    return sorted_vals[f] + (sorted_vals[c] - sorted_vals[f]) * (k - f)


def suggest_slider(maxes: list[float]) -> dict:
    """
    Map observed query maxes → slider *hints* (settings_thr).

    Host uses: abs_floor = max(0.16, thr * 0.85), then rel floor + Top-K.
    So thr is NOT “require score > thr”. Placing thr near observed max empties UI.

    Product-oriented mapping for CLIP B/32 text→image (owner band ~0.22–0.28 max):
      Low    = 0.16  → floor 0.16 (junk only)
      Medium = 0.20  → floor 0.17 (default)
      High   = 0.24  → floor 0.204
      VH     = min(0.28, max(0.26, p80 rounded)) when strong queries top out ~0.28
    If median max is weak (<0.24), pull the whole ladder down slightly.
    """
    s = sorted(maxes)
    p20, p40, p60, p80 = (
        percentile(s, 0.20),
        percentile(s, 0.40),
        percentile(s, 0.60),
        percentile(s, 0.80),
    )
    med_max = percentile(s, 0.50)
    peak = max(maxes)

    def r(x: float) -> float:
        return round(max(0.14, min(0.32, x)), 2)

    # Anchor ladder; nudge only if library is unusually strong/weak.
    low = 0.16
    med = 0.20
    high = 0.24
    vh = 0.28
    if med_max < 0.24:
        # Weaker library / more concept queries — slightly looser hints.
        med = r(0.18)
        high = r(0.22)
        vh = r(min(0.26, max(0.24, p80)))
    elif peak >= 0.30 and p80 >= 0.28:
        # Stronger scores (future model) — allow slightly stricter VH.
        vh = r(min(0.32, p80))
        high = r(max(0.24, p60 * 0.95))

    # Enforce monotonic VH ≥ H ≥ M ≥ L with 0.02 steps.
    high = max(high, med + 0.02)
    vh = max(vh, high + 0.02)
    low, med, high, vh = r(low), r(med), r(high), r(vh)

    return {
        "very_high": vh,
        "high": high,
        "medium": med,
        "low": low,
        "as_array": [vh, high, med, low],
        "abs_floors": {
            "very_high": round(max(0.16, vh * 0.85), 3),
            "high": round(max(0.16, high * 0.85), 3),
            "medium": round(max(0.16, med * 0.85), 3),
            "low": round(max(0.16, low * 0.85), 3),
        },
        "percentiles": {
            "p20": p20,
            "p40": p40,
            "p50": med_max,
            "p60": p60,
            "p80": p80,
            "peak": peak,
        },
        "method": "product-oriented CLIP band (not thr≈max percentile)",
    }


def main() -> None:
    ap = argparse.ArgumentParser(description="Calibrate search thresholds from search_similar logs")
    ap.add_argument("--log", type=Path, default=None, help="Log file (default: stdin)")
    ap.add_argument("--json-out", type=Path, default=None)
    args = ap.parse_args()

    if args.log is not None:
        text = args.log.read_text(encoding="utf-8", errors="replace")
    else:
        print(
            "Paste search_similar log lines, then EOF (Ctrl-Z Enter on Windows / Ctrl-D Unix)…",
            file=sys.stderr,
        )
        text = sys.stdin.read()

    rows = parse_lines(text)
    if not rows:
        print("ERROR: no search_similar lines with max=/band counts found", file=sys.stderr)
        sys.exit(1)

    maxes = [r["max"] for r in rows]
    suggestion = suggest_slider(maxes)

    print("PicAiPic search threshold calibration (from logs)")
    print(f"  queries_parsed: {len(rows)}")
    print(
        f"  max scores: min={min(maxes):.4f}  mean={sum(maxes)/len(maxes):.4f}  "
        f"max={max(maxes):.4f}"
    )
    print(
        f"  percentiles: p20={suggestion['percentiles']['p20']:.4f} "
        f"p40={suggestion['percentiles']['p40']:.4f} "
        f"p60={suggestion['percentiles']['p60']:.4f} "
        f"p80={suggestion['percentiles']['p80']:.4f}"
    )
    print("\n  Per-query max:")
    for r in sorted(rows, key=lambda x: -x["max"]):
        label = r["preview"] or "?"
        if len(label) > 36:
            label = label[:35] + "…"
        print(
            f"    {r['max']:.4f}  >0.18={r['gt_018']:4d} >0.22={r['gt_022']:4d} "
            f">0.28={r['gt_028']:3d}  {label}"
        )

    arr = suggestion["as_array"]
    fl = suggestion["abs_floors"]
    print("\n== Suggested settings thr [VH, H, M, L] ==")
    print(f"  {arr}")
    print("  → host abs floor max(0.16, thr*0.85):")
    print(
        f"     VH={fl['very_high']}  H={fl['high']}  M={fl['medium']}  L={fl['low']}"
    )
    print("\n  Notes:")
    print("  - Ranking still applies relative floor + Top-K; thr is a hint.")
    print("  - Smart-tag thr usually tracks Medium.")
    print("  - Re-run after large library / model change; do not ship without owner check.")
    print("  - B/32 max often <0.30; VH above ~0.30 empties almost everything.")

    report = {
        "n": len(rows),
        "maxes": maxes,
        "rows": [{k: v for k, v in r.items() if k != "raw"} for r in rows],
        "suggestion": suggestion,
        "current_shipped_reference": {
            "imageSearchThresholds": [0.28, 0.24, 0.20, 0.16],
            "smartTagThreshold": 0.20,
            "note": "Owner-album calibrated 2026-07-23; re-run this script to refresh",
        },
    }
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"\nWrote {args.json_out}")

    sys.exit(0)


if __name__ == "__main__":
    main()
