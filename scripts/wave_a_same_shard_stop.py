#!/usr/bin/env python3
"""Wave A harness: two same-shard nodes with deterministic graceful stop."""

import argparse
import hashlib
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Optional

MASTER_SEED_HEX = "edf0537be8ab846b22990ef31c8c7c61dbdad1d1a53c930fb102d048d6ffb520"
SENDER_DI = 167708
RECV_DI = 201143
GENESIS_PASS = "12345"


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def pick_free_tcp_ports(count: int) -> list[int]:
    """Return `count` distinct ephemeral TCP ports on 127.0.0.1 (best-effort)."""
    if count < 1:
        raise ValueError("count must be >= 1")
    socks: list[socket.socket] = []
    ports: list[int] = []
    try:
        for _ in range(count):
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            s.bind(("127.0.0.1", 0))
            ports.append(int(s.getsockname()[1]))
            socks.append(s)
        if len(set(ports)) != len(ports):
            raise RuntimeError(f"duplicate ephemeral ports from OS: {ports}")
        return ports
    finally:
        for s in socks:
            try:
                s.close()
            except OSError:
                pass


def pick_one_free_tcp_port() -> int:
    return pick_free_tcp_ports(1)[0]


def pick_two_wave_rpc_ports() -> tuple[int, int]:
    """Two RPC ports such that {p,p+100} sets do not overlap (pwmd peer_listen = rpc+100)."""
    for _ in range(200):
        p1 = pick_one_free_tcp_port()
        p2 = pick_one_free_tcp_port()
        if p1 == p2:
            continue
        if p1 > 65535 - 100 or p2 > 65535 - 100:
            continue
        peers1 = {p1, p1 + 100}
        peers2 = {p2, p2 + 100}
        if peers1.isdisjoint(peers2):
            return p1, p2
    raise RuntimeError("failed to allocate two non-conflicting wave RPC ports")


def parse_snap_chk_iv(root: Path) -> int:
    epoch_rs = root / "crates" / "pwmd" / "src" / "snapshot" / "epoch.rs"
    marker = "pub(crate) const SNAP_CHK_BLK_IV: u64 = "
    for line in epoch_rs.read_text(encoding="utf-8").splitlines():
        if marker in line:
            tail = line.split(marker, 1)[1].strip().rstrip(";")
            return int(tail)
    raise RuntimeError("cannot read SNAP_CHK_BLK_IV from snapshot/epoch.rs")


def cargo_target_root(root: Path) -> Path:
    env_root = os.environ.get("PWM_WORKSPACE_TARGET_ROOT")
    if env_root:
        return Path(env_root)
    configured = (root / ".." / "rust-target-shared").resolve()
    default = root / "target"
    probe = "pwmd.exe" if os.name == "nt" else "pwmd"
    if (configured / "debug" / probe).is_file():
        return configured
    return default


def bin_path(root: Path, name: str) -> Path:
    exe = ".exe" if os.name == "nt" else ""
    path = cargo_target_root(root) / "debug" / f"{name}{exe}"
    if not path.is_file():
        raise RuntimeError(f"missing binary: {path}")
    return path


def run_cmd(args: list[str], cwd: Path) -> str:
    proc = subprocess.run(
        args,
        cwd=str(cwd),
        check=False,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed: {' '.join(args)}\n"
            f"exit={proc.returncode}\nstdout={proc.stdout}\nstderr={proc.stderr}"
        )
    return proc.stdout


def parse_account_hex(cmd_out: str, label: str) -> str:
    marker = "account_id_hex "
    for line in cmd_out.splitlines():
        if line.startswith(marker):
            val = line[len(marker) :].strip()
            if val:
                return val
    raise RuntimeError(f"cannot parse account_id_hex from {label} output")


def http_json(url: str) -> dict:
    req = urllib.request.Request(url, method="GET")
    with urllib.request.urlopen(req, timeout=3.0) as resp:
        return json.loads(resp.read().decode("utf-8"))


def wait_ready(base: str, timeout_s: float = 45.0) -> None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        try:
            st = http_json(f"{base}/v1/status")
            if st.get("ready") is True:
                return
        except Exception:
            pass
        time.sleep(0.25)
    raise RuntimeError(f"timeout waiting for ready: {base}")


def wait_tcp_accept(host: str, port: int, timeout_s: float = 20.0) -> None:
    deadline = time.time() + timeout_s
    last_err: Optional[OSError] = None
    while time.time() < deadline:
        try:
            with socket.create_connection((host, port), timeout=1.0):
                return
        except OSError as exc:
            last_err = exc
            time.sleep(0.1)
    raise RuntimeError(
        f"timeout waiting for tcp accept {host}:{port} (last_err={last_err!r})"
    )


def wait_height(base: str, want_h: int, timeout_s: float = 90.0) -> int:
    deadline = time.time() + timeout_s
    seen = 0
    while time.time() < deadline:
        try:
            head = http_json(f"{base}/v1/head")
            seen = int(head["height"])
            if seen >= want_h:
                return seen
        except Exception:
            pass
        time.sleep(0.25)
    raise RuntimeError(f"timeout waiting for {base} head >= {want_h}, last={seen}")


def wait_children_exit(
    p1: subprocess.Popen,
    p2: subprocess.Popen,
    timeout_s: float,
) -> tuple[int, int]:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        rc1 = p1.poll()
        rc2 = p2.poll()
        if rc1 is not None and rc2 is not None:
            return rc1, rc2
        time.sleep(0.5)
    raise RuntimeError("timeout waiting both nodes to stop by debug-stop-height")


def kill_proc(proc: subprocess.Popen) -> None:
    if proc.poll() is not None:
        return
    try:
        if os.name == "nt":
            proc.send_signal(signal.CTRL_BREAK_EVENT)  # type: ignore[attr-defined]
        else:
            proc.terminate()
    except Exception:
        proc.kill()
    try:
        proc.wait(timeout=5)
    except Exception:
        proc.kill()
        proc.wait(timeout=5)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fp:
        while True:
            chunk = fp.read(1024 * 64)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as fp:
        return json.load(fp)


def account_row(snapshot: dict, acct_hex: str) -> dict:
    rows = snapshot["state"]["accounts"]
    for row in rows:
        if row["id"] == acct_hex:
            return row["account"]
    raise RuntimeError(f"account {acct_hex} not found in snapshot")


def print_hash_divergence_diag(report: dict) -> None:
    n1 = report["node1"]
    n2 = report["node2"]
    print("=== Wave A hash divergence diagnostics ===", file=sys.stderr)
    print(f"tip_hash_equal={report['tip_hash_equal']}", file=sys.stderr)
    print(f"last_epoch_hash_equal={report['last_epoch_hash_equal']}", file=sys.stderr)
    print(f"nodeA.tip_hash={n1['tip_hash']}", file=sys.stderr)
    print(f"nodeB.tip_hash={n2['tip_hash']}", file=sys.stderr)
    print(f"nodeA.head_height={n1['canonical_h']}", file=sys.stderr)
    print(f"nodeB.head_height={n2['canonical_h']}", file=sys.stderr)
    print(f"nodeA.checkpoint={n1['checkpoint_height']}", file=sys.stderr)
    print(f"nodeB.checkpoint={n2['checkpoint_height']}", file=sys.stderr)
    print(f"nodeA.last_epoch_hash={report['last_epoch_hash_node1']}", file=sys.stderr)
    print(f"nodeB.last_epoch_hash={report['last_epoch_hash_node2']}", file=sys.stderr)
    print("==========================================", file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Wave A harness: two same-shard nodes with deterministic stop",
    )
    parser.add_argument(
        "--stop-height",
        type=int,
        default=0,
        help="explicit debug stop height (default derives from 2 checkpoint windows)",
    )
    parser.add_argument(
        "--allowed-prestop-lag",
        type=int,
        default=3,
        help="max temporary head lag allowed while nodes still running",
    )
    parser.add_argument(
        "--max-wait-sec",
        type=int,
        default=900,
        help="upper bound for full wave run",
    )
    parser.add_argument(
        "--keep-artifacts",
        action="store_true",
        help="do not remove temporary wave directory on success",
    )
    args = parser.parse_args()

    root = repo_root()
    pwmd_bin = bin_path(root, "pwmd")
    pwm_bin = bin_path(root, "pwm")
    snap_chk_iv = parse_snap_chk_iv(root)
    min_stop_h = 2 * snap_chk_iv
    stop_h = max(args.stop_height, min_stop_h)

    wave_dir = Path(tempfile.mkdtemp(prefix="pwm_wave_a_"))
    wallets = wave_dir / "wallets"
    logs = wave_dir / "logs"
    states = wave_dir / "states"
    for p in (wallets, logs, states):
        p.mkdir(parents=True, exist_ok=True)

    wallet_sender = wallets / "sender.yaml"
    wallet_genesis = wallets / "genesis.yaml"
    wallet_recv = wallets / "receiver.yaml"
    genesis_json = wave_dir / "genesis.json"

    sender_out = run_cmd(
        [
            str(pwm_bin),
            "--genesis-passphrase",
            GENESIS_PASS,
            "wallet",
            "import-seed",
            "--master",
            MASTER_SEED_HEX,
            "--derivation-index",
            str(SENDER_DI),
            "--plaintext-dev",
            "--wallet-out",
            str(wallet_sender),
        ],
        root,
    )
    sender_hex = parse_account_hex(sender_out, "wallet sender import-seed")
    shutil.copyfile(wallet_sender, wallet_genesis)
    recv_out = run_cmd(
        [
            str(pwm_bin),
            "--genesis-passphrase",
            GENESIS_PASS,
            "wallet",
            "import-seed",
            "--master",
            MASTER_SEED_HEX,
            "--derivation-index",
            str(RECV_DI),
            "--plaintext-dev",
            "--wallet-out",
            str(wallet_recv),
        ],
        root,
    )
    recv_hex = parse_account_hex(recv_out, "wallet receiver import-seed")
    run_cmd(
        [
            str(pwm_bin),
            "--genesis-passphrase",
            GENESIS_PASS,
            "genesis-build",
            "--wallet",
            str(wallet_genesis),
            "--out",
            str(genesis_json),
            "--premine-bal",
            "1000000",
        ],
        root,
    )

    node1_state = states / "node1"
    node2_state = states / "node2"
    node1_state.mkdir(parents=True, exist_ok=True)
    node2_state.mkdir(parents=True, exist_ok=True)
    node1_data = node1_state / "pwm-data.json"
    node2_data = node2_state / "pwm-data.json"

    n1_rpc, n2_rpc = pick_two_wave_rpc_ports()
    n1_peer = n1_rpc + 100
    n2_peer = n2_rpc + 100
    print(
        f"wave-a dynamic ports: node1_rpc={n1_rpc} node2_rpc={n2_rpc} "
        f"(peer_listen defaults rpc+100: {n1_peer}, {n2_peer})",
        file=sys.stderr,
    )
    base1 = f"http://127.0.0.1:{n1_rpc}"
    base2 = f"http://127.0.0.1:{n2_rpc}"

    n1_log = (logs / "node1.log").open("w", encoding="utf-8")
    n2_log = (logs / "node2.log").open("w", encoding="utf-8")
    p1 = None
    p2 = None
    try:
        p1 = subprocess.Popen(
            [
                str(pwmd_bin),
                "--listen",
                f"127.0.0.1:{n1_rpc}",
                "--state-root",
                str(node1_state),
                "--data-file",
                str(node1_data),
                "--genesis-file",
                str(genesis_json),
                "--genesis-passphrase",
                GENESIS_PASS,
                "--network-id",
                "wave-a-testnet",
                "--domain-hi",
                "0x2C",
                "--cluster-id",
                "cluster-CY",
                "--node-id",
                "wave-node-1",
                "--transport-real",
                "--transport-peer-seed",
                f"127.0.0.1:{n2_peer}",
                "--debug-stop-height",
                str(stop_h),
                "--debug-deterministic-seal-time",
            ],
            cwd=str(root),
            stdout=n1_log,
            stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0,
        )
        wait_ready(base1)
        wait_tcp_accept("127.0.0.1", n1_peer)
        p2 = subprocess.Popen(
            [
                str(pwmd_bin),
                "--listen",
                f"127.0.0.1:{n2_rpc}",
                "--state-root",
                str(node2_state),
                "--data-file",
                str(node2_data),
                "--genesis-file",
                str(genesis_json),
                "--genesis-passphrase",
                GENESIS_PASS,
                "--network-id",
                "wave-a-testnet",
                "--domain-hi",
                "0x2C",
                "--cluster-id",
                "cluster-CY",
                "--node-id",
                "wave-node-2",
                "--transport-real",
                "--transport-peer-seed",
                f"127.0.0.1:{n1_peer}",
                "--debug-stop-height",
                str(stop_h),
                "--debug-deterministic-seal-time",
                "--debug-disable-seal-loop",
            ],
            cwd=str(root),
            stdout=n2_log,
            stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0,
        )

        wait_ready(base2)
        wait_tcp_accept("127.0.0.1", n2_peer)

        run_cmd(
            [
                str(pwm_bin),
                "--rpc",
                base1,
                "tx-init",
                "--wallet",
                str(wallet_recv),
                "--index",
                "1",
                "--flags",
                "0",
            ],
            root,
        )
        wait_height(base1, 2)

        tx_send_rpcs = (base1, base1, base1)
        print(
            "wave-a tx-send rpc plan (leader-only): "
            + ", ".join(tx_send_rpcs),
            file=sys.stderr,
        )
        for i, base in enumerate(tx_send_rpcs, start=1):
            print(f"wave-a tx-send#{i} via {base}", file=sys.stderr)
            run_cmd(
                [
                    str(pwm_bin),
                    "--rpc",
                    base,
                    "tx-send",
                    "--wallet",
                    str(wallet_sender),
                    "--to",
                    recv_hex,
                    "--amount",
                    "10",
                    "--fee",
                    "1",
                ],
                root,
            )
            # Observe temporary lag budget while nodes are still live.
            try:
                h1 = int(http_json(f"{base1}/v1/head")["height"])
                h2 = int(http_json(f"{base2}/v1/head")["height"])
                if abs(h1 - h2) > args.allowed_prestop_lag:
                    raise RuntimeError(
                        f"pre-stop lag exceeded after tx#{i}: h1={h1} h2={h2} "
                        f"allowed={args.allowed_prestop_lag}"
                    )
            except urllib.error.URLError:
                pass
            time.sleep(0.8)

        rc1, rc2 = wait_children_exit(p1, p2, timeout_s=args.max_wait_sec)
        if rc1 != 0 or rc2 != 0:
            raise RuntimeError(f"pwmd exited non-zero: node1={rc1}, node2={rc2}")

        snap1 = load_json(node1_data)
        snap2 = load_json(node2_data)
        man1_path = node1_state / "epochs" / "pwm-epochs-manifest.json"
        man2_path = node2_state / "epochs" / "pwm-epochs-manifest.json"
        man1 = load_json(man1_path)
        man2 = load_json(man2_path)

        h1 = int(man1["canonical_h"])
        h2 = int(man2["canonical_h"])
        if h1 < stop_h or h2 < stop_h:
            raise RuntimeError(
                f"canonical height below stop target: node1={h1} node2={h2} target={stop_h}"
            )
        if h1 != h2:
            raise RuntimeError(f"post-stop canonical height mismatch: node1={h1} node2={h2}")
        if len(man1.get("epochs", [])) != len(man2.get("epochs", [])):
            raise RuntimeError("epoch manifest length mismatch")
        if man1.get("epoch_span") != man2.get("epoch_span"):
            raise RuntimeError("epoch manifest epoch_span mismatch")
        if man1.get("schema_v") != man2.get("schema_v"):
            raise RuntimeError("epoch manifest schema_v mismatch")
        if int(snap1["checkpoint_height"]) != h1 or int(snap2["checkpoint_height"]) != h2:
            raise RuntimeError("checkpoint_height mismatch vs manifest canonical_h")

        acct_s1 = account_row(snap1, sender_hex)
        acct_s2 = account_row(snap2, sender_hex)
        acct_r1 = account_row(snap1, recv_hex)
        acct_r2 = account_row(snap2, recv_hex)
        key_fields = ("balance_pwm", "nonce", "initialized")
        for fld in key_fields:
            if acct_s1[fld] != acct_s2[fld]:
                raise RuntimeError(f"sender field mismatch {fld}: {acct_s1[fld]} vs {acct_s2[fld]}")
            if acct_r1[fld] != acct_r2[fld]:
                raise RuntimeError(
                    f"receiver field mismatch {fld}: {acct_r1[fld]} vs {acct_r2[fld]}"
                )

        last_epoch1 = man1["epochs"][-1]["file_name"] if man1["epochs"] else None
        last_epoch2 = man2["epochs"][-1]["file_name"] if man2["epochs"] else None
        if last_epoch1 != last_epoch2:
            raise RuntimeError("last epoch file name mismatch between nodes")
        epoch_hash_eq = True
        epoch_hash_1 = None
        epoch_hash_2 = None
        if last_epoch1 is not None:
            ep1 = node1_state / "epochs" / last_epoch1
            ep2 = node2_state / "epochs" / last_epoch2
            epoch_hash_1 = sha256_file(ep1)
            epoch_hash_2 = sha256_file(ep2)
            epoch_hash_eq = epoch_hash_1 == epoch_hash_2

        tip_hash_equal = man1["tip_hash"] == man2["tip_hash"]
        report = {
            "wave": "A",
            "snap_chk_blk_iv": snap_chk_iv,
            "stop_height_target": stop_h,
            "node1": {
                "rpc": base1,
                "canonical_h": h1,
                "checkpoint_height": int(snap1["checkpoint_height"]),
                "tip_hash": man1["tip_hash"],
                "manifest_sha256": sha256_file(man1_path),
            },
            "node2": {
                "rpc": base2,
                "canonical_h": h2,
                "checkpoint_height": int(snap2["checkpoint_height"]),
                "tip_hash": man2["tip_hash"],
                "manifest_sha256": sha256_file(man2_path),
            },
            "tip_hash_equal": tip_hash_equal,
            "sender": {k: acct_s1[k] for k in key_fields},
            "receiver": {k: acct_r1[k] for k in key_fields},
            "last_epoch_file": last_epoch1,
            "last_epoch_hash_node1": epoch_hash_1,
            "last_epoch_hash_node2": epoch_hash_2,
            "last_epoch_hash_equal": epoch_hash_eq,
            "artifacts_dir": str(wave_dir),
        }
        (wave_dir / "wave-a-report.json").write_text(
            json.dumps(report, indent=2),
            encoding="utf-8",
        )
        if not tip_hash_equal or not epoch_hash_eq:
            print_hash_divergence_diag(report)
            reasons = []
            if not tip_hash_equal:
                reasons.append("tip_hash_equal=false")
            if not epoch_hash_eq:
                reasons.append("last_epoch_hash_equal=false")
            raise RuntimeError("wave-a hash divergence: " + ", ".join(reasons))
        print(json.dumps(report, indent=2))
        if not args.keep_artifacts:
            shutil.rmtree(wave_dir, ignore_errors=True)
        return 0
    finally:
        n1_log.close()
        n2_log.close()
        if p1 is not None:
            kill_proc(p1)
        if p2 is not None:
            kill_proc(p2)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"wave-a failed: {exc}", file=sys.stderr)
        raise SystemExit(1)
