#!/usr/bin/env python3
"""Build a short human-readable codebase index from CQDS get_index payload.

Usage:
  python scripts/cqds_index_digest.py --input index.json --output docs/reviews/pwm-codebase-index-YYYYMMDD.md
  cat index.json | python scripts/cqds_index_digest.py --output out.md

The script is intentionally conservative: it deduplicates noisy paths and focuses
on crates/*/src entities for quick navigation.
"""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path
from typing import Any


def _read_payload(input_path: str | None) -> dict[str, Any]:
    if input_path:
        return json.loads(Path(input_path).read_text(encoding="utf-8"))
    import sys

    return json.loads(sys.stdin.read())


def _norm_path(p: str) -> str:
    p = p.replace("\\", "/")
    m = "PWM-cryptocurrency/"
    i = p.find(m)
    if i >= 0:
        p = p[i + len(m) :]
    return p


def _crate_from_path(p: str) -> str | None:
    p = _norm_path(p)
    if not p.startswith("crates/"):
        return None
    parts = p.split("/")
    if len(parts) < 3:
        return None
    return parts[1]


def _pick_entity_name(e: dict[str, Any]) -> str:
    for key in ("qualified_name", "name", "symbol", "id"):
        v = e.get(key)
        if isinstance(v, str) and v.strip():
            return v.strip()
    return "<unnamed>"


def _extract_entities(payload: dict[str, Any]) -> list[dict[str, Any]]:
    for key in ("entities", "index", "items"):
        v = payload.get(key)
        if isinstance(v, list):
            return [x for x in v if isinstance(x, dict)]
    return []


def _extract_file_path(e: dict[str, Any]) -> str:
    for key in ("file_path", "path", "file", "source"):
        v = e.get(key)
        if isinstance(v, str) and v.strip():
            return _norm_path(v)
    return ""


def build_markdown(payload: dict[str, Any]) -> str:
    entities = _extract_entities(payload)
    crate_entities: dict[str, set[str]] = defaultdict(set)
    crate_files: dict[str, set[str]] = defaultdict(set)

    for e in entities:
        p = _extract_file_path(e)
        c = _crate_from_path(p)
        if not c:
            continue
        if p:
            crate_files[c].add(p)
        crate_entities[c].add(_pick_entity_name(e))

    crates = sorted(crate_files.keys())
    lines: list[str] = []
    lines.append("# PWM Codebase Index")
    lines.append("")
    lines.append("## Source")
    lines.append("")
    lines.append("- Built from CQDS cached index payload (`cq_files_ctl#get_index`, `project_id=5`).")
    lines.append(
        f"- Entities seen: `{len(entities)}` (raw); filtered to `crates/*/src` for practical navigation."
    )
    lines.append("")
    lines.append("## Crate Map")
    lines.append("")
    for c in crates:
        lines.append(f"- `{c}`: files `{len(crate_files[c])}`, entities `{len(crate_entities[c])}`")

    lines.append("")
    lines.append("## Key Entities")
    lines.append("")
    for c in crates:
        lines.append(f"### `{c}`")
        sample = sorted(crate_entities[c])[:12]
        if not sample:
            lines.append("- _(no entities in filtered scope)_")
        else:
            for s in sample:
                lines.append(f"- `{s}`")
        lines.append("")

    lines.append("## Fast Navigation")
    lines.append("")
    for c in crates:
        sample_files = sorted(crate_files[c])[:6]
        lines.append(f"- `{c}`:")
        for p in sample_files:
            lines.append(f"  - `{p}`")
    lines.append("")
    lines.append(
        "_Note: refresh this document only when new modules/crates are added or major structure changes._"
    )
    return "\n".join(lines) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", help="Path to CQDS index JSON; stdin if omitted.")
    ap.add_argument("--output", required=True, help="Output markdown path.")
    args = ap.parse_args()

    payload = _read_payload(args.input)
    md = build_markdown(payload)
    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(md, encoding="utf-8")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
