#!/usr/bin/env python3
"""CY lab seal console: RPC step wrapper + dual-node JSON log window.

Stdlib-only. Emits one JSON document per command, or JSONL for watch.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

REPO = Path(__file__).resolve().parent.parent
STATE_PATH = REPO / "tmp" / "cy-lab-seal-console.state.json"
DEFAULT_RPC = "http://127.0.0.1:3030"

LOG_PATTERNS = {
    "proposer": [
        "logs/*/pwmd-cy-proposer-*.log",
        "logs/*/pwmd-proposer-*.log",
    ],
    "attester": [
        "logs/*/pwmd-cy-attester-*.log",
        "logs/*/pwmd-attester-*.log",
    ],
}

TS_RE = re.compile(r"\[(\d{2}:\d{2}:\d{2}\.\d{3})\]")
KV_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)=([^\s,]+)")


@dataclass
class LogSpec:
    path: Path | None
    offset: int
    size: int


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def now_ms() -> int:
    return int(time.time() * 1000)


def read_json(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def latest_match(root: Path, patterns: list[str]) -> tuple[Path | None, list[Path]]:
    matches: list[Path] = []
    for pattern in patterns:
        matches.extend(Path(root).glob(pattern))
    matches = sorted({path.resolve() for path in matches if path.is_file()}, key=lambda p: p.stat().st_mtime_ns)
    if not matches:
        return None, []
    return matches[-1], matches


def discover_logs(root: Path = REPO) -> dict[str, Any]:
    logs = {}
    warnings: list[str] = []
    for kind, patterns in LOG_PATTERNS.items():
        chosen, matches = latest_match(root, patterns)
        logs[kind] = {
            "path": str(chosen) if chosen else None,
            "matches": [str(path) for path in matches],
            "active": len(matches),
        }
        if len(matches) > 1:
            warnings.append(f"multiple {kind} logs found; using latest mtime")
    return {"logs": logs, "warnings": warnings}


def rpc_request(base_url: str, method: str, path: str, payload: dict[str, Any] | None = None, timeout: float = 120.0) -> tuple[dict[str, Any], dict[str, Any]]:
    url = base_url.rstrip("/") + path
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    req = Request(url, data=body, method=method.upper())
    req.add_header("Content-Type", "application/json")
    t0 = time.perf_counter()
    try:
        with urlopen(req, timeout=timeout) as resp:
            raw = resp.read().decode("utf-8")
            data = json.loads(raw) if raw else {}
            meta = {
                "url": url,
                "status_code": int(resp.status),
                "elapsed_ms": int((time.perf_counter() - t0) * 1000),
                "reachable": True,
            }
            return data, meta
    except HTTPError as exc:
        raw = exc.read().decode("utf-8") if exc.fp else ""
        data = {"error": raw or str(exc)}
        meta = {
            "url": url,
            "status_code": int(exc.code),
            "elapsed_ms": int((time.perf_counter() - t0) * 1000),
            "reachable": False,
        }
        return data, meta
    except (URLError, TimeoutError, OSError, ValueError) as exc:
        data = {"error": str(exc)}
        meta = {
            "url": url,
            "status_code": None,
            "elapsed_ms": int((time.perf_counter() - t0) * 1000),
            "reachable": False,
        }
        return data, meta


def parse_ts(line: str) -> str:
    match = TS_RE.search(line)
    return match.group(1) if match else ""


def parse_fields(line: str) -> dict[str, str]:
    return {key: val.rstrip(",") for key, val in KV_RE.findall(line)}


def event_kind(line: str) -> str:
    text = line.lower()
    if "manual_seal" in text:
        return "manual_seal"
    if "cluster_attest" in text or "attest accepted" in text or "attest_ready" in text:
        return "cluster_attest"
    if "cluster_gate" in text or "quorum_timeout" in text or "gate_wait" in text:
        return "cluster_gate"
    if "seal_suppression_summary" in text or "seal_suppressed" in text:
        return "seal_suppress"
    if "waiting_for_attester" in text or "sync progress" in text or "continuity_break" in text or "sync_live" in text:
        return "sync"
    if "clusterpropose" in text or "cluster propose sent" in text or "binding_mismatch" in text:
        return "peer_cluster"
    if "sealed height=" in text:
        return "sealed"
    return "other"


def parse_line(line: str) -> dict[str, Any] | None:
    line = line.rstrip("\n")
    if not line.strip():
        return None
    kind = event_kind(line)
    fields = parse_fields(line)
    event = {
        "event_id": hashlib.blake2s(line.encode("utf-8"), digest_size=8).hexdigest(),
        "ts_log": parse_ts(line),
        "level": "ERROR" if " error " in f" {line.lower()} " else ("WARN" if " warn " in f" {line.lower()} " else "INFO"),
        "kind": kind,
        "fields": fields,
    }
    if kind == "other":
        event["raw"] = line[:500]
    return event


def file_spec(path: str | None, before: int = 0) -> LogSpec:
    if not path:
        return LogSpec(path=None, offset=0, size=0)
    p = Path(path)
    try:
        size = p.stat().st_size
    except FileNotFoundError:
        return LogSpec(path=p, offset=0, size=0)
    return LogSpec(path=p, offset=min(before, size), size=size)


def seed_offset(path: str | None, stored: int) -> int:
    if not path:
        return 0
    p = Path(path)
    try:
        size = p.stat().st_size
    except FileNotFoundError:
        return 0
    if stored <= 0:
        return size
    return min(stored, size)


def read_window(spec: LogSpec) -> dict[str, Any]:
    if not spec.path or not spec.path.is_file():
        return {
            "log_path": str(spec.path) if spec.path else None,
            "byte_offset_before": spec.offset,
            "byte_offset_after": spec.offset,
            "lines_read": 0,
            "events": [],
        }
    after = spec.path.stat().st_size
    start = min(spec.offset, after)
    with spec.path.open("rb") as fh:
        fh.seek(start)
        data = fh.read(after - start)
    text = data.decode("utf-8", errors="replace")
    events = []
    for line in text.splitlines():
        event = parse_line(line)
        if event:
            events.append(event)
    return {
        "log_path": str(spec.path),
        "byte_offset_before": start,
        "byte_offset_after": after,
        "lines_read": len(text.splitlines()),
        "events": events,
    }


def load_state() -> dict[str, Any]:
    return read_json(STATE_PATH)


def save_state(payload: dict[str, Any]) -> None:
    write_json(STATE_PATH, payload)


def state_offset(state: dict[str, Any], kind: str, path: str | None) -> int:
    if not path:
        return 0
    slot = state.get(kind, {})
    if isinstance(slot, dict) and slot.get("path") == path:
        try:
            return int(slot.get("offset", 0))
        except (TypeError, ValueError):
            return 0
    return 0


def summarize(events: list[dict[str, Any]]) -> str:
    counts: dict[str, int] = {}
    for event in events:
        kind = str(event.get("kind", "other"))
        counts[kind] = counts.get(kind, 0) + 1
    if not counts:
        return "no new log events"
    parts = ", ".join(f"{kind}={count}" for kind, count in sorted(counts.items()))
    return f"captured events: {parts}"


def merge_events(*windows: dict[str, Any]) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for window in windows:
        events.extend(window.get("events", []))
    return events


def make_doc(
    cmd: str,
    rpc: dict[str, Any] | None,
    rpc_meta: dict[str, Any] | None,
    window: dict[str, Any],
    warnings: list[str] | None = None,
    step: str | None = None,
    ok: bool = True,
) -> dict[str, Any]:
    events = merge_events(window.get("proposer", {}), window.get("attester", {}))
    return {
        "ok": ok,
        "cmd": cmd,
        "step": step,
        "ts_utc": utc_now(),
        "duration_ms": window.get("duration_ms", 0),
        "rpc": rpc or {},
        "rpc_meta": rpc_meta or {},
        "window": window,
        "summary": summarize(events),
        "warnings": warnings or [],
    }


def capture_window(before: dict[str, Any], after: dict[str, Any], paths: dict[str, str | None]) -> dict[str, Any]:
    proposer_spec = file_spec(paths.get("proposer"), before.get("proposer", 0))
    attester_spec = file_spec(paths.get("attester"), before.get("attester", 0))
    proposer_window = read_window(proposer_spec)
    attester_window = read_window(attester_spec)
    return {
        "t0_ms": before.get("t0_ms", now_ms()),
        "t1_ms": after.get("t1_ms", now_ms()),
        "duration_ms": max(0, int(after.get("t1_ms", now_ms()) - before.get("t0_ms", now_ms()))),
        "proposer": proposer_window,
        "attester": attester_window,
    }


def run_cmd(args: argparse.Namespace, cmd: str, payload: dict[str, Any] | None = None, path: str = "/v1/lab/seal/status") -> dict[str, Any]:
    discovered = discover_logs(REPO)
    paths = {kind: info["path"] for kind, info in discovered["logs"].items()}
    state = load_state()
    before = {
        "t0_ms": now_ms(),
        "proposer": seed_offset(paths.get("proposer"), state_offset(state, "proposer", paths.get("proposer"))),
        "attester": seed_offset(paths.get("attester"), state_offset(state, "attester", paths.get("attester"))),
    }
    rpc = {}
    rpc_meta = {"reachable": False, "url": args.rpc_url, "status_code": None, "elapsed_ms": 0}
    if cmd != "discover":
        rpc, rpc_meta = rpc_request(args.rpc_url, "GET" if cmd == "status" else "POST", path, payload, timeout=args.timeout_sec)
    after = {"t1_ms": now_ms()}
    window = capture_window(before, after, paths)
    updated = {
        "proposer": {"path": paths.get("proposer"), "offset": window["proposer"]["byte_offset_after"]},
        "attester": {"path": paths.get("attester"), "offset": window["attester"]["byte_offset_after"]},
    }
    save_state(updated)
    doc = make_doc(cmd, rpc, rpc_meta, window, warnings=discovered["warnings"], step=getattr(args, "step", None), ok=bool(rpc_meta.get("reachable", cmd == "discover")))
    doc["discover"] = discovered["logs"]
    return doc


def cmd_status(args: argparse.Namespace) -> dict[str, Any]:
    return run_cmd(args, "status", path="/v1/lab/seal/status")


def normalize_seal_control_mode(mode: str) -> str:
    """API expects snake_case (manual_rpc); CLI may accept manual-rpc alias."""
    return mode.replace("-", "_")


def cmd_control(args: argparse.Namespace) -> dict[str, Any]:
    payload = {"mode": normalize_seal_control_mode(args.mode)}
    if args.verbose_default is not None:
        payload["verbose_default"] = args.verbose_default
    return run_cmd(args, "control", payload=payload, path="/v1/lab/seal/control")


def cmd_step(args: argparse.Namespace) -> dict[str, Any]:
    payload: dict[str, Any] = {"step": args.step}
    if args.verbose:
        payload["verbose"] = True
    if args.timeout_ms is not None:
        payload["timeout_ms"] = args.timeout_ms
    if args.target_h is not None:
        payload["target_h"] = args.target_h
    return run_cmd(args, "step", payload=payload, path="/v1/lab/seal/step")


def cmd_discover(args: argparse.Namespace) -> dict[str, Any]:
    doc = run_cmd(args, "discover")
    doc["rpc_meta"] = {"url": args.rpc_url, "reachable": False, "status_code": None, "elapsed_ms": 0}
    if args.probe_rpc:
        _, rpc_meta = rpc_request(args.rpc_url, "GET", "/v1/lab/seal/status", timeout=args.timeout_sec)
        doc["rpc_meta"] = rpc_meta
        doc["ok"] = bool(rpc_meta.get("reachable"))
    return doc


def cmd_watch_tick(args: argparse.Namespace) -> dict[str, Any]:
    return run_cmd(args, "watch_tick", path="/v1/lab/seal/status")


def emit(doc: dict[str, Any], out: str | None = None, pretty: bool = False) -> None:
    text = json.dumps(doc, indent=2 if pretty else None, sort_keys=pretty)
    print(text)
    if out:
        Path(out).parent.mkdir(parents=True, exist_ok=True)
        Path(out).write_text(text + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="CY lab seal console")
    parser.add_argument("--rpc-url", default=DEFAULT_RPC)
    parser.add_argument("--timeout-sec", type=float, default=120.0)
    parser.add_argument("--out", default=None, help="Optional file to mirror the JSON result")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print JSON for one-shot commands")

    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("discover")

    p_status = sub.add_parser("status")
    p_status.set_defaults(func=cmd_status)

    p_control = sub.add_parser("control")
    p_control.add_argument("--mode", required=True, choices=["auto", "manual_rpc", "manual-rpc"])
    p_control.add_argument("--verbose-default", action="store_true", dest="verbose_default")
    p_control.add_argument("--no-verbose-default", action="store_false", dest="verbose_default")
    p_control.set_defaults(verbose_default=None, func=cmd_control)

    p_step = sub.add_parser("step")
    p_step.add_argument("step", choices=["preflight", "lease", "propose", "gate_poll", "gate_wait", "seal_commit", "step_all"])
    p_step.add_argument("--verbose", action="store_true")
    p_step.add_argument("--timeout-ms", type=int, default=None)
    p_step.add_argument("--target-h", type=int, default=None)
    p_step.set_defaults(func=cmd_step)

    p_tick = sub.add_parser("watch_tick")
    p_tick.set_defaults(func=cmd_watch_tick)

    p_watch = sub.add_parser("watch")
    p_watch.add_argument("--interval-ms", type=int, default=500)
    p_watch.add_argument("--max-ticks", type=int, default=120)
    p_watch.add_argument("--verbose", action="store_true")
    p_watch.add_argument("--out", default=None)

    args = parser.parse_args(argv)
    if args.cmd == "discover":
        doc = cmd_discover(args)
        emit(doc, args.out, args.pretty)
        return 0 if doc.get("ok", False) else 1

    if args.cmd == "watch":
        for tick in range(max(0, args.max_ticks)):
            doc = cmd_watch_tick(args)
            doc["tick"] = tick
            doc["cmd"] = "watch"
            emit(doc, args.out, pretty=False)
            if tick + 1 < args.max_ticks:
                time.sleep(max(1, args.interval_ms) / 1000.0)
        return 0

    doc = args.func(args)
    emit(doc, args.out, args.pretty)
    return 0 if doc.get("ok", False) else 1


if __name__ == "__main__":
    raise SystemExit(main())