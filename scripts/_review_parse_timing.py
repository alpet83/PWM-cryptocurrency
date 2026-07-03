#!/usr/bin/env python3
"""
Streaming pwmd log timing parser for cluster gate / seal / attest analysis.

Reads one or more pwmd text logs line-by-line (no full-file load).
Extracts key=value fields from lines like:
  [HH:MM:SS.mmm] #LEVEL: message key=value ...

Example (repo root):
  python scripts/_review_parse_timing.py \\
    --files 'logs/2026-06-25/pwmd-cy-proposer-*.log' \\
    --height-from 201960 --height-to 202000 \\
    --event 'cluster|seal|attest|gate' \\
    --out tmp/timing-events.jsonl

  python scripts/_review_parse_timing.py --files logs/2026-06-25/pwmd-cy-proposer-131749.log --summary
"""
from __future__ import annotations

import argparse
import glob
import json
import re
import sys
from pathlib import Path
from typing import Any, Iterator

RE_LINE = re.compile(
    r"^\[(\d{2}:\d{2}:\d{2}\.\d{3})\] #(INFO|WARN|ERROR|DEBUG): (.+)$"
)
RE_KV = re.compile(r"(\w+)=([^\s]+)")

# Leading token before first key= is treated as event_type (may contain spaces/pipes).
EVENT_PREFIXES = (
    "seal_suppressed_by_cluster",
    "seal_suppression_summary",
    "seal skip:",
    "sealed height=",
    "cluster_attest",
    "cluster_gate",
    "cluster_prep",
    "cluster propose",
    "cluster attest",
    "prop_seal_commit",
    "tx_included",
    "tx commit delta:",
    "head_stall",
    "attest_timeout",
    "sync_epoch_to_tip:",
)


def parse_ts_ms(ts: str) -> int:
    h, m, rest = ts.split(":")
    s, ms = rest.split(".")
    return (int(h) * 3600 + int(m) * 60 + int(s)) * 1000 + int(ms)


def split_message(msg: str) -> tuple[str, dict[str, str]]:
    """Return (event_type, kv dict)."""
    event = msg
    kv_start = None
    for i, ch in enumerate(msg):
        if ch == "=" and i > 0 and msg[i - 1] not in " \t":
            j = i - 1
            while j >= 0 and msg[j] not in " \t|":
                j -= 1
            token = msg[j + 1 : i]
            if token.replace("_", "").isalnum():
                kv_start = j + 1
                break
    fields: dict[str, str] = {}
    if kv_start is not None:
        event = msg[:kv_start].strip()
        tail = msg[kv_start:]
        for m in RE_KV.finditer(tail):
            fields[m.group(1)] = m.group(2)
    else:
        # sealed height=NNN style
        m = re.match(r"sealed height=(\d+)", msg)
        if m:
            event = "sealed"
            fields["height"] = m.group(1)
    return event, fields


def iter_events(
    paths: list[str],
    *,
    height_from: int | None,
    height_to: int | None,
    event_pat: re.Pattern[str] | None,
) -> Iterator[dict[str, Any]]:
    for path in paths:
        src = Path(path).name
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            for line_no, raw in enumerate(fh, 1):
                line = raw.rstrip("\n\r")
                m = RE_LINE.match(line)
                if not m:
                    continue
                ts, level, msg = m.group(1), m.group(2), m.group(3)
                event, fields = split_message(msg)
                if event_pat and not event_pat.search(event):
                    continue
                height = None
                if "height" in fields:
                    try:
                        height = int(fields["height"])
                    except ValueError:
                        pass
                if height is not None:
                    if height_from is not None and height < height_from:
                        continue
                    if height_to is not None and height > height_to:
                        continue
                yield {
                    "ts": ts,
                    "ts_ms": parse_ts_ms(ts),
                    "level": level,
                    "event_type": event,
                    "height": height,
                    "fields": fields,
                    "source": src,
                    "line": line_no,
                }


def attest_rtt_from_peer(paths: list[str], height_from: int, height_to: int) -> list[dict[str, Any]]:
    """Pair cluster propose sent / attest accepted from peer proposer logs."""
    propose: dict[tuple[int, int], int] = {}
    rtts: list[dict[str, Any]] = []
    pat = re.compile(r"cluster (propose sent|attest accepted)")
    for ev in iter_events(paths, height_from=height_from, height_to=height_to, event_pat=pat):
        h = ev["height"]
        if h is None:
            continue
        rnd = int(ev["fields"].get("round", "0"))
        key = (h, rnd)
        if "propose sent" in ev["event_type"]:
            propose[key] = ev["ts_ms"]
        elif "attest accepted" in ev["event_type"] and key in propose:
            dt = ev["ts_ms"] - propose[key]
            rtts.append({"height": h, "round": rnd, "rtt_ms": dt, "propose_ts": propose[key], "attest_ts": ev["ts_ms"]})
    return rtts


def percentile(vals: list[float], p: float) -> float:
    if not vals:
        return 0.0
    s = sorted(vals)
    k = (len(s) - 1) * p / 100.0
    f = int(k)
    c = min(f + 1, len(s) - 1)
    if f == c:
        return s[f]
    return s[f] + (s[c] - s[f]) * (k - f)


def print_summary(paths: list[str], height_from: int | None, height_to: int | None) -> None:
    sealed: list[tuple[int, int]] = []
    suppress = 0
    waiting_sync = 0
    seal_skip = 0
    for ev in iter_events(paths, height_from=height_from, height_to=height_to, event_pat=None):
        et = ev["event_type"]
        if et == "sealed":
            h = ev["height"]
            if h is not None:
                sealed.append((h, ev["ts_ms"]))
        elif et == "seal_suppressed_by_cluster":
            suppress += 1
        elif et.startswith("cluster_attest_waiting_sync"):
            waiting_sync += 1
        elif et.startswith("seal skip:"):
            seal_skip += 1
    sealed.sort()
    print("=== seal intervals (logged sealed height=) ===")
    prev = None
    for h, t in sealed:
        if prev:
            print(f"  {prev[0]} -> {h}: {t - prev[1]} ms")
        prev = (h, t)
    print(f"seal_suppressed_by_cluster: {suppress}")
    print(f"cluster_attest_waiting_sync: {waiting_sync}")
    print(f"seal skip evictions: {seal_skip}")
    peer_files = [p for p in paths if "peer" in Path(p).name and "proposer" in Path(p).name]
    if peer_files and height_from and height_to:
        rtts = attest_rtt_from_peer(peer_files, height_from, height_to)
        if rtts:
            ms = [float(r["rtt_ms"]) for r in rtts]
            print("=== attest RTT (propose sent -> attest accepted) ===")
            print(f"  count={len(ms)} p50={percentile(ms, 50):.1f} p95={percentile(ms, 95):.1f} max={max(ms):.1f}")


def main() -> None:
    ap = argparse.ArgumentParser(description="Stream-parse pwmd timing logs to JSONL or summary.")
    ap.add_argument("--files", required=True, help="Glob or comma-separated log paths")
    ap.add_argument("--height-from", type=int, default=None)
    ap.add_argument("--height-to", type=int, default=None)
    ap.add_argument("--event", default=None, help="Regex filter on event_type")
    ap.add_argument("--out", default=None, help="Output JSONL path (default stdout)")
    ap.add_argument("--summary", action="store_true", help="Print aggregate stats instead of JSONL")
    args = ap.parse_args()

    raw = args.files.split(",") if "," in args.files else [args.files]
    paths: list[str] = []
    for item in raw:
        paths.extend(sorted(glob.glob(item)))
    if not paths:
        print(f"No files match: {args.files}", file=sys.stderr)
        sys.exit(1)

    event_pat = re.compile(args.event) if args.event else None

    if args.summary:
        print_summary(paths, args.height_from, args.height_to)
        return

    out_fh = open(args.out, "w", encoding="utf-8") if args.out else sys.stdout
    try:
        for ev in iter_events(paths, height_from=args.height_from, height_to=args.height_to, event_pat=event_pat):
            out_fh.write(json.dumps(ev, ensure_ascii=False) + "\n")
    finally:
        if args.out:
            out_fh.close()


if __name__ == "__main__":
    main()