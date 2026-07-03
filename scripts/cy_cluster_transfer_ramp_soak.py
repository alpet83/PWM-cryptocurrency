#!/usr/bin/env python3
"""
CY cluster transfer-ramp throughput soak.

Bursts same-domain Transfer txs between demo-genesis wallet accounts via proposer RPC.
Ramp volume increases each sealed block (or each window of blocks). Emits client JSONL
and a markdown summary; pair with scripts/_analyze_transfer_ramp.py for block_timing join.

Example (live cluster):
  python scripts/cy_cluster_transfer_ramp_soak.py \\
    --rpc http://127.0.0.1:3030 \\
    --wallet tmp/demo-genesis-wallet.yaml \\
    --pwm-bin F:/pwm-test/pwm-protocol/debug/pwm.exe \\
    --max-blocks 30 --start-txs-per-block 1 --step-txs-per-block 1
"""
from __future__ import annotations

import argparse
import json
from concurrent.futures import ThreadPoolExecutor, as_completed
import os
import statistics
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parent.parent


@dataclass
class WalletAccount:
    derivation_index: int
    id_hex: str
    id_pretty: str
    expected_flags_u32: int
    flags_derived_u32: int
    flags_mask_u32: int


@dataclass
class TxResult:
    ok: bool
    latency_ms: float
    exit_code: int
    output: str
    head_at_submit: int
    from_index: int
    to_pretty: str
    nonce_before: int | None = None


@dataclass
class BlockBatch:
    height: int
    level: int
    txs_target: int
    txs_ok: int = 0
    txs_fail: int = 0
    rpc_latency_ms_p50: float | None = None
    seal_slip_ms: float | None = None
    node_pending: int | None = None
    pending_ticks_at_seal: int | None = None
    block_dt_ms: float | None = None


def utc_ts() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")


def load_wallet_accounts(path: Path) -> list[WalletAccount]:
    try:
        import yaml  # type: ignore
    except ImportError as exc:
        raise SystemExit("PyYAML required: pip install pyyaml") from exc
    data = yaml.safe_load(path.read_text(encoding="utf-8"))
    out: list[WalletAccount] = []
    for row in data.get("accounts") or []:
        out.append(
            WalletAccount(
                derivation_index=int(row["derivation_index"]),
                id_hex=str(row["id_hex"]).lower(),
                id_pretty=str(row["id_pretty"]),
                expected_flags_u32=int(row.get("expected_flags_u32", 0)),
                flags_derived_u32=int(row.get("flags_derived_u32", 0)),
                flags_mask_u32=int(row.get("flags_mask_u32", 0)),
            )
        )
    return out


def resolve_pwm_bin(explicit: str | None) -> Path:
    if explicit:
        p = Path(explicit)
        if p.is_file():
            return p
        raise SystemExit(f"pwm binary not found: {p}")
    for cand in (
        os.environ.get("PWM_BIN"),
        REPO / "target" / "debug" / "pwm.exe",
        REPO / "target" / "debug" / "pwm",
        Path("F:/pwm-test/pwm-protocol/debug/pwm.exe"),
    ):
        if not cand:
            continue
        p = Path(cand)
        if p.is_file():
            return p
    raise SystemExit("pwm binary not found; pass --pwm-bin or set PWM_BIN")


def http_json(url: str, timeout: float = 5.0) -> dict[str, Any]:
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def get_head(rpc: str) -> int:
    data = http_json(f"{rpc.rstrip('/')}/v1/head")
    return int(data["height"])


def get_node_counters(rpc: str) -> dict[str, int] | None:
    try:
        data = http_json(f"{rpc.rstrip('/')}/v1/status")
        counters = data.get("tx_counters") or {}
        incoming = int(counters.get("incoming", 0))
        sealed = int(counters.get("sealed", 0))
        rejected = int(counters.get("rejected", 0))
        return {
            "incoming": incoming,
            "sealed": sealed,
            "rejected": rejected,
            "pending": max(0, incoming - sealed - rejected),
        }
    except Exception:  # noqa: BLE001
        return None


def get_account(rpc: str, id_hex: str) -> dict[str, Any] | None:
    try:
        return http_json(f"{rpc.rstrip('/')}/v1/account/{id_hex}")
    except urllib.error.HTTPError as err:
        if err.code == 404:
            return None
        raise


def percentile(values: list[float], pct: float) -> float | None:
    if not values:
        return None
    if len(values) == 1:
        return values[0]
    values = sorted(values)
    k = (len(values) - 1) * (pct / 100.0)
    f = int(k)
    c = min(f + 1, len(values) - 1)
    if f == c:
        return values[f]
    return values[f] + (values[c] - values[f]) * (k - f)


def read_block_timing_row(path: Path, height: int) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    # Tail scan is enough for live soak; full file for post-run analyze.
    last_match = None
    with path.open(encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            if int(row.get("height", -1)) == height:
                last_match = row
    return last_match


def run_pwm_tx_send(
    pwm_bin: Path,
    rpc: str,
    wallet: Path,
    from_index: int,
    to_pretty: str,
    from_hex: str,
    amount: int,
    fee: int,
    cwd: Path,
    nonce_override: int | None = None,
) -> TxResult:
    t0 = time.perf_counter()
    head = get_head(rpc)
    if nonce_override is None:
        view = get_account(rpc, from_hex)
        nonce_before = int(view["nonce"]) if view and view.get("nonce") is not None else None
    else:
        nonce_before = nonce_override
    cmd = [
        str(pwm_bin),
        "--rpc",
        rpc,
        "tx-send",
        "--wallet",
        str(wallet),
        "--index",
        str(from_index),
        "--to",
        to_pretty,
        "--amount",
        str(amount),
        "--fee",
        str(fee),
    ]
    if nonce_override is not None:
        cmd.extend(["--nonce", str(nonce_override)])
    proc = subprocess.run(
        cmd,
        cwd=str(cwd),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    latency_ms = (time.perf_counter() - t0) * 1000.0
    out = (proc.stdout or "") + (proc.stderr or "")
    return TxResult(
        ok=proc.returncode == 0,
        latency_ms=latency_ms,
        exit_code=proc.returncode,
        output=out.strip(),
        head_at_submit=head,
        from_index=from_index,
        to_pretty=to_pretty,
        nonce_before=nonce_before,
    )


def run_pwm_tx_init(pwm_bin: Path, rpc: str, wallet: Path, index: int, cwd: Path) -> tuple[bool, str]:
    proc = subprocess.run(
        [
            str(pwm_bin),
            "--rpc",
            rpc,
            "tx-init",
            "--wallet",
            str(wallet),
            "--index",
            str(index),
            "--flags",
            "0",
        ],
        cwd=str(cwd),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    out = ((proc.stdout or "") + (proc.stderr or "")).strip()
    return proc.returncode == 0, out


def wait_account_exists(rpc: str, id_hex: str, timeout_ms: int) -> dict[str, Any] | None:
    deadline = time.perf_counter() + timeout_ms / 1000.0
    last_err: Exception | None = None
    while time.perf_counter() < deadline:
        try:
            view = get_account(rpc, id_hex)
        except Exception as exc:  # noqa: BLE001
            last_err = exc
            view = None
        if view is not None:
            return view
        time.sleep(0.15)
    if last_err is not None:
        print(f"WARN: last account poll error for {id_hex}: {last_err}", file=sys.stderr)
    return None


def prefund_senders(
    accounts: list[WalletAccount],
    pwm_bin: Path,
    wallet: Path,
    rpc: str,
    cwd: Path,
    timeout_ms: int = 30_000,
) -> list[tuple[WalletAccount, dict[str, Any]]]:
    """Initialize every ring account and wait until it is visible on-chain."""
    views: list[tuple[WalletAccount, dict[str, Any]]] = []
    for acct in accounts:
        view = get_account(rpc, acct.id_hex)
        if view is None:
            print(f"prefund tx-init index={acct.derivation_index} account={acct.id_hex[:16]}...")
            ok, out = run_pwm_tx_init(pwm_bin, rpc, wallet, acct.derivation_index, cwd)
            if not ok:
                raise SystemExit(
                    f"prefund tx-init failed for index={acct.derivation_index} "
                    f"account={acct.id_hex}: {out[-300:]}"
                )
            view = wait_account_exists(rpc, acct.id_hex, timeout_ms)
            if view is None:
                raise SystemExit(
                    f"prefund timeout: account index={acct.derivation_index} "
                    f"id={acct.id_hex} did not appear in {rpc.rstrip('/')}/v1/account/{acct.id_hex} "
                    f"within {timeout_ms / 1000.0:.0f}s after tx-init"
                )
        views.append((acct, view))
    return views


def wait_nonce_at_least(rpc: str, id_hex: str, want_nonce: int, timeout_ms: int) -> bool:
    deadline = time.perf_counter() + timeout_ms / 1000.0
    while time.perf_counter() < deadline:
        view = get_account(rpc, id_hex)
        if view and int(view.get("nonce", 0)) >= want_nonce:
            return True
        time.sleep(0.15)
    return False


def wait_balance_at_least(rpc: str, id_hex: str, min_bal: int, timeout_ms: int) -> bool:
    deadline = time.perf_counter() + timeout_ms / 1000.0
    while time.perf_counter() < deadline:
        view = get_account(rpc, id_hex)
        if view:
            bal = int(str(view.get("balance_pwm") or view.get("local_state_balance") or 0))
            if bal >= min_bal:
                return True
        time.sleep(0.15)
    return False


def has_plain_flags(acct: WalletAccount) -> bool:
    if acct.expected_flags_u32 != 0:
        return False
    if acct.flags_mask_u32 == 0:
        return True
    return (acct.flags_derived_u32 & acct.flags_mask_u32) == 0


def is_plain_soak_account(acct: WalletAccount, view: dict[str, Any] | None) -> bool:
    if not has_plain_flags(acct):
        return False
    if view is None or not bool(view.get("initialized")):
        return False
    if view.get("rescue_address"):
        return False
    if int(view.get("active_policies") or 0) > 0:
        return False
    if int(view.get("dormant_policies") or 0) > 0:
        return False
    return True


def filter_plain_accounts(
    rpc: str, accounts: list[WalletAccount], min_balance_raw: int = 0
) -> list[WalletAccount]:
    out: list[WalletAccount] = []
    for acct in accounts:
        view = get_account(rpc, acct.id_hex)
        if not is_plain_soak_account(acct, view):
            print(f"skip non-plain account index={acct.derivation_index}", file=sys.stderr)
            continue
        if min_balance_raw > 0:
            bal = int(str(view.get("balance_pwm") or view.get("local_state_balance") or 0))
            if bal < min_balance_raw:
                print(
                    f"skip underfunded account index={acct.derivation_index} bal={bal} < {min_balance_raw}",
                    file=sys.stderr,
                )
                continue
        out.append(acct)
    return out


def probe_sendable_accounts(
    rpc: str,
    pwm_bin: Path,
    wallet: Path,
    accounts: list[WalletAccount],
    cwd: Path,
    amount: int,
    fee: int,
) -> list[WalletAccount]:
    """Keep accounts that can submit Transfer at tip (excludes cosign-nd without policy)."""
    sendable: list[WalletAccount] = []
    for i, acct in enumerate(accounts):
        recipient = accounts[(i + 1) % len(accounts)]
        res = run_pwm_tx_send(
            pwm_bin,
            rpc,
            wallet,
            acct.derivation_index,
            recipient.id_pretty,
            acct.id_hex,
            amount,
            fee,
            cwd,
        )
        if res.ok:
            want = (res.nonce_before or 0) + 1
            if wait_nonce_at_least(rpc, acct.id_hex, want, 30_000):
                sendable.append(acct)
            else:
                print(
                    f"probe skip sender index={acct.derivation_index}: "
                    f"transfer accepted but nonce did not advance to {want}",
                    file=sys.stderr,
                )
        else:
            print(
                f"probe skip sender index={acct.derivation_index}: "
                f"{res.output[-120:] if res.output else res.exit_code}",
                file=sys.stderr,
            )
    return sendable


def ensure_address_book(pwm_bin: Path, wallet: Path, accounts: list[WalletAccount], cwd: Path) -> None:
    try:
        import yaml  # type: ignore
    except ImportError:
        return
    data = yaml.safe_load(wallet.read_text(encoding="utf-8"))
    existing: set[str] = set()
    for e in data.get("address_book") or []:
        if isinstance(e, dict):
            existing.add(str(e.get("address", "")))
        elif isinstance(e, str):
            existing.add(e)
    for acct in accounts:
        if acct.id_pretty in existing:
            continue
        proc = subprocess.run(
            [
                str(pwm_bin),
                "wallet",
                "book-add",
                "--wallet",
                str(wallet),
                "--address",
                acct.id_pretty,
                "--label",
                f"ramp-{acct.derivation_index}",
            ],
            cwd=str(cwd),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if proc.returncode != 0 and "already" not in (proc.stderr or "").lower():
            out = ((proc.stdout or "") + (proc.stderr or "")).strip()
            if proc.returncode != 0:
                raise SystemExit(f"wallet book-add failed for {acct.id_pretty}: {out[-300:]}")


def preflight_accounts(
    rpc: str,
    pwm_bin: Path,
    wallet: Path,
    accounts: list[WalletAccount],
    cwd: Path,
    min_balance_raw: int,
    bootstrap_fund_raw: int,
    prefund: bool,
) -> None:
    if prefund or bootstrap_fund_raw > 0:
        views = prefund_senders(accounts, pwm_bin, wallet, rpc, cwd)
    else:
        views = [(acct, get_account(rpc, acct.id_hex)) for acct in accounts]

    def bal(v: dict[str, Any] | None) -> int:
        if not v:
            return 0
        return int(str(v.get("balance_pwm") or v.get("local_state_balance") or 0))

    funder_acct, funder_view = max(views, key=lambda pair: bal(pair[1]))
    funder_balance = bal(funder_view)
    need = sum(max(0, bootstrap_fund_raw - bal(v)) for _, v in views)
    if need > 0:
        if funder_balance < need + bootstrap_fund_raw:
            print(
                f"WARN: funder balance {funder_balance} may be tight for bootstrap need {need}",
                file=sys.stderr,
            )
        for acct, view in views:
            if acct.derivation_index == funder_acct.derivation_index:
                continue
            if bal(view) >= bootstrap_fund_raw:
                continue
            gap = bootstrap_fund_raw - bal(view)
            print(f"bootstrap fund {gap} raw -> {acct.id_pretty[:40]}...")
            res = run_pwm_tx_send(
                pwm_bin,
                rpc,
                wallet,
                funder_acct.derivation_index,
                acct.id_pretty,
                funder_acct.id_hex,
                gap,
                1,
                cwd,
            )
            if not res.ok:
                raise SystemExit(f"bootstrap fund failed: {res.output[-300:]}")
            want_nonce = (res.nonce_before or 0) + 1
            if not wait_nonce_at_least(rpc, funder_acct.id_hex, want_nonce, 30_000):
                print("WARN: funder nonce not advanced after bootstrap send", file=sys.stderr)
            if not wait_balance_at_least(rpc, acct.id_hex, min(bootstrap_fund_raw, min_balance_raw), 45_000):
                print(f"WARN: bootstrap balance timeout for {acct.id_pretty[:48]}", file=sys.stderr)
        views = [(acct, get_account(rpc, acct.id_hex)) for acct, _ in views]

    for acct, view in views:
        if bal(view) < min_balance_raw:
            print(
                f"WARN: account {acct.id_pretty[:48]} balance {bal(view)} < {min_balance_raw}",
                file=sys.stderr,
            )


def txs_for_level(args: argparse.Namespace, block_idx: int) -> int:
    if args.ramp_mode == "window":
        window = block_idx // args.window_blocks
        if window < args.warmup_windows:
            return max(1, args.start_txs_per_block)
        level = args.start_txs_per_block + window * args.step_txs_per_block
    else:
        if block_idx < args.warmup_blocks:
            return max(1, args.start_txs_per_block)
        level = args.start_txs_per_block + (block_idx - args.warmup_blocks) * args.step_txs_per_block
    return min(level, args.max_txs_per_block)


def wait_head_advance(rpc: str, last_head: int, timeout_ms: int, poll_ms: int) -> int | None:
    deadline = time.perf_counter() + timeout_ms / 1000.0
    while time.perf_counter() < deadline:
        h = get_head(rpc)
        if h > last_head:
            return h
        time.sleep(poll_ms / 1000.0)
    return None


def fetch_node_version(rpc: str) -> str:
    """Return a short version string from GET {rpc}/v1/version, or 'unknown' on error."""
    try:
        req = urllib.request.Request(f"{rpc}/v1/version", method="GET")
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = json.loads(resp.read())
        ver = data.get("version", "?")
        git = data.get("git", "?")
        build_ts = data.get("build_ts", "")
        return f"{ver} ({git}) built {build_ts}"
    except Exception:
        return "unknown"


def fetch_perfmon(rpc: str) -> dict[str, Any]:
    """Return GET {rpc}/v1/perfmon rows keyed by entity name, or empty on error."""
    try:
        req = urllib.request.Request(f"{rpc}/v1/perfmon", method="GET")
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = json.loads(resp.read())
    except Exception as exc:  # noqa: BLE001
        print(f"WARN: perfmon fetch failed: {exc}", file=sys.stderr)
        return {}
    if isinstance(data, list):
        return {
            str(row.get("name", f"row_{idx}")): row
            for idx, row in enumerate(data)
            if isinstance(row, dict)
        }
    if isinstance(data, dict):
        return data
    print(f"WARN: perfmon fetch returned unexpected payload: {type(data).__name__}", file=sys.stderr)
    return {}


def write_report(
    path: Path,
    args: argparse.Namespace,
    batches: list[BlockBatch],
    baseline_slip: list[float],
    stop_reason: str,
    client_path: Path,
    node_version: str = "unknown",
    perfmon_snap: dict[str, Any] | None = None,
) -> None:
    lines: list[str] = []
    lines.append("# CY transfer-ramp soak report")
    lines.append("")
    lines.append(f"- generated_utc: {datetime.now(timezone.utc).isoformat()}")
    lines.append(f"- node_version: {node_version}")
    lines.append(f"- rpc: {args.rpc}")
    lines.append(f"- wallet: {args.wallet}")
    lines.append(f"- ramp_mode: {args.ramp_mode}")
    lines.append(f"- stop_reason: {stop_reason}")
    lines.append(f"- client_jsonl: {client_path}")
    lines.append(f"- block_timing: {args.block_timing}")
    lines.append("")

    if baseline_slip:
        lines.append("## Baseline seal_slip_ms (warm-up)")
        lines.append("")
        lines.append(f"- count: {len(baseline_slip)}")
        lines.append(f"- p50: {percentile(baseline_slip, 50):.2f}")
        lines.append(f"- p95: {percentile(baseline_slip, 95):.2f}")
        lines.append("")

    lines.append("## Per-block batches")
    lines.append("")
    lines.append(
        "| height | level | target | ok | fail | reject% | rpc_p50_ms | seal_slip_ms | node_pending | pending_ticks | block_dt_ms |"
    )
    lines.append(
        "|--------|-------|--------|----|------|---------|------------|--------------|--------------|---------------|-------------|"
    )

    sustained_level = args.start_txs_per_block
    for b in batches:
        total = b.txs_ok + b.txs_fail
        reject_pct = (100.0 * b.txs_fail / total) if total else 0.0
        if reject_pct < args.max_reject_pct and b.txs_ok > 0:
            sustained_level = b.level
        lines.append(
            f"| {b.height} | {b.level} | {b.txs_target} | {b.txs_ok} | {b.txs_fail} | "
            f"{reject_pct:.1f} | "
            f"{b.rpc_latency_ms_p50 or ''} | {b.seal_slip_ms or ''} | "
            f"{'' if b.node_pending is None else b.node_pending} | "
            f"{b.pending_ticks_at_seal or ''} | {b.block_dt_ms or ''} |"
        )

    lines.append("")
    lines.append("## Throughput estimate")
    lines.append("")
    loaded = [b for b in batches if b.level > 0]
    if loaded:
        ok_total = sum(b.txs_ok for b in loaded)
        dt = sum(b.block_dt_ms or 0 for b in loaded if b.block_dt_ms)
        if dt > 0:
            lines.append(f"- loaded_tx_ok: {ok_total}")
            lines.append(f"- loaded_wall_ms: {dt:.0f}")
            lines.append(f"- sustained_tx_per_block (last good level): {sustained_level}")
            lines.append(f"- avg_tx_per_sec (burst submit only): {ok_total * 1000.0 / dt:.3f}")
        else:
            lines.append("- insufficient timing data for tx/s")
    else:
        lines.append("- no post-warmup batches")

    lines.append("")
    lines.append("## Perfmon Counters")
    lines.append("")
    snap = perfmon_snap or {}
    if snap:
        lines.append("| counter | value |")
        lines.append("|---------|-------|")
        for entity in sorted(snap):
            row = snap[entity]
            if isinstance(row, dict):
                for key in sorted(row):
                    if key == "name":
                        continue
                    value = json.dumps(row[key], sort_keys=True)
                    lines.append(f"| {entity}.{key} | {value} |")
            else:
                value = json.dumps(row, sort_keys=True)
                lines.append(f"| {entity} | {value} |")
    else:
        lines.append("- unavailable")

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="CY cluster transfer-ramp soak")
    parser.add_argument("--repo-root", type=Path, default=REPO)
    parser.add_argument("--rpc", "--url", default=os.environ.get("PWM_RPC", "http://127.0.0.1:3030"))
    parser.add_argument("--wallet", type=Path, default=Path("tmp/demo-genesis-wallet.yaml"))
    parser.add_argument("--pwm-bin", default=None)
    parser.add_argument("--block-timing", type=Path, default=Path("tmp/cy-lab-block-timing.jsonl"))
    parser.add_argument("--amount", type=int, default=1000)
    parser.add_argument("--fee", type=int, default=1)
    parser.add_argument("--min-balance-raw", type=int, default=0,
                        help="Min sender balance to include in ramp (default: amount+fee+1)")
    parser.add_argument("--max-accounts", type=int, default=0, help="0 = all flags=0 accounts")
    parser.add_argument("--ramp-mode", choices=("block", "window"), default="block")
    parser.add_argument("--window-blocks", type=int, default=10)
    parser.add_argument("--warmup-blocks", type=int, default=2)
    parser.add_argument("--warmup-windows", type=int, default=2)
    parser.add_argument("--start-txs-per-block", type=int, default=1)
    parser.add_argument("--step-txs-per-block", type=int, default=1)
    parser.add_argument("--max-txs-per-block", "--target-tps", type=int, default=64)
    parser.add_argument("--max-blocks", type=int, default=0, help="0 = until stop condition")
    parser.add_argument("--soak-sec", "--duration", type=int, default=60)
    parser.add_argument("--max-reject-pct", type=float, default=5.0)
    parser.add_argument("--inflight-limit", type=int, default=200)
    parser.add_argument("--max-throttle-ms", type=int, default=500)
    parser.add_argument("--stall-timeout-ms", type=int, default=8000)
    parser.add_argument("--slip-mult-stop", type=float, default=3.0)
    parser.add_argument("--block-dt-overrun-mult", type=float, default=1.15)
    parser.add_argument("--head-poll-ms", type=int, default=100)
    parser.add_argument("--out-prefix", type=Path, default=None)
    parser.add_argument(
        "--skip-probe",
        action="store_true",
        default=True,
        help="Deprecated: flags=0 read-only sender filtering is always used",
    )
    args = parser.parse_args()

    repo = args.repo_root.resolve()
    wallet = (repo / args.wallet).resolve() if not args.wallet.is_absolute() else args.wallet
    block_timing = (repo / args.block_timing).resolve() if not args.block_timing.is_absolute() else args.block_timing
    pwm_bin = resolve_pwm_bin(args.pwm_bin)

    ts = utc_ts()
    out_prefix = args.out_prefix or (repo / "tmp" / f"cy-transfer-ramp-{ts}")
    if out_prefix.suffix:
        client_path = out_prefix.with_suffix(".client.jsonl")
        report_path = out_prefix.with_suffix(".md")
    else:
        client_path = Path(str(out_prefix) + ".client.jsonl")
        report_path = Path(str(out_prefix) + ".md")

    min_balance = args.min_balance_raw if args.min_balance_raw > 0 else args.amount + args.fee + 1
    accounts = [a for a in load_wallet_accounts(wallet) if a.expected_flags_u32 == 0]
    accounts = filter_plain_accounts(args.rpc, accounts, min_balance_raw=min_balance)
    if len(accounts) < 2:
        print("FATAL: need >= 2 accounts with expected_flags_u32=0", file=sys.stderr)
        return 1

    print(
        f"rpc={args.rpc} wallet={wallet} accounts={len(accounts)} "
        f"max_senders={args.max_accounts or 'all'} pwm={pwm_bin}"
    )

    try:
        head0 = get_head(args.rpc)
    except Exception as exc:  # noqa: BLE001
        print(f"FATAL: RPC unreachable: {exc}", file=sys.stderr)
        return 1
    print(f"head_start={head0}")

    ensure_address_book(pwm_bin, wallet, accounts, repo)

    recipient_accounts = accounts
    sender_accounts = accounts[: args.max_accounts] if args.max_accounts > 0 else accounts[:]
    nonce_cache: dict[str, int] = {}
    for acct in sender_accounts:
        view = get_account(args.rpc, acct.id_hex)
        if not view or view.get("nonce") is None:
            print(f"FATAL: missing nonce for sender {acct.id_pretty}", file=sys.stderr)
            return 1
        nonce_cache[acct.id_hex] = int(view["nonce"])
    print(f"senders={len(sender_accounts)} recipients={len(recipient_accounts)}")

    batches: list[BlockBatch] = []
    baseline_slip: list[float] = []
    stop_reason = "max_blocks"
    last_head = head0
    last_block_ts = time.perf_counter()
    soak_started = last_block_ts
    slow_block_streak = 0
    global_tx = 0
    block_idx = 0
    # Per-sender: confirmed block height when last used.
    # Sender is safe to reuse only after their block is confirmed (last_used_height < last_head).
    sender_last_used: dict[str, int] = {}  # id_hex → confirmed block height of last use
    # Cursor for round-robin rotation: start index into sender_accounts for next batch.
    sender_cursor = 0

    def pick_senders(target: int, confirmed_height: int) -> list:
        """Pick up to `target` senders not pending, rotating through the pool each block."""
        n = len(sender_accounts)
        if n == 0:
            return []
        chosen = []
        # Round-robin: start from sender_cursor, wrap around
        for offset in range(n):
            s = sender_accounts[(sender_cursor + offset) % n]
            if sender_last_used.get(s.id_hex, -1) < confirmed_height:
                chosen.append(s)
            if len(chosen) >= target:
                break
        if len(chosen) < target:
            print(
                f"WARN: only {len(chosen)}/{target} senders available "
                f"(not reusing pending senders at height {confirmed_height})",
                file=sys.stderr,
            )
        return chosen

    with client_path.open("w", encoding="utf-8") as client_fh:
        while True:
            if args.max_blocks > 0 and block_idx >= args.max_blocks:
                stop_reason = "max_blocks"
                break

            batch_height = last_head
            level = txs_for_level(args, block_idx)
            target = min(level, len(sender_accounts)) if sender_accounts else level
            if target < level:
                print(
                    f"cap block txs {level}->{target} (senders={len(sender_accounts)})",
                    file=sys.stderr,
                )
            batch = BlockBatch(
                height=batch_height,
                level=level,
                txs_target=target,
            )
            node_counters = get_node_counters(args.rpc)
            if node_counters is not None:
                batch.node_pending = node_counters["pending"]
                if batch.node_pending > args.inflight_limit:
                    delay_ms = min(
                        args.max_throttle_ms,
                        (batch.node_pending - args.inflight_limit) * 5,
                    )
                    print(
                        f"throttle node_pending={batch.node_pending} "
                        f"limit={args.inflight_limit} delay_ms={delay_ms}"
                    )
                    time.sleep(delay_ms / 1000.0)
                    node_counters = get_node_counters(args.rpc)
                    if node_counters is not None:
                        batch.node_pending = node_counters["pending"]
            latencies: list[float] = []

            # Pick unique senders not pending from this confirmed height
            batch_senders = pick_senders(target, last_head)
            batch_nonces: dict[str, int] = {}
            for sender in batch_senders:
                batch_nonces[sender.id_hex] = nonce_cache[sender.id_hex]
                nonce_cache[sender.id_hex] += 1
            actual_target = len(batch_senders)
            batch.txs_target = actual_target
            pool_ready   = sum(1 for s in sender_accounts if sender_last_used.get(s.id_hex, -1) < last_head)
            pool_pending = len(sender_accounts) - pool_ready
            print(
                f"  submit height={last_head} level={target} picked={actual_target} "
                f"pool_ready={pool_ready} pool_pending={pool_pending} "
                f"node_pending={batch.node_pending}"
            )

            def submit_one(i: int) -> tuple[int, TxResult]:
                sender = batch_senders[i]
                # recipient: offset by half the pool to avoid sender==recipient
                recv_pos = (i + len(recipient_accounts) // 2) % len(recipient_accounts)
                if recipient_accounts[recv_pos].id_hex == sender.id_hex:
                    recv_pos = (recv_pos + 1) % len(recipient_accounts)
                recipient = recipient_accounts[recv_pos]
                return i, run_pwm_tx_send(
                    pwm_bin,
                    args.rpc,
                    wallet,
                    sender.derivation_index,
                    recipient.id_pretty,
                    sender.id_hex,
                    args.amount,
                    args.fee,
                    repo,
                    nonce_override=batch_nonces[sender.id_hex],
                )

            results: list[tuple[int, TxResult]] = []
            with ThreadPoolExecutor(max_workers=max(1, actual_target)) as pool:
                futures = [pool.submit(submit_one, i) for i in range(actual_target)]
                for fut in as_completed(futures):
                    results.append(fut.result())

            # Mark senders as used at this batch height (before confirmation)
            for s in batch_senders:
                sender_last_used[s.id_hex] = last_head
            # Advance cursor so next block starts from where we left off (fair rotation)
            sender_cursor = (sender_cursor + actual_target) % max(1, len(sender_accounts))

            for res_idx, res in sorted(results, key=lambda row: row[0]):
                sender = batch_senders[res_idx]
                latencies.append(res.latency_ms)
                if res.ok:
                    batch.txs_ok += 1
                else:
                    batch.txs_fail += 1
                    nonce_cache[sender.id_hex] = batch_nonces[sender.id_hex]
                    if "nonce" in res.output.lower():
                        view = get_account(args.rpc, sender.id_hex)
                        if view and view.get("nonce") is not None:
                            nonce_cache[sender.id_hex] = int(view["nonce"])
                client_fh.write(
                    json.dumps(
                        {
                            "ts_ms": int(time.time() * 1000),
                            "height_at_submit": res.head_at_submit,
                            "batch_height": batch_height,
                            "block_idx": block_idx,
                            "level": level,
                            "from_index": res.from_index,
                            "to": res.to_pretty,
                            "amount": args.amount,
                            "fee": args.fee,
                            "ok": res.ok,
                            "exit_code": res.exit_code,
                            "rpc_latency_ms": round(res.latency_ms, 2),
                            "nonce_before": res.nonce_before,
                            "node_pending": batch.node_pending,
                            "error_tail": res.output[-300:] if not res.ok else "",
                        },
                        ensure_ascii=False,
                    )
                    + "\n"
                )
                client_fh.flush()

            global_tx += actual_target
            batch.rpc_latency_ms_p50 = percentile(latencies, 50)

            confirmed_head = wait_head_advance(
                args.rpc, batch_height, args.stall_timeout_ms, args.head_poll_ms
            )
            if confirmed_head is None:
                stop_reason = "head_stall"
                batches.append(batch)
                break

            now = time.perf_counter()
            block_dt_ms = (now - last_block_ts) * 1000.0
            last_block_ts = now

            timing = read_block_timing_row(block_timing, confirmed_head)
            seal_slip = float(timing["seal_slip_ms"]) if timing and timing.get("seal_slip_ms") is not None else None
            pending_ticks = (
                int(timing["pending_ticks_at_seal"])
                if timing and timing.get("pending_ticks_at_seal") is not None
                else None
            )
            batch.height = confirmed_head
            batch.seal_slip_ms = seal_slip
            batch.pending_ticks_at_seal = pending_ticks
            batch.block_dt_ms = block_dt_ms
            batches.append(batch)

            if block_idx < args.warmup_blocks and seal_slip is not None:
                baseline_slip.append(seal_slip)

            total = batch.txs_ok + batch.txs_fail
            reject_pct = (100.0 * batch.txs_fail / total) if total else 0.0

            # Sender pool stats after confirmation
            senders_ready   = sum(1 for s in sender_accounts if sender_last_used.get(s.id_hex, -1) < confirmed_head)
            senders_pending = len(sender_accounts) - senders_ready
            print(
                f"height={confirmed_head} block_idx={block_idx} level={level} "
                f"ok={batch.txs_ok} fail={batch.txs_fail} slip={seal_slip} "
                f"node_pending={batch.node_pending} "
                f"senders_ready={senders_ready} senders_pending={senders_pending} "
                f"cursor={sender_cursor}"
            )

            if block_idx >= args.warmup_blocks and reject_pct > args.max_reject_pct:
                stop_reason = "reject_rate"
                break
            if block_dt_ms > 1000.0 * args.block_dt_overrun_mult:
                slow_block_streak += 1
            else:
                slow_block_streak = 0
            if slow_block_streak >= 2:
                stop_reason = "block_dt_overrun"
                break
            if args.soak_sec > 0 and (time.perf_counter() - soak_started) >= args.soak_sec:
                stop_reason = "soak_sec"
                break
            if level >= args.max_txs_per_block and block_idx >= args.warmup_blocks:
                stop_reason = "max_tx_level"
                break

            last_head = confirmed_head
            block_idx += 1

    node_version = fetch_node_version(args.rpc)
    perfmon_snap = fetch_perfmon(args.rpc)
    print("perfmon_snapshot=" + json.dumps(perfmon_snap, sort_keys=True))
    write_report(
        report_path,
        args,
        batches,
        baseline_slip,
        stop_reason,
        client_path,
        node_version,
        perfmon_snap,
    )
    print(f"client_jsonl={client_path}")
    print(f"report={report_path}")
    print(f"stop_reason={stop_reason}")
    return 0 if stop_reason in ("max_blocks", "max_tx_level") else 0


if __name__ == "__main__":
    raise SystemExit(main())
