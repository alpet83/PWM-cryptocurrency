#!/usr/bin/env python3
"""Join transfer-ramp client JSONL with block_timing JSONL; emit enriched markdown."""
from __future__ import annotations

import argparse
import json
import statistics
import sys
from pathlib import Path


def percentile(values: list[float], pct: float) -> float | None:
    if not values:
        return None
    values = sorted(values)
    k = (len(values) - 1) * (pct / 100.0)
    f = int(k)
    c = min(f + 1, len(values) - 1)
    if f == c:
        return values[f]
    return values[f] + (values[c] - values[f]) * (k - f)


def load_timing_by_height(path: Path) -> dict[int, dict]:
    out: dict[int, dict] = {}
    if not path.is_file():
        return out
    with path.open(encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            h = int(row.get("height", -1))
            if h >= 0:
                out[h] = row
    return out


def load_client_rows(path: Path) -> list[dict]:
    rows: list[dict] = []
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--client-jsonl", required=True, type=Path)
    parser.add_argument("--block-timing", default=Path("tmp/cy-lab-block-timing.jsonl"), type=Path)
    parser.add_argument("--out", type=Path, default=None)
    args = parser.parse_args()

    client = load_client_rows(args.client_jsonl)
    timing = load_timing_by_height(args.block_timing)
    if not client:
        print("empty client jsonl", file=sys.stderr)
        return 1

    heights = sorted({int(r["batch_height"]) for r in client if "batch_height" in r})
    h_min, h_max = heights[0], heights[-1]

    by_level: dict[int, list[dict]] = {}
    for row in client:
        by_level.setdefault(int(row.get("level", 0)), []).append(row)

    slip_by_level: dict[int, list[float]] = {}
    pending_by_level: dict[int, list[int]] = {}
    for h in heights:
        t = timing.get(h)
        if not t:
            continue
        level_rows = [r for r in client if int(r.get("batch_height", -1)) == h]
        if not level_rows:
            continue
        level = int(level_rows[0].get("level", 0))
        if t.get("seal_slip_ms") is not None:
            slip_by_level.setdefault(level, []).append(float(t["seal_slip_ms"]))
        if t.get("pending_ticks_at_seal") is not None:
            pending_by_level.setdefault(level, []).append(int(t["pending_ticks_at_seal"]))

    lines: list[str] = []
    lines.append("# Transfer-ramp analysis (block_timing join)")
    lines.append("")
    lines.append(f"- client: {args.client_jsonl}")
    lines.append(f"- block_timing: {args.block_timing}")
    lines.append(f"- height_range: {h_min}..{h_max}")
    lines.append(f"- client_rows: {len(client)}")
    lines.append(f"- ok: {sum(1 for r in client if r.get('ok'))}")
    lines.append(f"- fail: {sum(1 for r in client if not r.get('ok'))}")
    lines.append("")

    lat_ok = [float(r["rpc_latency_ms"]) for r in client if r.get("ok") and r.get("rpc_latency_ms") is not None]
    if lat_ok:
        lines.append("## Client RPC latency (ok submits)")
        lines.append("")
        lines.append(f"- p50_ms: {percentile(lat_ok, 50):.2f}")
        lines.append(f"- p95_ms: {percentile(lat_ok, 95):.2f}")
        lines.append(f"- max_ms: {max(lat_ok):.2f}")
        lines.append("")

    lines.append("## By ramp level")
    lines.append("")
    lines.append("| level | txs | ok | fail | rpc_p50_ms | seal_slip_p50 | seal_slip_p95 | pending_p95 |")
    lines.append("|-------|-----|----|------|------------|---------------|---------------|-------------|")
    for level in sorted(by_level):
        rows = by_level[level]
        ok = sum(1 for r in rows if r.get("ok"))
        fail = len(rows) - ok
        lats = [float(r["rpc_latency_ms"]) for r in rows if r.get("rpc_latency_ms") is not None]
        slips = slip_by_level.get(level, [])
        pend = pending_by_level.get(level, [])
        lines.append(
            f"| {level} | {len(rows)} | {ok} | {fail} | "
            f"{percentile(lats, 50) or ''} | {percentile(slips, 50) or ''} | "
            f"{percentile(slips, 95) or ''} | {percentile([float(x) for x in pend], 95) or ''} |"
        )

    if heights and timing:
        nominal = [float(timing[h].get("nominal_seal_ms", 1000)) for h in heights if h in timing]
        slips = [float(timing[h]["seal_slip_ms"]) for h in heights if h in timing and timing[h].get("seal_slip_ms") is not None]
        if len(heights) >= 2 and slips:
            span_blocks = max(1, h_max - h_min)
            lines.append("")
            lines.append("## Cluster cadence (window)")
            lines.append("")
            lines.append(f"- blocks_observed: {span_blocks + 1}")
            lines.append(f"- seal_slip_p50_ms: {percentile(slips, 50):.2f}")
            lines.append(f"- seal_slip_p95_ms: {percentile(slips, 95):.2f}")
            if nominal:
                lines.append(f"- nominal_seal_ms: {statistics.median(nominal):.0f}")
                bps = (span_blocks * 1000.0) / max(
                    1.0,
                    sum(
                        float(timing[heights[i + 1]].get("t0_ms", 0)) - float(timing[heights[i]].get("t0_ms", 0))
                        for i in range(len(heights) - 1)
                        if heights[i] in timing and heights[i + 1] in timing
                    ),
                )
                lines.append(f"- blocks_per_sec_est: {bps:.3f}")

    out = args.out or args.client_jsonl.with_suffix(".analysis.md")
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
