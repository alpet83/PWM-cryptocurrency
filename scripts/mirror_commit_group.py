#!/usr/bin/env python3
"""Emit git_safe_commit MCP arguments JSON for a mirror commit group."""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIRROR = Path("P:/GitHub/pwm-protocol")
GROUPS = {
    "docs": {
        "message": "docs(v6): release notes, ADRs, runbooks, reviews",
        "list": ROOT / "tmp/pwm_mirror_commit_docs.json",
    },
    "tasks": {
        "message": "chore(tasks): V6 sprint and pre-publication tickets",
        "list": ROOT / "tmp/pwm_mirror_commit_tasks.json",
    },
}


def main() -> int:
    if len(sys.argv) != 2 or sys.argv[1] not in GROUPS:
        print(f"usage: {sys.argv[0]} docs|tasks", file=sys.stderr)
        return 2
    group = GROUPS[sys.argv[1]]
    files = json.loads(group["list"].read_text(encoding="utf-8"))
    payload = {
        "mode": "commit",
        "repo_path": str(MIRROR).replace("\\", "/"),
        "public_repo": True,
        "confirm": "I_UNDERSTAND_AND_APPROVE",
        "commit_message": group["message"],
        "commit_files": files,
    }
    print(json.dumps(payload, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
