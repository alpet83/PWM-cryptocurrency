#!/usr/bin/env python3
"""
Scan Rust sources for identifier names whose underscore segments exceed policy.

Entities (snake_case / SCREAMING_SNAKE), same prod vs test budget as fn-only predecessor:
  - fn items
  - const / static
  - mod declarations (file-level modules)
  - type aliases where the alias is snake_case
  - macro_rules! names
  - struct / enum / union fields (lines inside aggregate `{ ... }` bodies)

Policy (docs/AGENT_PROMPT_coding.md):
  - production code: max 4 segments
  - test-only code: max 5 segments

Test context heuristics (unchanged):
  - Paths containing ``/tests/`` or ``/src/tests/`` → entire file uses test budget.
  - ``#[cfg(test)] mod tests { ... }`` regions and ``#[cfg(test)] fn ...`` → test budget.

Output: JSON with violations including ``entity`` (``fn``, ``const``, ``field``, ...).

stdlib only; from repo root:
  python scripts/check_entity_name_segments.py [paths...]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

PROD_MAX = 4
TEST_MAX = 5

# fn name after optional pub/unsafe/async...
FN_LINE_RE = re.compile(
    r"^\s*(?:pub\s*(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)"
)
MOD_LINE_RE = re.compile(
    r"^\s*(?:pub\s*(?:\([^)]*\))?\s+)?mod\s+([a-zA-Z_][a-zA-Z0-9_]*)\b"
)
CONST_OR_STATIC_RE = re.compile(
    r"^\s*(?:pub\s*(?:\([^)]*\))?\s+)?(?:const|static\s+mut|static)\s+"
    r"([a-zA-Z_][a-zA-Z0-9_]*)\b"
)
TYPE_ALIAS_SNAKE_RE = re.compile(
    r"^\s*(?:pub\s*(?:\([^)]*\))?\s+)?type\s+([a-z][a-z0-9_]*)\s*[=<]"
)
MACRO_RULES_RE = re.compile(r"macro_rules!\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\{")
# Struct / union / enum field: name : Type — exclude fn lines and type ascriptions in let guarded by struct-body context.
FIELD_LINE_RE = re.compile(
    r"^\s*(?:pub\s*(?:\([^)]*\))?\s+)?([a-z][a-z0-9_]*)\s*:\s*(?!:)"
)
AGGREGATE_START_RE = re.compile(
    r"(?:^|\s)(?:pub\s*(?:\([^)]*\))?\s+)?(?:struct|enum|union)\s+[A-Za-z_][A-Za-z0-9_]*"
)


def segment_count(name: str) -> int:
    parts = [p for p in name.split("_") if p]
    return len(parts)


def is_snake_or_upper_snake(name: str) -> bool:
    if not name or not (name[0].isascii() and (name[0].islower() or name[0].isupper())):
        return False
    if name[0].islower():
        return bool(re.fullmatch(r"[a-z][a-z0-9_]*", name))
    return bool(re.fullmatch(r"[A-Z][A-Z0-9_]*", name))


def is_entire_file_test_context(rel_posix: str) -> bool:
    return "/tests/" in rel_posix or "/src/tests/" in rel_posix


def strip_rust_line_comment(line: str) -> str:
    in_string = False
    escape = False
    out: list[str] = []
    quote = '"'
    i = 0
    while i < len(line):
        c = line[i]
        if not in_string:
            if c in ('"', "'"):
                if c == "'" and i + 1 < len(line) and line[i + 1] == "\\":
                    pass
                in_string = True
                quote = c
                out.append(c)
            elif c == "/" and i + 1 < len(line) and line[i + 1] == "/":
                break
            else:
                out.append(c)
        else:
            out.append(c)
            if escape:
                escape = False
            elif c == "\\":
                escape = True
            elif c == quote:
                in_string = False
        i += 1
    return "".join(out)


def count_braces_delta(line: str) -> int:
    s = strip_rust_line_comment(line)
    return s.count("{") - s.count("}")


def find_cfg_test_mod_tests_regions(lines: list[str]) -> list[tuple[int, int]]:
    regions: list[tuple[int, int]] = []
    i = 0
    n = len(lines)
    while i < n:
        if re.search(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", lines[i]):
            j = i + 1
            while j < n and not lines[j].strip():
                j += 1
            if j >= n:
                break
            if re.search(r"\bmod\s+tests\s*\{", lines[j]):
                start_line = j + 1
                depth = count_braces_delta(lines[j])
                k = j + 1
                while k < n and depth > 0:
                    depth += count_braces_delta(lines[k])
                    k += 1
                end_line = k
                regions.append((start_line, end_line))
                i = k
                continue
        i += 1
    return regions


def line_in_regions(line_no: int, regions: list[tuple[int, int]]) -> bool:
    for a, b in regions:
        if a <= line_no <= b:
            return True
    return False


def find_cfg_test_single_fns(lines: list[str]) -> set[int]:
    single: set[int] = set()
    i = 0
    n = len(lines)
    while i < n:
        if re.search(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", lines[i]):
            j = i + 1
            while j < n and lines[j].strip() == "":
                j += 1
            if j < n:
                m = FN_LINE_RE.match(lines[j])
                if m and not re.search(r"\bmod\s+tests\s*\{", lines[j]):
                    single.add(j + 1)
        i += 1
    return single


def aggregate_body_stack_delta(s: str) -> tuple[int, int]:
    """
    Returns (opens, closes) for `{` `}` that belong to an aggregate (struct/enum/union)
    declaration on this line, using a shallow heuristic: braces on the same line as AGGREGATE_START.
    If the opening brace is on a later line, caller handles via pending_aggregate flag.
    """
    if not re.search(AGGREGATE_START_RE, s):
        return 0, 0
    # Braces on line after struct Name — handled elsewhere
    if "{" not in s:
        return 0, 0
    opens = s.count("{")
    closes = s.count("}")
    return opens, closes


def violations_for_file(content: str, rel_posix: str) -> list[dict[str, Any]]:
    lines = content.splitlines()
    results: list[dict[str, Any]] = []
    entire_test = is_entire_file_test_context(rel_posix)
    regions = [] if entire_test else find_cfg_test_mod_tests_regions(lines)
    single_test_lines = set() if entire_test else find_cfg_test_single_fns(lines)

    brace_depth = 0
    # After `struct Name {` opened, fields apply while brace_depth >= floor (last opened aggregate).
    aggregate_floors: list[int] = []
    pending_aggregate = False

    def limit_for_line(idx: int) -> tuple[int, str]:
        if entire_test:
            return TEST_MAX, "test"
        if idx in single_test_lines or line_in_regions(idx, regions):
            return TEST_MAX, "test"
        return PROD_MAX, "prod"

    def push_violation(
        idx: int, name: str, entity: str, limit: int, kind: str
    ) -> None:
        segs = segment_count(name)
        if segs > limit:
            results.append(
                {
                    "line": idx,
                    "name": name,
                    "entity": entity,
                    "segments": segs,
                    "limit": limit,
                    "kind": kind,
                }
            )

    for idx, line in enumerate(lines, start=1):
        raw = line
        s = strip_rust_line_comment(raw)
        delta = count_braces_delta(s)

        pre_brace = brace_depth
        brace_depth += delta

        if pending_aggregate:
            if "{" in s:
                aggregate_floors.append(brace_depth)
                pending_aggregate = False
        elif re.search(AGGREGATE_START_RE, s):
            if "{" in s and brace_depth > pre_brace:
                aggregate_floors.append(brace_depth)
            elif "{" not in s:
                pending_aggregate = True

        while aggregate_floors and brace_depth < aggregate_floors[-1]:
            aggregate_floors.pop()

        in_aggregate_body = bool(aggregate_floors) and brace_depth >= aggregate_floors[-1]

        lim, k = limit_for_line(idx)

        m_fn = FN_LINE_RE.match(raw)
        if m_fn:
            name = m_fn.group(1)
            if is_snake_or_upper_snake(name):
                push_violation(idx, name, "fn", lim, k)
            continue

        m_mod = MOD_LINE_RE.match(s)
        if m_mod:
            name = m_mod.group(1)
            if name != "tests" and is_snake_or_upper_snake(name):
                push_violation(idx, name, "mod", lim, k)

        m_cs = CONST_OR_STATIC_RE.match(s)
        if m_cs:
            name = m_cs.group(1)
            if is_snake_or_upper_snake(name):
                push_violation(idx, name, "const_or_static", lim, k)

        m_ty = TYPE_ALIAS_SNAKE_RE.match(s)
        if m_ty:
            name = m_ty.group(1)
            push_violation(idx, name, "type_alias", lim, k)

        m_mac = MACRO_RULES_RE.search(s)
        if m_mac:
            name = m_mac.group(1)
            if is_snake_or_upper_snake(name):
                push_violation(idx, name, "macro_rules", lim, k)

        if in_aggregate_body and "fn " not in s and "struct " not in s and "enum " not in s and "union " not in s:
            m_f = FIELD_LINE_RE.match(s)
            if m_f:
                fname = m_f.group(1)
                if is_snake_or_upper_snake(fname):
                    push_violation(idx, fname, "field", lim, k)

    return results


def normalize_rel(path: Path, root: Path) -> str:
    try:
        rel = path.resolve().relative_to(root.resolve())
    except ValueError:
        rel = path
    return str(rel).replace("\\", "/")


def scan_files(paths: list[Path], root: Path) -> dict[str, Any]:
    out_files: list[dict[str, Any]] = []
    for p in paths:
        if not p.exists():
            continue
        if p.is_dir():
            for rs in sorted(p.rglob("*.rs")):
                rel = normalize_rel(rs, root)
                if "/target/" in rel:
                    continue
                text = rs.read_text(encoding="utf-8", errors="replace")
                viols = violations_for_file(text, rel)
                out_files.append({"path": rel, "violations": viols})
        elif p.suffix == ".rs":
            rel = normalize_rel(p, root)
            text = p.read_text(encoding="utf-8", errors="replace")
            viols = violations_for_file(text, rel)
            out_files.append({"path": rel, "violations": viols})
    out_files.sort(key=lambda x: x["path"])
    return {"policy": {"prod_max": PROD_MAX, "test_max": TEST_MAX}, "files": out_files}


def self_check(root: Path) -> None:
    samples = [
        root / "crates/pwm-core/src/reject_wire.rs",
        root / "crates/pwm-tui/src/tx_submit.rs",
        root / "crates/pwm-cli/src/tests/mod.rs",
    ]
    missing = [str(p) for p in samples if not p.is_file()]
    if missing:
        print(
            json.dumps(
                {
                    "self_check": "SKIP",
                    "reason": "fixture files not present",
                    "missing": missing,
                },
                indent=2,
            ),
            file=sys.stderr,
        )
        sys.exit(0)

    data = scan_files(samples, root)
    expected_paths = {
        "crates/pwm-cli/src/tests/mod.rs",
        "crates/pwm-core/src/reject_wire.rs",
        "crates/pwm-tui/src/tx_submit.rs",
    }
    paths_ok = {f["path"] for f in data["files"]} == expected_paths
    all_empty = all(len(f["violations"]) == 0 for f in data["files"])
    if not paths_ok or not all_empty:
        print("SELF_CHECK_FAIL: emitted JSON != expected (paths or violations)", file=sys.stderr)
        print(json.dumps(data, indent=2))
        sys.exit(1)
    print(json.dumps({"self_check": "OK", "policy": data["policy"]}, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="*",
        help="Files or directories to scan (default: all crates/*/src and crates/*/tests)",
    )
    parser.add_argument(
        "--self-check",
        action="store_true",
        help="Run frozen fixture comparison on spot-review sample files; exit 1 on mismatch",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]

    if args.self_check:
        self_check(root)
        return

    if args.paths:
        scan_paths = [Path(p) for p in args.paths]
    else:
        scan_paths = []
        crates = root / "crates"
        if crates.is_dir():
            for c in sorted(crates.iterdir()):
                if not c.is_dir():
                    continue
                for sub in ("src", "tests"):
                    d = c / sub
                    if d.is_dir():
                        scan_paths.append(d)

    if not scan_paths:
        print(json.dumps({"error": "nothing to scan"}, indent=2))
        sys.exit(2)

    data = scan_files(scan_paths, root)
    print(json.dumps(data, indent=2))


if __name__ == "__main__":
    main()
