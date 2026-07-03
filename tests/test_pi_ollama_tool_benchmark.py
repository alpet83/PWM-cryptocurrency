"""Integration benchmark: pi + Ollama tool-fix proxy (optional, needs live stack)."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
BENCH = ROOT / "scripts" / "pi_ollama_tool_benchmark.py"


@pytest.mark.integration
def test_pi_ollama_tool_benchmark_quick() -> None:
    if os.environ.get("PI_BENCH_RUN") != "1":
        pytest.skip("set PI_BENCH_RUN=1 to run live pi+ollama benchmark")
    if not BENCH.is_file():
        pytest.skip("benchmark script missing")

    proc = subprocess.run(
        [
            sys.executable,
            str(BENCH),
            "--quick",
            "--json-out",
            str(ROOT / ".cqds" / "tmp" / "pi-bench-pytest.json"),
        ],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        timeout=600,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
