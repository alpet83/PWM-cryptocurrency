#!/usr/bin/env python3
"""
fund_accounts.py — пополняет аккаунты из wallet до заданного целевого баланса.

Алгоритм:
  1. Загружает все аккаунты из wallet (expected_flags_u32 == 0).
  2. Для каждого аккаунта ниже --target-balance:
     a. Если аккаунт не инициализирован → tx-init.
     b. Переводит недостающую сумму с --funder-index (макс. баланс по умолчанию).
  3. Ждёт подтверждения каждого перевода (по nonce funder).

Пример:
  python scripts/fund_accounts.py \
    --rpc http://127.0.0.1:3030 \
    --wallet tmp/demo-genesis-wallet.yaml \
    --pwm-bin ..\rust-target-shared\debug\pwm.exe \
    --target-balance 100000 \
    --fee 1
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parent.parent


# ---------------------------------------------------------------------------
# Wallet / account types (subset from cy_cluster_transfer_ramp_soak)
# ---------------------------------------------------------------------------

@dataclass
class WalletAccount:
    derivation_index: int
    id_hex: str
    id_pretty: str
    expected_flags_u32: int


def load_wallet_accounts(path: Path) -> list[WalletAccount]:
    try:
        import yaml  # type: ignore
    except ImportError as exc:
        raise SystemExit("PyYAML required: pip install pyyaml") from exc
    data = yaml.safe_load(path.read_text(encoding="utf-8"))
    out: list[WalletAccount] = []
    for row in data.get("accounts") or []:
        out.append(WalletAccount(
            derivation_index=int(row["derivation_index"]),
            id_hex=str(row["id_hex"]).lower(),
            id_pretty=str(row["id_pretty"]),
            expected_flags_u32=int(row.get("expected_flags_u32", 0)),
        ))
    return out


# ---------------------------------------------------------------------------
# RPC helpers
# ---------------------------------------------------------------------------

def http_json(url: str, timeout: float = 8.0) -> dict[str, Any]:
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def get_account(rpc: str, id_hex: str) -> dict[str, Any] | None:
    try:
        return http_json(f"{rpc.rstrip('/')}/v1/account/{id_hex}")
    except urllib.error.HTTPError as err:
        if err.code == 404:
            return None
        raise


def balance_of(view: dict[str, Any] | None) -> int:
    if not view:
        return 0
    return int(str(view.get("balance_pwm") or view.get("local_state_balance") or 0))


def nonce_of(view: dict[str, Any] | None) -> int:
    if not view:
        return 0
    return int(view.get("nonce") or 0)


def wait_nonce(rpc: str, id_hex: str, want: int, timeout_s: float = 45.0) -> bool:
    deadline = time.perf_counter() + timeout_s
    while time.perf_counter() < deadline:
        v = get_account(rpc, id_hex)
        if v and nonce_of(v) >= want:
            return True
        time.sleep(0.3)
    return False


def wait_balance(rpc: str, id_hex: str, min_bal: int, timeout_s: float = 45.0) -> bool:
    deadline = time.perf_counter() + timeout_s
    while time.perf_counter() < deadline:
        v = get_account(rpc, id_hex)
        if v and balance_of(v) >= min_bal:
            return True
        time.sleep(0.3)
    return False


def wait_initialized(rpc: str, id_hex: str, timeout_s: float = 45.0) -> dict[str, Any] | None:
    deadline = time.perf_counter() + timeout_s
    while time.perf_counter() < deadline:
        v = get_account(rpc, id_hex)
        if v and v.get("initialized"):
            return v
        time.sleep(0.3)
    return None


# ---------------------------------------------------------------------------
# pwm CLI wrappers
# ---------------------------------------------------------------------------

def resolve_pwm_bin(explicit: str | None) -> Path:
    candidates = [
        explicit,
        os.environ.get("PWM_BIN"),
        str(REPO / "target" / "debug" / "pwm.exe"),
        str(REPO / "target" / "debug" / "pwm"),
        r"F:\pwm-test\pwm-protocol\debug\pwm.exe",
    ]
    for c in candidates:
        if c:
            p = Path(c)
            if p.is_file():
                return p
    raise SystemExit("pwm binary not found; pass --pwm-bin or set PWM_BIN")


def tx_init(pwm_bin: Path, rpc: str, wallet: Path, index: int) -> tuple[bool, str]:
    proc = subprocess.run(
        [str(pwm_bin), "--rpc", rpc, "tx-init",
         "--wallet", str(wallet), "--index", str(index), "--flags", "0"],
        cwd=str(REPO), capture_output=True, text=True,
        encoding="utf-8", errors="replace",
    )
    return proc.returncode == 0, (proc.stdout + proc.stderr).strip()


def tx_send(
    pwm_bin: Path, rpc: str, wallet: Path,
    from_index: int, to_pretty: str, amount: int, fee: int,
) -> tuple[bool, str]:
    proc = subprocess.run(
        [str(pwm_bin), "--rpc", rpc, "tx-send",
         "--wallet", str(wallet),
         "--index", str(from_index),
         "--to", to_pretty,
         "--amount", str(amount),
         "--fee", str(fee)],
        cwd=str(REPO), capture_output=True, text=True,
        encoding="utf-8", errors="replace",
    )
    return proc.returncode == 0, (proc.stdout + proc.stderr).strip()


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(description="Fund wallet accounts to target balance")
    parser.add_argument("--rpc", default=os.environ.get("PWM_RPC", "http://127.0.0.1:3030"))
    parser.add_argument("--wallet", type=Path, default=Path("tmp/demo-genesis-wallet.yaml"))
    parser.add_argument("--pwm-bin", default=None)
    parser.add_argument("--target-balance", type=int, default=100_000,
                        help="Target balance in raw PWM coins (default: 100000)")
    parser.add_argument("--fee", type=int, default=1)
    parser.add_argument("--funder-index", type=int, default=-1,
                        help="Derivation index of funder account (-1 = richest)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Print what would be done without sending tx")
    parser.add_argument("--max-accounts", type=int, default=0,
                        help="Limit number of accounts to fund (0 = all)")
    args = parser.parse_args()

    pwm_bin = resolve_pwm_bin(args.pwm_bin)
    wallet: Path = args.wallet
    if not wallet.is_absolute():
        wallet = REPO / wallet

    all_accounts = [a for a in load_wallet_accounts(wallet) if a.expected_flags_u32 == 0]
    print(f"Wallet: {wallet}  accounts with flags=0: {len(all_accounts)}")

    # Fetch balances for all accounts
    print("Fetching account balances...")
    views: list[tuple[WalletAccount, dict[str, Any] | None]] = []
    for acct in all_accounts:
        views.append((acct, get_account(args.rpc, acct.id_hex)))

    # Determine funder
    if args.funder_index >= 0:
        funder = next((a for a, _ in views if a.derivation_index == args.funder_index), None)
        if funder is None:
            raise SystemExit(f"Funder index {args.funder_index} not in wallet")
        funder_view = get_account(args.rpc, funder.id_hex)
    else:
        # Richest account
        funder, funder_view = max(views, key=lambda p: balance_of(p[1]))

    funder_bal = balance_of(funder_view)
    print(f"Funder: index={funder.derivation_index} id={funder.id_hex[:16]}... bal={funder_bal}")

    # Accounts that need funding
    need_fund = [
        (acct, view) for acct, view in views
        if acct.derivation_index != funder.derivation_index
        and balance_of(view) < args.target_balance
    ]
    if args.max_accounts > 0:
        need_fund = need_fund[:args.max_accounts]

    total_needed = sum(args.target_balance - balance_of(v) + args.fee for _, v in need_fund)
    print(f"Accounts to fund: {len(need_fund)}  total coins needed: {total_needed}")

    if funder_bal < total_needed:
        print(f"WARN: funder balance {funder_bal} < total needed {total_needed} — may run dry")

    if args.dry_run:
        print("\n--- DRY RUN ---")
        for acct, view in need_fund:
            gap = args.target_balance - balance_of(view)
            init_needed = not (view and view.get("initialized"))
            print(f"  index={acct.derivation_index:>8}  bal={balance_of(view):>10}  "
                  f"gap={gap:>10}  init={'YES' if init_needed else 'no'}")
        return 0

    # Fund each account
    ok_count = 0
    fail_count = 0
    funder_nonce = nonce_of(funder_view)

    for i, (acct, view) in enumerate(need_fund):
        current_bal = balance_of(view)
        gap = args.target_balance - current_bal
        initialized = bool(view and view.get("initialized"))
        short_id = f"index={acct.derivation_index} {acct.id_hex[:12]}..."

        print(f"[{i+1}/{len(need_fund)}] {short_id}  bal={current_bal}  gap={gap}", end="  ")

        # Step 1: init if needed
        if not initialized:
            print("tx-init...", end=" ", flush=True)
            ok, out = tx_init(pwm_bin, args.rpc, wallet, acct.derivation_index)
            if not ok:
                print(f"FAIL init: {out[-120:]}")
                fail_count += 1
                continue
            view = wait_initialized(args.rpc, acct.id_hex, timeout_s=45.0)
            if view is None:
                print("FAIL: init timeout")
                fail_count += 1
                continue
            print("init OK", end="  ", flush=True)

        # Step 2: send
        print(f"send {gap}...", end=" ", flush=True)
        ok, out = tx_send(pwm_bin, args.rpc, wallet,
                          funder.derivation_index, acct.id_pretty, gap, args.fee)
        if not ok:
            print(f"FAIL send: {out[-120:]}")
            fail_count += 1
            continue

        # Wait for funder nonce to advance (confirms tx was sealed)
        funder_nonce += 1
        if not wait_nonce(args.rpc, funder.id_hex, funder_nonce, timeout_s=30.0):
            print("WARN: nonce timeout — continuing anyway")
        else:
            print("OK", flush=True)
        ok_count += 1

    print(f"\nDone. ok={ok_count} fail={fail_count} / total={len(need_fund)}")
    return 0 if fail_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
