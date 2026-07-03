#!/usr/bin/env python3
"""Benchmark pi + Ollama + pi-ollama-tool-fix extension: accuracy and latency on scoped file edits.

Requires:
  - Ollama on http://127.0.0.1:11434
  - pi CLI on PATH
  - pi packages: pi-ollama-tool-fix, pi-mcp-adapter (see ~/.pi/agent/settings.json)
  - ~/.pi/agent/models.json provider `default` -> Ollama /v1

Run:
  python scripts/pi_ollama_tool_benchmark.py
  python scripts/pi_ollama_tool_benchmark.py --quick
  python scripts/pi_ollama_tool_benchmark.py --json-out .cqds/tmp/pi-bench-last.json
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OLLAMA = "http://127.0.0.1:11434"
DEFAULT_PROVIDER = os.environ.get("PI_BENCH_PROVIDER", "default")
DEFAULT_MODEL = os.environ.get("PI_BENCH_MODEL", "qwen2.5-coder:14b")
MCP_CONFIG = ROOT / ".mcp.json"
PRINT_PROMPT = "Execute the instructions in the provided context."


def _resolve_bash() -> str:
    for key in ("BASH", "SHELL"):
        val = (os.environ.get(key) or "").strip()
        if val and Path(val).is_file():
            return val
    for candidate in (r"C:\Apps\Git\bin\bash.exe", r"C:\Apps\Git\usr\bin\bash.exe"):
        if Path(candidate).is_file():
            return candidate
    return "bash"


def _argv_for_pi_path(path: Path) -> list[str]:
    """Windows: run pi shell shim via bash so stdin + MCP tool loop work."""
    if sys.platform == "win32":
        bash = _resolve_bash()
        if path.suffix.lower() == ".cmd":
            sibling = path.with_name("pi")
            if sibling.is_file():
                return [bash, str(sibling.resolve())]
        if path.name == "pi" and not path.suffix:
            return [bash, str(path.resolve())]
    return [str(path)]


def _resolve_pi_argv() -> list[str]:
    override = (os.environ.get("PI_BIN") or "").strip()
    if override:
        return _argv_for_pi_path(Path(override))
    found = shutil.which("pi")
    if found:
        return _argv_for_pi_path(Path(found))
    if sys.platform == "win32":
        for name in ("pi.cmd", "pi.exe", "pi.CMD"):
            found = shutil.which(name)
            if found:
                return _argv_for_pi_path(Path(found))
    return ["pi"]

@dataclass
class TaskResult:
    task_id: str
    title: str
    ok: bool
    score: float
    elapsed_sec: float
    efficiency: float
    detail: str
    expected: str
    actual: str
    returncode: int
    stderr_tail: str = ""


@dataclass
class BenchReport:
    started_at: float
    finished_at: float
    provider: str
    model: str
    ollama_url: str
    work_dir: str
    results: list[TaskResult] = field(default_factory=list)

    @property
    def elapsed_sec(self) -> float:
        return max(0.0, self.finished_at - self.started_at)

    @property
    def mean_score(self) -> float:
        if not self.results:
            return 0.0
        return sum(r.score for r in self.results) / len(self.results)

    @property
    def pass_rate(self) -> float:
        if not self.results:
            return 0.0
        return sum(1 for r in self.results if r.ok) / len(self.results)

    @property
    def mean_task_sec(self) -> float:
        if not self.results:
            return 0.0
        return sum(r.elapsed_sec for r in self.results) / len(self.results)

    @property
    def mean_efficiency(self) -> float:
        if not self.results:
            return 0.0
        return sum(r.efficiency for r in self.results) / len(self.results)


def _http_ok(url: str, timeout: float = 3.0) -> bool:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:
            return 200 <= int(resp.status) < 500
    except (URLError, urllib.error.HTTPError, TimeoutError, OSError):
        return False


def _read_text(path: Path) -> str:
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8", errors="replace")


def _sandwich(task_xml: str) -> str:
    return (
        '<cqds_cli_sandwich version="1">\n'
        f"<subagent_task>{task_xml}</subagent_task>\n"
        "</cqds_cli_sandwich>"
    )


def _run_pi(
    *,
    work_dir: Path,
    provider: str,
    model: str,
    prompt_text: str,
    timeout_sec: int,
) -> subprocess.CompletedProcess[str]:
    cmd = [
        *_resolve_pi_argv(),
        "--thinking",
        "off",
        "--provider",
        provider,
        "--model",
        model,
        "--no-session",
        "--mcp-config",
        str(MCP_CONFIG.resolve()),
        "--exclude-tools",
        "bash",
        "--print",
        PRINT_PROMPT,
    ]
    env = os.environ.copy()
    env.setdefault("CQDS_PROJECT_ROOT", str(ROOT.resolve()))
    return subprocess.run(
        cmd,
        input=prompt_text,
        text=True,
        capture_output=True,
        cwd=str(work_dir),
        env=env,
        timeout=timeout_sec,
        shell=False,
    )


def _score_exact(actual: str, expected: str) -> tuple[float, str, bool]:
    if actual == expected:
        return 1.0, "exact match", True
    norm_a = actual.replace("\r\n", "\n").strip()
    norm_e = expected.replace("\r\n", "\n").strip()
    if norm_a == norm_e:
        return 1.0, "exact match (normalized EOL)", True
    if norm_e in norm_a and len(norm_a) <= len(norm_e) + 8:
        return 0.7, "expected content present with minor extra text", False
    if norm_e.splitlines()[0] in norm_a and len(norm_e.splitlines()) == 1:
        return 0.4, "first line matched only", False
    return 0.0, "content mismatch", False


def _build_tasks(work: Path) -> list[dict[str, Any]]:
    target = work / "edit_me.txt"
    decoy = work / "do_not_touch.txt"

    def reset_base() -> None:
        work.mkdir(parents=True, exist_ok=True)
        target.write_text("alpha\nbeta\ngamma\n", encoding="utf-8", newline="\n")
        decoy.write_text("KEEP\n", encoding="utf-8", newline="\n")

    tasks: list[dict[str, Any]] = []

    tasks.append(
        {
            "task_id": "write_replace_all",
            "title": "built-in write: replace whole file",
            "setup": reset_base,
            "prompt": (
                f'Allowlisted file only: {target.resolve()}. '
                "Use the write tool (not bash). Replace entire file with exactly two lines:\n"
                "REPLACED\nEND\n"
                "Reply DONE when finished."
            ),
            "expected": "REPLACED\nEND\n",
            "verify_path": target,
            "decoy_path": decoy,
            "decoy_expected": "KEEP\n",
        }
    )

    tasks.append(
        {
            "task_id": "git_write_replace_all",
            "title": "MCP git_write_file: replace whole file",
            "setup": reset_base,
            "prompt": (
                f'Allowlisted file only: {target.resolve()}. '
                "Use git_write_file MCP tool only. Write exactly:\n"
                "GIT_OK\nSECOND\n"
                "Reply DONE."
            ),
            "expected": "GIT_OK\nSECOND\n",
            "verify_path": target,
            "decoy_path": decoy,
            "decoy_expected": "KEEP\n",
        }
    )

    tasks.append(
        {
            "task_id": "git_write_line2",
            "title": "MCP git_write_file: change line 2 only",
            "setup": reset_base,
            "prompt": (
                f'Allowlisted file only: {target.resolve()}. '
                "Use git_write_file with insert_at line_start=2 line_end=2 to replace line 2 only. "
                "New line 2 text must be exactly: BETA2\n"
                "Line 1 must stay alpha, line 3 must stay gamma. Reply DONE."
            ),
            "expected": "alpha\nBETA2\ngamma\n",
            "verify_path": target,
            "decoy_path": decoy,
            "decoy_expected": "KEEP\n",
        }
    )

    tasks.append(
        {
            "task_id": "session_replace_line2",
            "title": "MCP session_open/cmd: replace line 2",
            "setup": reset_base,
            "prompt": (
                f'Allowlisted file only: {target.resolve()}. '
                "Use session_open then session_cmd op replace_range on line 2 only. "
                "Replace line 2 with exactly: SESSION2\n"
                "Do not change lines 1 or 3. Reply DONE."
            ),
            "expected": "alpha\nSESSION2\ngamma\n",
            "verify_path": target,
            "decoy_path": decoy,
            "decoy_expected": "KEEP\n",
        }
    )

    tasks.append(
        {
            "task_id": "scope_lock",
            "title": "scope: edit target only, decoy unchanged",
            "setup": reset_base,
            "prompt": (
                f'Allowlisted file ONLY: {target.resolve()}. '
                f'Never edit {decoy.resolve()}. '
                "Use any write/edit MCP tool. Set target file content to exactly:\n"
                "SCOPE_OK\n"
                "Reply DONE."
            ),
            "expected": "SCOPE_OK\n",
            "verify_path": target,
            "decoy_path": decoy,
            "decoy_expected": "KEEP\n",
        }
    )

    return tasks


def _run_task(
    spec: dict[str, Any],
    *,
    provider: str,
    model: str,
    timeout_sec: int,
) -> TaskResult:
    setup: Callable[[], None] = spec["setup"]
    setup()
    verify_path: Path = spec["verify_path"]
    expected: str = spec["expected"]
    decoy_path: Path | None = spec.get("decoy_path")
    decoy_expected: str = str(spec.get("decoy_expected") or "")

    prompt = _sandwich(str(spec["prompt"]))
    t0 = time.monotonic()
    try:
        proc = _run_pi(
            work_dir=verify_path.parent,
            provider=provider,
            model=model,
            prompt_text=prompt,
            timeout_sec=timeout_sec,
        )
        rc = int(proc.returncode or 0)
        stderr_tail = (proc.stderr or "")[-800:]
    except subprocess.TimeoutExpired as exc:
        elapsed = time.monotonic() - t0
        return TaskResult(
            task_id=str(spec["task_id"]),
            title=str(spec["title"]),
            ok=False,
            score=0.0,
            elapsed_sec=elapsed,
            efficiency=0.0,
            detail=f"timeout after {timeout_sec}s",
            expected=expected,
            actual=_read_text(verify_path),
            returncode=-1,
            stderr_tail=str(exc)[-800:],
        )

    elapsed = time.monotonic() - t0
    actual = _read_text(verify_path)
    score, detail, ok = _score_exact(actual, expected)

    if ok and decoy_path is not None and decoy_expected:
        decoy_actual = _read_text(decoy_path)
        d_score, d_detail, d_ok = _score_exact(decoy_actual, decoy_expected)
        if not d_ok:
            ok = False
            score = min(score, 0.2)
            detail = f"{detail}; decoy violated: {d_detail}"

    if rc != 0 and ok:
        detail = f"{detail}; pi exit={rc}"
        ok = False
        score = min(score, 0.5)

    efficiency = score / max(elapsed, 0.5)
    return TaskResult(
        task_id=str(spec["task_id"]),
        title=str(spec["title"]),
        ok=ok,
        score=score,
        elapsed_sec=round(elapsed, 2),
        efficiency=round(efficiency, 4),
        detail=detail,
        expected=expected,
        actual=actual,
        returncode=rc,
        stderr_tail=stderr_tail,
    )


def _print_report(report: BenchReport) -> None:
    print(
        f"pi+ollama tool benchmark  provider={report.provider}  model={report.model}  "
        f"ollama={report.ollama_url}"
    )
    print(f"work_dir={report.work_dir}")
    print("-" * 72)
    for row in report.results:
        mark = "PASS" if row.ok else "FAIL"
        print(
            f"[{mark}] {row.task_id:24} score={row.score:.2f} "
            f"time={row.elapsed_sec:.1f}s eff={row.efficiency:.3f}  {row.title}"
        )
        if not row.ok:
            print(f"       {row.detail}")
            print(f"       expected: {row.expected!r}")
            print(f"       actual:   {row.actual!r}")
    print("-" * 72)
    print(
        f"summary: pass_rate={report.pass_rate:.0%} mean_score={report.mean_score:.2f} "
        f"mean_task_sec={report.mean_task_sec:.1f} mean_efficiency={report.mean_efficiency:.3f} "
        f"total_sec={report.elapsed_sec:.1f}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--quick", action="store_true", help="Run first 2 tasks only")
    parser.add_argument("--provider", default=DEFAULT_PROVIDER)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--ollama-url", default=os.environ.get("OLLAMA_URL", DEFAULT_OLLAMA))
    parser.add_argument("--timeout", type=int, default=180, help="Per-task pi timeout seconds")
    parser.add_argument("--work-dir", default=str(ROOT / ".cqds" / "tmp" / "pi-tool-bench"))
    parser.add_argument("--json-out", default="", help="Write full JSON report to path")
    parser.add_argument("--keep-workdir", action="store_true")
    args = parser.parse_args()

    pi_argv = _resolve_pi_argv()
    pi_probe = pi_argv[-1]
    if shutil.which(pi_probe) is None and not Path(pi_probe).is_file():
        print(f"ERROR: pi not found (PI_BIN={os.environ.get('PI_BIN', '')})", file=sys.stderr)
        return 2
    if not MCP_CONFIG.is_file():
        print(f"ERROR: missing MCP config {MCP_CONFIG}", file=sys.stderr)
        return 2
    if not _http_ok(args.ollama_url.rstrip("/") + "/api/tags"):
        print(f"ERROR: ollama not reachable at {args.ollama_url}", file=sys.stderr)
        return 2

    work = Path(args.work_dir).resolve()
    if work.exists() and not args.keep_workdir:
        shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True, exist_ok=True)

    tasks = _build_tasks(work)
    if args.quick:
        tasks = tasks[:2]

    started = time.time()
    results: list[TaskResult] = []
    for spec in tasks:
        print(f"running {spec['task_id']}...", flush=True)
        results.append(
            _run_task(spec, provider=str(args.provider), model=str(args.model), timeout_sec=int(args.timeout))
        )

    report = BenchReport(
        started_at=started,
        finished_at=time.time(),
        provider=str(args.provider),
        model=str(args.model),
        ollama_url=str(args.ollama_url),
        work_dir=str(work),
        results=results,
    )
    _print_report(report)

    if args.json_out:
        out = Path(args.json_out)
        out.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            **{
                k: getattr(report, k)
                for k in ("started_at", "finished_at", "provider", "model", "ollama_url", "work_dir")
            },
            "pass_rate": report.pass_rate,
            "mean_score": report.mean_score,
            "mean_task_sec": report.mean_task_sec,
            "mean_efficiency": report.mean_efficiency,
            "elapsed_sec": report.elapsed_sec,
            "results": [asdict(r) for r in report.results],
        }
        out.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"json: {out}")

    return 0 if report.pass_rate >= 1.0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
