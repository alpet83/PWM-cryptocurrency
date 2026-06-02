#!/usr/bin/env python3
"""
V5 seal log analytics parser — CSV + SVG from pwmd proposer logs.
Example:
  python scripts/_review_seal_log_analytics.py --log "logs/2026-05-30/pwmd-cy-proposer-*.log"

Produces tmp/analytics/seal-<timestamp>/ with CSVs + SVGs.
No external dependencies (pure Python stdlib SVG generation).
Ref: scripts/analyze_seal_suppression_overnight.ps1
"""
import argparse, csv, glob, json, math, re, os, sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# ── log line patterns ──────────────────────────────────────────
RE_BUILD = re.compile(r"\[(\d{2}:\d{2}:\d{2}\.\d{3})\].*build control marker=pwmd/([\d.]+) binary_path=(.+)")
RE_SEALED = re.compile(r"\[(\d{2}:\d{2}:\d{2}\.\d{3})\].*sealed height=(\d+)")
RE_PENDING = re.compile(r"\[(\d{2}:\d{2}:\d{2}\.\d{3})\].*cluster_gate_pending_summary pending_ticks_since_last_sealed=(\d+) sealed_h=(\d+)")
RE_DRIFT = re.compile(r"\[(\d{2}:\d{2}:\d{2}\.\d{3})\].*seal_cadence_drift blocks=(\d+) nominal_ms=(\d+) effective_ms=(\d+) actual_ms=(\d+) expected_ms=(\d+) adjust_pct=([\d.+-]+)(?: envelope_pct=([\d.+-]+))?(?: clamp_applied=(true|false))?")
RE_CHECKPOINT = re.compile(r"\[(\d{2}:\d{2}:\d{2}\.\d{3})\].*autosnapshot checkpoint hit source=(\w+) interval=(\d+) height=(\d+)")
RE_AHEAD = re.compile(r"\[(\d{2}:\d{2}:\d{2}\.\d{3})\].*seal_ahead_summary window_sec=(\d+) ahead_ms=(\d+) fired=(\d+) preflight_skip=(\d+) avg_lead_ms=(\d+)")
RE_SUPPRESS = re.compile(
    r"\[(\d{2}:\d{2}:\d{2}\.\d{3})\].*seal_suppression_summary "
    r"window_sec=(\d+) slots=(\d+) slots_waited_att=(\d+) slots_timeout=(\d+) slots_struck=(\d+) "
    r"suppression_pct=([\d.]+) sealed_in_window=(\d+)(?: last_reason=(\w+))?"
)

def parse_ts(ts_str):
    """Convert HH:MM:SS.mmm to seconds from midnight."""
    parts = ts_str.split(":")
    h, m = int(parts[0]), int(parts[1])
    s_ms = parts[2].split(".")
    s = int(s_ms[0])
    ms = int(s_ms[1]) if len(s_ms) > 1 else 0
    return h * 3600 + m * 60 + s + ms / 1000.0

def read_logs(log_glob):
    """Read all log lines from glob, return sorted list."""
    files = sorted(glob.glob(log_glob, recursive=True))
    if not files:
        print(f"ERROR: No log files match: {log_glob}", file=sys.stderr)
        sys.exit(1)
    print(f"Reading {len(files)} log file(s): {[Path(f).name for f in files]}")
    lines = []
    for path in files:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            for line in f:
                lines.append(line)
    print(f"  Total lines: {len(lines)}")
    return lines

# ── SVG generation (pure python, no matplotlib) ─────────────────

SVG_HEADER = """<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">
<style>
  text {{ font-family: monospace; font-size: 11px; fill: #333; }}
  line {{ stroke: #ccc; stroke-width: 0.5; }}
  .title {{ font-size: 14px; font-weight: bold; fill: #111; }}
  .axis {{ stroke: #333; stroke-width: 1.5; }}
  .grid {{ stroke: #ddd; stroke-width: 0.3; stroke-dasharray: 3,3; }}
  .data {{ fill: none; stroke: #1f77b4; stroke-width: 1.5; }}
  .scatter {{ fill: #1f77b4; opacity: 0.6; }}
  .mean {{ stroke: #d62728; stroke-width: 2; stroke-dasharray: 6,3; }}
  .p95 {{ stroke: #ff7f0e; stroke-width: 1.5; stroke-dasharray: 3,3; }}
  .checkpoint {{ fill: none; stroke: #2ca02c; stroke-width: 1; opacity: 0.3; }}
</style>"""

def svg_chart(xs, ys, title, xlabel, ylabel, w=900, h=400, ymin=None, ymax=None,
              mean=None, p95=None, checkpoints_x=None):
    """Generate simple SVG line chart. xs/ys are float lists."""
    margin = {"top": 40, "right": 30, "bottom": 50, "left": 70}
    pw = w - margin["left"] - margin["right"]
    ph = h - margin["top"] - margin["bottom"]

    if ymin is None: ymin = 0
    if ymax is None: ymax = max(ys) * 1.1 if ys else 100
    if ymax == ymin: ymax = ymin + 1
    xmin, xmax = min(xs) if xs else 0, max(xs) if xs else 1
    if xmax == xmin: xmax = xmin + 1

    def tx(x): return margin["left"] + (x - xmin) / (xmax - xmin) * pw
    def ty(y): return margin["top"] + ph - (y - ymin) / (ymax - ymin) * ph

    parts = [SVG_HEADER.format(w=w, h=h)]
    # title
    parts.append(f'<text x="{w//2}" y="25" text-anchor="middle" class="title">{title}</text>')
    # axis labels
    parts.append(f'<text x="{w//2}" y="{h-8}" text-anchor="middle">{xlabel}</text>')
    parts.append(f'<text x="12" y="{h//2}" text-anchor="middle" transform="rotate(-90,12,{h//2})">{ylabel}</text>')

    # grid
    for i in range(6):
        y = ymin + (ymax - ymin) * i / 5
        parts.append(f'<line x1="{margin["left"]}" y1="{ty(y):.1f}" x2="{w-margin["right"]}" y2="{ty(y):.1f}" class="grid"/>')
        parts.append(f'<text x="{margin["left"]-5}" y="{ty(y)+4:.1f}" text-anchor="end">{y:.0f}</text>')

    # axes
    parts.append(f'<line x1="{margin["left"]}" y1="{ty(ymin)}" x2="{margin["left"]}" y2="{ty(ymax)}" class="axis"/>')
    parts.append(f'<line x1="{margin["left"]}" y1="{ty(ymin)}" x2="{w-margin["right"]}" y2="{ty(ymin)}" class="axis"/>')

    # data line
    if len(xs) > 1:
        points = " ".join(f"{tx(x):.1f},{ty(y):.1f}" for x, y in zip(xs, ys))
        parts.append(f'<polyline points="{points}" class="data"/>')
        # scatter dots
        for x, y in zip(xs, ys):
            parts.append(f'<circle cx="{tx(x):.1f}" cy="{ty(y):.1f}" r="2" class="scatter"/>')

    # checkpoint lines
    if checkpoints_x:
        for cx in checkpoints_x:
            vx = tx(cx) if xmin <= cx <= xmax else None
            if vx is not None:
                parts.append(f'<line x1="{vx:.1f}" y1="{ty(ymin)}" x2="{vx:.1f}" y2="{ty(ymax)}" class="checkpoint"/>')

    # mean line
    if mean is not None:
        parts.append(f'<line x1="{tx(xmin):.1f}" y1="{ty(mean):.1f}" x2="{tx(xmax):.1f}" y2="{ty(mean):.1f}" class="mean"/>')
        parts.append(f'<text x="{w-margin["right"]-5}" y="{ty(mean)-3:.1f}" text-anchor="end" fill="#d62728" font-size="10">μ={mean:.1f}</text>')

    # p95 line
    if p95 is not None:
        parts.append(f'<line x1="{tx(xmin):.1f}" y1="{ty(p95):.1f}" x2="{tx(xmax):.1f}" y2="{ty(p95):.1f}" class="p95"/>')
        parts.append(f'<text x="{w-margin["right"]-5}" y="{ty(p95)-3:.1f}" text-anchor="end" fill="#ff7f0e" font-size="10">p95={p95:.1f}</text>')

    parts.append("</svg>")
    return "\n".join(parts)


def svg_scatter(xs, ys, title, xlabel, ylabel, w=700, h=400):
    """Generate SVG scatter plot."""
    margin = {"top": 40, "right": 30, "bottom": 50, "left": 70}
    pw = w - margin["left"] - margin["right"]
    ph = h - margin["top"] - margin["bottom"]

    ymin, ymax = min(ys) * 0.9, max(ys) * 1.1
    if ymax == ymin: ymax += 1
    xmin, xmax = min(xs), max(xs)
    if xmax == xmin: xmax += 1

    def tx(x): return margin["left"] + (x - xmin) / (xmax - xmin) * pw
    def ty(y): return margin["top"] + ph - (y - ymin) / (ymax - ymin) * ph

    parts = [SVG_HEADER.format(w=w, h=h)]
    parts.append(f'<text x="{w//2}" y="25" text-anchor="middle" class="title">{title}</text>')
    parts.append(f'<text x="{w//2}" y="{h-8}" text-anchor="middle">{xlabel}</text>')
    parts.append(f'<text x="12" y="{h//2}" text-anchor="middle" transform="rotate(-90,12,{h//2})">{ylabel}</text>')

    for i in range(6):
        y = ymin + (ymax - ymin) * i / 5
        parts.append(f'<line x1="{margin["left"]}" y1="{ty(y):.1f}" x2="{w-margin["right"]}" y2="{ty(y):.1f}" class="grid"/>')
        parts.append(f'<text x="{margin["left"]-5}" y="{ty(y)+4:.1f}" text-anchor="end">{y:.0f}</text>')

    parts.append(f'<line x1="{margin["left"]}" y1="{ty(ymin)}" x2="{margin["left"]}" y2="{ty(ymax)}" class="axis"/>')
    parts.append(f'<line x1="{margin["left"]}" y1="{ty(ymin)}" x2="{w-margin["right"]}" y2="{ty(ymin)}" class="axis"/>')

    for x, y in zip(xs, ys):
        parts.append(f'<circle cx="{tx(x):.1f}" cy="{ty(y):.1f}" r="3" class="scatter"/>')

    parts.append("</svg>")
    return "\n".join(parts)


def mean_vals(vals):
    return sum(vals) / len(vals) if vals else 0

def p95_vals(vals):
    if not vals: return 0
    s = sorted(vals)
    return s[int(len(s) * 0.95)]

# ── main ────────────────────────────────────────────────────────

def main():
    p = argparse.ArgumentParser(description="V5 seal log analytics parser")
    p.add_argument("--log", required=True, help="Log file glob pattern")
    p.add_argument("--out-dir", default=None, help="Output directory (default: tmp/analytics/seal-<ts>)")
    args = p.parse_args()

    if args.out_dir:
        out_dir = Path(args.out_dir)
    else:
        ts = datetime.now().strftime("%Y%m%d_%H%M%S")
        out_dir = REPO / "tmp" / "analytics" / f"seal-{ts}"
    out_dir.mkdir(parents=True, exist_ok=True)
    print(f"Output: {out_dir}")

    lines = read_logs(args.log)

    # ── parse all lines ─────────────────────────────────────────
    suppression_rows = []
    sealed_events = []
    pending_rows = []
    drift_rows = []
    checkpoint_rows = []
    ahead_rows = []
    build_markers = []

    last_sealed_h = None

    for line in lines:
        # build marker
        m = RE_BUILD.search(line)
        if m:
            build_markers.append({"ts": parse_ts(m.group(1)), "version": m.group(2), "path": m.group(3).strip()})

        # sealed height
        m = RE_SEALED.search(line)
        if m:
            sealed_events.append({"ts": parse_ts(m.group(1)), "height": int(m.group(2))})
            last_sealed_h = int(m.group(2))

        # pending summary
        m = RE_PENDING.search(line)
        if m:
            pending_rows.append({"ts": parse_ts(m.group(1)), "sealed_h": int(m.group(3)),
                                "pending_ticks": int(m.group(2))})

        # cadence drift
        m = RE_DRIFT.search(line)
        if m:
            eff = int(m.group(3))
            act = int(m.group(5))
            exp = int(m.group(6))
            drift_rows.append({
                "ts": parse_ts(m.group(1)), "blocks": int(m.group(2)),
                "nominal_ms": int(m.group(2)), "effective_ms": eff,
                "actual_ms": act, "expected_ms": exp,
                "adjust_pct": float(m.group(7)),
                "envelope_pct": float(m.group(8)) if m.group(8) else (eff - 1000) / 10.0,
                "ms_per_block_actual": act / int(m.group(2)) if int(m.group(2)) > 0 else 0
            })

        # checkpoint
        m = RE_CHECKPOINT.search(line)
        if m:
            checkpoint_rows.append({"ts": parse_ts(m.group(1)), "height": int(m.group(4)),
                                    "source": m.group(2), "interval": int(m.group(3))})

        # ahead summary
        m = RE_AHEAD.search(line)
        if m:
            ahead_rows.append({"ts": parse_ts(m.group(1)), "ahead_ms": int(m.group(2)),
                              "fired": int(m.group(3)), "preflight_skip": int(m.group(4)),
                              "avg_lead_ms": int(m.group(5))})

        # suppression summary
        m = RE_SUPPRESS.search(line)
        if m:
            h = last_sealed_h or 0
            suppression_rows.append({
                "ts": parse_ts(m.group(1)), "tip_h": h,
                "h_mod_100": h % 100, "h_mod_1000": h % 1000,
                "near_chk_100": (h % 100) >= 95 or (h % 100) <= 5,
                "near_epoch_1000": (h % 1000) >= 995 or (h % 1000) <= 5,
                "slots": int(m.group(3)), "slots_waited_att": int(m.group(4)),
                "slots_timeout": int(m.group(5)), "slots_struck": int(m.group(6)),
                "suppression_pct": float(m.group(7)),
                "sealed_in_window": int(m.group(8)),
                "last_reason": m.group(9) or "",
                "ratio_struck_slots": int(m.group(6)) / int(m.group(3)) if int(m.group(3)) > 0 else 0
            })

    # ── compute inter-seal deltas ────────────────────────────────
    for i in range(len(sealed_events)):
        if i > 0:
            sealed_events[i]["delta_h_prev"] = sealed_events[i]["height"] - sealed_events[i-1]["height"]
            sealed_events[i]["wall_ms_since_prev_seal"] = int((sealed_events[i]["ts"] - sealed_events[i-1]["ts"]) * 1000)
        else:
            sealed_events[i]["delta_h_prev"] = 0
            sealed_events[i]["wall_ms_since_prev_seal"] = 0

    print(f"\nParsed:")
    print(f"  build markers: {len(build_markers)}")
    print(f"  sealed events: {len(sealed_events)}")
    print(f"  pending summaries: {len(pending_rows)}")
    print(f"  cadence drift: {len(drift_rows)}")
    print(f"  checkpoints: {len(checkpoint_rows)}")
    print(f"  ahead summaries: {len(ahead_rows)}")
    print(f"  suppression windows: {len(suppression_rows)}")

    # ── write CSVs ──────────────────────────────────────────────
    def write_csv(name, rows, fields):
        path = out_dir / name
        with open(path, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=fields, extrasaction='ignore')
            w.writeheader()
            for r in rows:
                w.writerow(r)
        print(f"  Wrote {name} ({len(rows)} rows)")

    suppression_fields = ["ts", "tip_h", "h_mod_100", "h_mod_1000", "near_chk_100", "near_epoch_1000",
                         "slots", "slots_waited_att", "slots_timeout", "slots_struck",
                         "suppression_pct", "sealed_in_window", "last_reason", "ratio_struck_slots"]
    write_csv("suppression_windows.csv", suppression_rows, suppression_fields)

    write_csv("sealed_events.csv", sealed_events,
              ["ts", "height", "delta_h_prev", "wall_ms_since_prev_seal"])

    write_csv("pending_summary.csv", pending_rows,
              ["ts", "sealed_h", "pending_ticks"])

    write_csv("cadence_drift.csv", drift_rows,
              ["ts", "blocks", "nominal_ms", "effective_ms", "actual_ms", "expected_ms",
               "adjust_pct", "envelope_pct", "ms_per_block_actual"])

    write_csv("checkpoint_events.csv", checkpoint_rows,
              ["ts", "height", "source", "interval"])

    write_csv("ahead_summary.csv", ahead_rows,
              ["ts", "ahead_ms", "fired", "preflight_skip", "avg_lead_ms"])

    write_csv("build_markers.csv", build_markers, ["ts", "version", "path"])

    # ── generate SVGs ───────────────────────────────────────────
    svg_count = 0

    # 1) suppression_pct_timeline
    if suppression_rows:
        xs = [r["tip_h"] for r in suppression_rows]
        ys = [r["suppression_pct"] for r in suppression_rows]
        mean_s = mean_vals(ys)
        p95_s = p95_vals(ys)
        cp_xs = [r["height"] for r in checkpoint_rows]
        svg = svg_chart(xs, ys, "Suppression % vs tip height",
                        "tip_h", "suppression_pct %", mean=mean_s, p95=p95_s,
                        checkpoints_x=cp_xs)
        (out_dir / "suppression_pct_timeline.svg").write_text(svg, encoding="utf-8")
        svg_count += 1

        # 4) suppression_pct_vs_h_mod_100 (scatter)
        xs2 = [r["h_mod_100"] for r in suppression_rows]
        svg2 = svg_scatter(xs2, ys, "Suppression % vs h mod 100",
                          "h % 100", "suppression_pct %")
        (out_dir / "suppression_pct_vs_h_mod_100.svg").write_text(svg2, encoding="utf-8")
        svg_count += 1

        # 5) slots_struck_vs_sealed_in_window
        xss = [r["sealed_in_window"] for r in suppression_rows]
        yss = [r["slots_struck"] for r in suppression_rows]
        svg3 = svg_scatter(xss, yss, "Slots struck vs sealed in window",
                           "sealed_in_window", "slots_struck")
        (out_dir / "slots_struck_vs_sealed_in_window.svg").write_text(svg3, encoding="utf-8")
        svg_count += 1

    # 2) pending_ticks_per_seal
    if pending_rows:
        xp = [r["sealed_h"] for r in pending_rows]
        yp = [r["pending_ticks"] for r in pending_rows]
        svg = svg_chart(xp, yp, "Pending ticks since last sealed",
                        "sealed_h", "pending_ticks", mean=mean_vals(yp))
        (out_dir / "pending_ticks_per_seal.svg").write_text(svg, encoding="utf-8")
        svg_count += 1

    # 3) actual_ms_per_block_100band
    if drift_rows:
        xd = [i for i in range(len(drift_rows))]
        yd = [r["ms_per_block_actual"] for r in drift_rows]
        svg = svg_chart(xd, yd, "Actual ms/block per 100-block window",
                        "window #", "ms/block", mean=mean_vals(yd), p95=p95_vals(yd),
                        ymin=0)
        (out_dir / "actual_ms_per_block_100band.svg").write_text(svg, encoding="utf-8")
        svg_count += 1

    print(f"\nGenerated {svg_count} SVG chart(s)")

    # ── compute aggregates ──────────────────────────────────────
    print("\n-- Aggregates --")
    if suppression_rows:
        sp = [r["suppression_pct"] for r in suppression_rows]
        print(f"  suppression_pct: mean={mean_vals(sp):.2f} median={sorted(sp)[len(sp)//2]:.2f} p95={p95_vals(sp):.2f}")

        exact_third = [r for r in suppression_rows if r["slots"] > 0 and r["slots_struck"] * 3 == r["slots"]]
        print(f"  windows with struck/slots = 1/3: {len(exact_third)}")

        near = [r for r in suppression_rows if r["near_chk_100"]]
        far = [r for r in suppression_rows if not r["near_chk_100"]]
        if near and far:
            print(f"  avg suppression near h%100 (±5): {mean_vals([r['suppression_pct'] for r in near]):.2f}%")
            print(f"  avg suppression far from h%100: {mean_vals([r['suppression_pct'] for r in far]):.2f}%")

    if pending_rows:
        pt = [r["pending_ticks"] for r in pending_rows]
        cp_heights = {r["height"] for r in checkpoint_rows}
        cp_pending = [r["pending_ticks"] for r in pending_rows if r["sealed_h"] in cp_heights]
        print(f"  avg pending_ticks: {mean_vals(pt):.1f}")
        if cp_pending:
            print(f"  avg pending_ticks at checkpoint heights: {mean_vals(cp_pending):.1f}")

    if sealed_events:
        # T100_est: find time for 100 blocks
        h0 = sealed_events[0]["height"]
        for e in sealed_events:
            if e["height"] >= h0 + 100:
                t100 = e["ts"] - sealed_events[0]["ts"]
                print(f"  T100_est (wall for +100 blocks): {t100:.1f}s")
                break

    # build version
    if build_markers:
        versions = set(b["version"] for b in build_markers)
        print(f"  pwmd versions: {sorted(versions)}")

    print(f"\nReport dir: {out_dir}")
    return 0

if __name__ == "__main__":
    sys.exit(main())