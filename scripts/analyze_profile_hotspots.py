#!/usr/bin/env python3
"""Quick Firefox Profiler JSON hotspot sampler for flamegraph review."""
import gzip
import json
import os
import sys
from collections import Counter

PROFILE = os.path.join(os.path.dirname(__file__), "..", "profile.json.gz")


def iter_samples(data):
    threads = data.get("threads", [])
    for th in threads:
        name = th.get("name", "?")
        stack_table = th.get("stackTable", {})
        frame_table = th.get("frameTable", {})
        func_table = th.get("funcTable", {})
        string_table = th.get("stringTable", [])
        samples = th.get("samples", {})
        stack_col = samples.get("stack", [])
        weight_col = samples.get("weight", samples.get("time", []))
        for i, stack_idx in enumerate(stack_col):
            w = weight_col[i] if i < len(weight_col) else 1
            yield name, stack_idx, w, stack_table, frame_table, func_table, string_table


def frame_name(frame_table, func_table, string_table, frame_idx):
    if frame_idx is None or frame_idx < 0:
        return "<root>"
    fr = frame_table.get("location", [])
    fu = frame_table.get("func", [])
    loc_idx = fr[frame_idx] if frame_idx < len(fr) else None
    func_idx = fu[frame_idx] if frame_idx < len(fu) else None
    if func_idx is not None and func_idx < len(func_table.get("name", [])):
        n = func_table["name"][func_idx]
        if n is not None and n < len(string_table):
            return string_table[n]
    if loc_idx is not None and loc_idx < len(string_table):
        return string_table[loc_idx]
    return f"frame#{frame_idx}"


def stack_frames(stack_table, frame_table, func_table, string_table, stack_idx):
    prefixes = stack_table.get("prefix", [])
    frames = stack_table.get("frame", [])
    out = []
    while stack_idx is not None and stack_idx >= 0:
        fi = frames[stack_idx] if stack_idx < len(frames) else None
        out.append(frame_name(frame_table, func_table, string_table, fi))
        stack_idx = prefixes[stack_idx] if stack_idx < len(prefixes) else None
    return out


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else PROFILE
    path = os.path.abspath(path)
    if not os.path.exists(path):
        print(f"missing: {path}")
        return 1
    mtime = os.path.getmtime(path)
    print(f"profile: {path}")
    print(f"mtime: {mtime}")

    with gzip.open(path, "rt", encoding="utf-8") as f:
        data = json.load(f)

    meta = data.get("meta", {})
    print(f"profiler: {meta.get('product', '?')} interval: {meta.get('interval', '?')}")

    counter = Counter()
    thread_weights = Counter()
    keywords = (
        "serde", "json", "ed25519", "validate", "tokio", "seal", "wire",
        "hex", "format", "tracing", "block_timing", "apply_tx", "signature",
    )

    for tname, stack_idx, w, st, ft, fut, strings in iter_samples(data):
        thread_weights[tname] += w
        frames = stack_frames(st, ft, fut, strings, stack_idx)
        leaf = frames[0] if frames else "?"
        counter[leaf] += w
        joined = " ".join(frames).lower()
        for kw in keywords:
            if kw in joined:
                counter[f"kw:{kw}"] += w

    print("\n-- thread weights (top 8) --")
    for t, w in thread_weights.most_common(8):
        print(f"  {w:12.1f}  {t}")

    print("\n-- leaf frames (top 25) --")
    for name, w in counter.most_common(40):
        if name.startswith("kw:"):
            continue
        print(f"  {w:12.1f}  {name}")

    print("\n-- keyword hits (aggregated) --")
    for name, w in sorted(counter.items(), key=lambda x: -x[1]):
        if name.startswith("kw:"):
            print(f"  {w:12.1f}  {name[3:]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())