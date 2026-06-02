#!/usr/bin/env python3
"""CY E2E s2 marks saturation soak harness (REST-only).
Usage: python scripts/cy_cluster_marks_soak.py [--interval-sec 300] [--soak-hours 2]
REST-only: no cargo compile overhead. Approximates effective marks from (head - last_block) * coeff / 3600.
"""
import argparse, json, sys, time
from datetime import datetime, timedelta
from pathlib import Path
from urllib.request import urlopen, Request

MARKS_CAP = 4294967295
BLOCKS_PER_HOUR = 3600
REPO = Path(__file__).resolve().parent.parent

def rpc_get(path, timeout=5):
    try:
        u = f"http://127.0.0.1:3030{path}"
        with urlopen(Request(u), timeout=timeout) as r:
            return json.loads(r.read())
    except Exception as e:
        print(f"WARN: {path} error: {e}", file=sys.stderr)
        return None

def get_head():
    r = rpc_get("/v1/head")
    return int(r["height"]) if r else -1

def get_staked_accounts():
    r = rpc_get("/v1/accounts")
    return [a for a in r.get("accounts", []) if int(a.get("staked", 0)) > 0] if r else []

def get_account_snapshot(account_id, marks_coeff=10000):
    r = rpc_get(f"/v1/account/{account_id}")
    if not r:
        return None
    stored = int(r.get("marks", 0))
    last_block = int(r.get("marks_last_block", 0))
    head = get_head()
    eff = stored
    sat_pct = min(100, int(stored * 100 / MARKS_CAP))
    if head > 0 and last_block > 0 and head > last_block:
        delta_blocks = head - last_block
        delta_hours = delta_blocks / BLOCKS_PER_HOUR if BLOCKS_PER_HOUR > 0 else 0
        lazy_est = int(delta_hours * marks_coeff)
        tentative_eff = min(stored + lazy_est, MARKS_CAP)
        if tentative_eff > stored:
            eff = tentative_eff
            sat_pct = min(100, int(eff * 100 / MARKS_CAP))
    return {"marks_stored": stored, "marks_effective": eff, "marks_sat_pct": sat_pct,
            "marks_last_block": last_block, "staked": int(r.get("staked", 0))}

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--interval-sec", type=int, default=300)
    p.add_argument("--soak-hours", type=float, default=2)
    p.add_argument("--rpc", default="http://127.0.0.1:3030")
    args = p.parse_args()

    report_path = REPO / f"tmp/cy-e2e-s2-{datetime.now().strftime('%Y%m%d_%H%M%S')}.md"
    lines = []

    def add(s=""):
        lines.append(s)
        print(s, flush=True)

    add(f"# CY E2E s2 — marks saturation soak")
    add(f"- Started: {datetime.now().isoformat()}")
    add(f"- Interval: {args.interval_sec}s, Soak: {args.soak_hours}h")
    add(f"- Blocks per hour: {BLOCKS_PER_HOUR}")
    add("")

    head0 = get_head()
    add(f"## Preflight: head={head0}")
    staked = get_staked_accounts()
    add(f"- Staked accounts: {len(staked)}")

    if not staked:
        add("FATAL: No staked accounts")
        report_path.write_text("\n".join(lines), encoding="utf-8")
        sys.exit(1)

    targets = []
    for acct in staked:
        aid = acct["id"]
        snap = get_account_snapshot(aid)
        if not snap:
            continue
        targets.append({**snap, "id": aid})
    if not targets:
        add("FATAL: No target accounts resolved")
        report_path.write_text("\n".join(lines), encoding="utf-8")
        sys.exit(1)

    for t in targets:
        add(f"- {t['id'][:16]}: stored={t['marks_stored']} eff={t['marks_effective']} sat={t['marks_sat_pct']}% last_block={t['marks_last_block']}")
    add("")

    add("## Time series")
    add("| Cycle | Elapsed | Head | AccountId | Stored | Effective | Sat% | LastBlock | Staked |")
    add("|-------|---------|------|-----------|--------|-----------|------|-----------|--------|")

    def write_row(cycle, elapsed, head, acct):
        add(f"| {cycle} | {int(elapsed)}min | {head} | {acct['id'][:16]} | {acct['marks_stored']} | "
            f"{acct['marks_effective']} | {acct['marks_sat_pct']}% | {acct['marks_last_block']} | {acct['staked']} |")

    start = datetime.now()
    deadline = start + timedelta(hours=args.soak_hours)

    head0 = get_head()
    for acct in targets:
        acct["_snap"] = get_account_snapshot(acct["id"]) or acct
        write_row(0, 0, head0, acct["_snap"] if acct.get("_snap") else acct)

    passed = False
    cycle = 0
    while datetime.now() < deadline:
        time.sleep(args.interval_sec)
        cycle += 1
        elapsed = (datetime.now() - start).total_seconds() / 60.0
        head = get_head()

        all_pass = True
        for acct in targets:
            snap = get_account_snapshot(acct["id"])
            if not snap:
                all_pass = False
                continue
            write_row(cycle, elapsed, head, snap)
            if snap["marks_sat_pct"] < 100 or snap["marks_effective"] < MARKS_CAP:
                all_pass = False

        if all_pass:
            add("")
            add("## PASS — all accounts at MARKS_CAP")
            add(f"PASS_EVIDENCE: soak=s2 elapsed={int(elapsed)}min head={head} accounts={len(targets)}")
            passed = True
            break

        best_sat = max((t.get("_snap", t) for t in targets),
                       key=lambda x: x.get("marks_sat_pct", 0)).get("marks_sat_pct", 0)
        add(f"- cycle={cycle} elapsed={int(elapsed)}min head={head} best_sat={best_sat}%")

    if not passed:
        add("")
        add("## FAIL — timeout")
        for acct in targets:
            snap = get_account_snapshot(acct["id"]) or acct
            status = "cap" if snap["marks_sat_pct"] >= 100 else "partial"
            add(f"- {acct['id'][:16]}: eff={snap['marks_effective']} sat={snap['marks_sat_pct']}% status={status}")

    add("")
    add(f"**Verdict: {'PASS' if passed else 'FAIL'}**")
    add(f"Report: {report_path}")
    report_path.write_text("\n".join(lines), encoding="utf-8")
    sys.exit(0 if passed else 1)

if __name__ == "__main__":
    main()