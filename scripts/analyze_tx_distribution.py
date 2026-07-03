#!/usr/bin/env python3
"""
analyze_tx_distribution.py — parse blockchain epoch files and client JSONL
to assess per-block tx distribution during ramp benchmarks.

Usage:
    python3 scripts/analyze_tx_distribution.py [--epoch-dir DIR] [--epochs E1,E2,...]
"""
import argparse
import collections
import glob
import json
import sys


DEFAULT_EPOCH_DIR = "tmp/state-cy-attester/epochs"


def load_epoch(epoch_dir: str, epoch_num: int) -> list[tuple[int, int]]:
    """Return [(height, tx_count)] for non-empty blocks in epoch file."""
    path = f"{epoch_dir}/block_e{epoch_num}.jsonl"
    result = []
    try:
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                blk = json.loads(line)
                n = len(blk["txs"])
                if n > 0:
                    result.append((blk["hdr"]["height"], n))
    except FileNotFoundError:
        pass
    return result


def load_epoch_range(epoch_dir: str, e_start: int, e_end: int) -> list[tuple[int, int]]:
    blocks = []
    for e in range(e_start, e_end + 1):
        blocks.extend(load_epoch(epoch_dir, e))
    return sorted(blocks)


def discover_epoch_range(epoch_dir: str) -> tuple[int, int]:
    files = glob.glob(f"{epoch_dir}/block_e*.jsonl")
    nums = [int(f.split("_e")[1].split(".")[0]) for f in files]
    return (min(nums), max(nums)) if nums else (0, 0)


def histogram(values: list[int], bucket: int = 10) -> dict[int, int]:
    h: dict[int, int] = collections.defaultdict(int)
    for v in values:
        h[(v // bucket) * bucket] += 1
    return dict(sorted(h.items()))


def render_bar(n: int, max_n: int, width: int = 40) -> str:
    filled = round(n / max_n * width) if max_n else 0
    return "█" * filled


def print_blocks(blocks: list[tuple[int, int]], max_count: int = 64) -> None:
    print(f"{'Height':>8}  {'N_tx':>5}  Bar")
    for h, cnt in blocks:
        bar = render_bar(cnt, max_count)
        print(f"{h:>8}  {cnt:>5}  {bar}")


def print_distribution(title: str, blocks: list[tuple[int, int]]) -> None:
    if not blocks:
        print(f"{title}: (no data)")
        return
    cnts = [c for _, c in blocks]
    hist = histogram(cnts)
    max_freq = max(hist.values())
    print(f"\n{title}")
    print(f"  Blocks: {len(blocks)}, tx: min={min(cnts)} max={max(cnts)} avg={sum(cnts)/len(cnts):.1f}")
    for bucket, freq in hist.items():
        bar = render_bar(freq, max_freq, width=30)
        print(f"  {bucket:>4}-{bucket+9:<4}: {freq:>4}  {bar}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Analyze per-block tx distribution from epoch JSONL files")
    parser.add_argument("--epoch-dir", default=DEFAULT_EPOCH_DIR, help="Path to epochs directory")
    parser.add_argument("--epochs", help="Comma-separated epoch numbers or range (e.g. '366,367,368' or '364-370')")
    parser.add_argument("--height-range", help="Height range to focus on (e.g. '369500-369600')")
    parser.add_argument("--all", action="store_true", help="Analyze all epochs")
    args = parser.parse_args()

    epoch_dir = args.epoch_dir
    e_min, e_max = discover_epoch_range(epoch_dir)
    print(f"Epoch dir: {epoch_dir}")
    print(f"Available epochs: e{e_min} - e{e_max}")

    if args.all:
        epochs = list(range(e_min, e_max + 1))
    elif args.epochs:
        epochs = []
        for part in args.epochs.split(","):
            if "-" in part:
                a, b = part.split("-")
                epochs.extend(range(int(a), int(b) + 1))
            else:
                epochs.append(int(part))
    else:
        # Default: last 10 non-empty epochs
        epochs = []
        for e in range(e_max, e_max - 30, -1):
            if load_epoch(epoch_dir, e):
                epochs.insert(0, e)
                if len(epochs) >= 10:
                    break

    # Load blocks
    all_blocks: list[tuple[int, int]] = []
    for e in sorted(epochs):
        blks = load_epoch(epoch_dir, e)
        if blks:
            all_blocks.extend(blks)

    if not all_blocks:
        print("No non-empty blocks found.")
        sys.exit(0)

    # Optional height filter
    if args.height_range:
        h_lo, h_hi = map(int, args.height_range.split("-"))
        all_blocks = [(h, c) for h, c in all_blocks if h_lo <= h <= h_hi]

    all_blocks.sort()
    print_distribution("Overall tx distribution", all_blocks)

    print(f"\nPer-epoch detail:")
    print(f"{'Epoch':>6}  {'Filled':>7}  {'MinH':>8}  {'MaxH':>8}  {'Min_tx':>7}  {'Max_tx':>7}  {'Avg_tx':>7}")
    for e in sorted(epochs):
        blks = [(h, c) for h, c in all_blocks
                if (e * 1000) < h <= ((e + 1) * 1000)]
        if not blks:
            continue
        heights = [h for h, _ in blks]
        cnts = [c for _, c in blks]
        print(f"  e{e:<4}  {len(blks):>7}  {min(heights):>8}  {max(heights):>8}  "
              f"{min(cnts):>7}  {max(cnts):>7}  {sum(cnts)/len(cnts):>7.1f}")

    print(f"\nBlock-level view:")
    print_blocks(all_blocks, max_count=max(c for _, c in all_blocks))

    # Monotonicity check (within contiguous runs)
    print(f"\nMonotonicity check (consecutive non-empty blocks):")
    drops = 0
    for i in range(1, len(all_blocks)):
        prev_h, prev_c = all_blocks[i - 1]
        curr_h, curr_c = all_blocks[i]
        # Only check consecutive blocks (no gap)
        if curr_h == prev_h + 1 and curr_c < prev_c - 4:  # allow ±4 slack
            drops += 1
            print(f"  Drop at h={curr_h}: {prev_c} -> {curr_c}")
    if drops == 0:
        print("  OK — no significant drops in consecutive filled blocks")
    else:
        print(f"  {drops} drop(s) detected")


if __name__ == "__main__":
    main()
