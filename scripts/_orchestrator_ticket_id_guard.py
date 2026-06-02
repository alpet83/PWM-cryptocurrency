#!/usr/bin/env python3
"""Guard ticket IDs from accidental future-date prefixes.

Usage:
  python scripts/_orchestrator_ticket_id_guard.py <ticket_id_or_path> [more...]

By default this exits non-zero if ticket ID prefix YYYYMMDD is in the future.
Override only intentionally with PWM_ALLOW_FUTURE_TICKET_DATE=1.
"""

from __future__ import annotations

import json
import os
import sys
from datetime import date
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def allow_future() -> bool:
    raw = str(os.environ.get("PWM_ALLOW_FUTURE_TICKET_DATE", "")).strip().lower()
    return raw not in ("", "0", "false", "no", "off")


def parse_prefix(ticket_id: str) -> date | None:
    if len(ticket_id) < 8 or not ticket_id[:8].isdigit():
        return None
    try:
        return date(int(ticket_id[:4]), int(ticket_id[4:6]), int(ticket_id[6:8]))
    except ValueError:
        return None


def load_ticket(arg: str) -> tuple[str, Path]:
    p = Path(arg)
    if p.suffix == ".json" or p.is_file():
        path = p if p.is_absolute() else (REPO / p)
    else:
        path = REPO / "tasks" / f"{arg}.json"
    if not path.is_file():
        raise FileNotFoundError(path)
    data = json.loads(path.read_text(encoding="utf-8-sig"))
    tid = str(data.get("id") or path.stem)
    return tid, path


def guard(tid: str, path: Path) -> None:
    pref = parse_prefix(tid)
    if pref is None:
        return
    today = date.today()
    if pref > today and not allow_future():
        raise ValueError(
            f"{path}: ticket id '{tid}' has future prefix {tid[:8]} (today={today.isoformat()})"
        )


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: _orchestrator_ticket_id_guard.py <ticket_id_or_path> [...]", file=sys.stderr)
        return 2
    errs = 0
    for arg in argv[1:]:
        try:
            tid, path = load_ticket(arg)
            guard(tid, path)
            print(f"ok: {tid}")
        except Exception as e:  # guard tool: keep diagnostics simple
            errs += 1
            print(f"error: {e}", file=sys.stderr)
    return 1 if errs else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
