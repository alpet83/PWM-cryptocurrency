#!/usr/bin/env python3
"""Orchestrator fallback: place ticket JSON in .cqds/team-tasks/queue with delegation block.
Prefer cq_team_bridge_ctl share_ticket (project_id=5) when MCP is up.
Guards against accidental future-dated ticket IDs unless explicitly overridden.
"""
from __future__ import annotations

import json
import os
import sys
import time
from datetime import date
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TEAM_QUEUE = REPO / ".cqds" / "team-tasks" / "queue"
EVENTS = REPO / ".cqds" / "team-tasks" / "events" / "bridge-events.jsonl"


def _allow_future_ticket_date() -> bool:
    raw = str(os.environ.get("PWM_ALLOW_FUTURE_TICKET_DATE", "")).strip().lower()
    return raw not in ("", "0", "false", "no", "off")


def _ticket_prefix_date(ticket_id: str) -> date | None:
    if len(ticket_id) < 8 or not ticket_id[:8].isdigit():
        return None
    yyyy = int(ticket_id[:4])
    mm = int(ticket_id[4:6])
    dd = int(ticket_id[6:8])
    try:
        return date(yyyy, mm, dd)
    except ValueError:
        return None


def _validate_ticket_date_guard(ticket_id: str, data: dict) -> None:
    pref = _ticket_prefix_date(ticket_id)
    if pref is None:
        return
    today = date.today()
    if pref > today and not _allow_future_ticket_date():
        raise ValueError(
            "ticket id date guard: "
            f"{ticket_id[:8]} is in the future (today={today.isoformat()}); "
            "set PWM_ALLOW_FUTURE_TICKET_DATE=1 only for explicit override"
        )
    planned_for = data.get("planned_for")
    if planned_for and isinstance(planned_for, str):
        # Soft guard: keep visibility when ticket carries a schedule date.
        if len(planned_for) == 10 and planned_for[4] == "-" and planned_for[7] == "-":
            if pref > today:
                print(
                    f"warn: ticket id uses future date {ticket_id[:8]} while planned_for={planned_for}",
                    file=sys.stderr,
                )


def share(ticket_id: str, lane: str = "coding", agent: str = "pwm-coding") -> Path:
    src = REPO / "tasks" / f"{ticket_id}.json"
    if not src.is_file():
        raise FileNotFoundError(src)
    data = json.loads(src.read_text(encoding="utf-8-sig"))
    _validate_ticket_date_guard(ticket_id, data)
    now = int(time.time())
    data["status"] = "queue"
    data["updated_at"] = now
    delegation = {
        "version": 1,
        "target_agent_name": agent,
        "worker_lane": lane,
        "invite_note": data.get("brief", "")[:2000],
        "shared": True,
        "shared_at": now,
        "shared_by": "orchestrator-share-fallback",
        "state": "queued",
        "active_ticket_path": f"$TEAM_TASKS_ROOT/queue/{ticket_id}.json",
        "branch_id": "main",
        "history": [{"ts": now, "event": "shared", "meta_block": "delegation"}],
    }
    data["delegation"] = delegation
    data["shared"] = True
    data["shared_at"] = now
    data["state"] = "queued"
    data["active_ticket_path"] = delegation["active_ticket_path"]
    TEAM_QUEUE.mkdir(parents=True, exist_ok=True)
    dest = TEAM_QUEUE / f"{ticket_id}.json"
    dest.write_text(
        json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    EVENTS.parent.mkdir(parents=True, exist_ok=True)
    with EVENTS.open("a", encoding="utf-8") as f:
        f.write(
            json.dumps(
                {
                    "type": "ticket_shared",
                    "ticket_id": ticket_id,
                    "worker_lane": lane,
                    "target_agent_name": agent,
                    "ts": now,
                    "source": "orchestrator_share_fallback",
                },
                ensure_ascii=False,
            )
            + "\n"
        )
    return dest


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: share_ticket_to_bridge.py <ticket_id> [ticket_id...]", file=sys.stderr)
        return 2
    for tid in argv[1:]:
        dest = share(tid)
        print(dest)
    print(f"queue_count={len(list(TEAM_QUEUE.glob('*.json')))}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
