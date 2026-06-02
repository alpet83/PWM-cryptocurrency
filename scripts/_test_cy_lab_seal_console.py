#!/usr/bin/env python3
"""Unit tests for cy_lab_seal_console parser and log discovery."""

from __future__ import annotations

import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "cy_lab_seal_console.py"
SPEC = importlib.util.spec_from_file_location("cy_lab_seal_console", SCRIPT)
assert SPEC and SPEC.loader
MOD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MOD
SPEC.loader.exec_module(MOD)


class SealConsoleTests(unittest.TestCase):
    def test_parse_manual_seal_line(self) -> None:
        line = "[12:34:56.789] INFO manual_seal step=propose target_h=42 phase=wire elapsed_ms=5"
        event = MOD.parse_line(line)
        self.assertIsNotNone(event)
        assert event is not None
        self.assertEqual(event["kind"], "manual_seal")
        self.assertEqual(event["fields"]["step"], "propose")
        self.assertEqual(event["fields"]["target_h"], "42")

    def test_parse_attest_line(self) -> None:
        line = "[12:34:56.790] INFO cluster_attest_ready live_synced_attesters=1"
        event = MOD.parse_line(line)
        self.assertIsNotNone(event)
        assert event is not None
        self.assertEqual(event["kind"], "cluster_attest")
        self.assertEqual(event["fields"]["live_synced_attesters"], "1")

    def test_discover_picks_latest_log(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old = root / "logs" / "2026-05-30" / "pwmd-cy-proposer-old.log"
            new = root / "logs" / "2026-05-31" / "pwmd-cy-proposer-new.log"
            old.parent.mkdir(parents=True, exist_ok=True)
            new.parent.mkdir(parents=True, exist_ok=True)
            old.write_text("old\n", encoding="utf-8")
            new.write_text("new\n", encoding="utf-8")
            os.utime(old, (1_000_000_000, 1_000_000_000))
            os.utime(new, (1_000_000_001, 1_000_000_001))

            discovered = MOD.discover_logs(root)
            self.assertEqual(discovered["logs"]["proposer"]["path"], str(new.resolve()))

    def test_tail_window_reads_events(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "pwmd-cy-attester-tail.log"
            first = "[12:34:56.000] INFO waiting_for_attester live_synced_attesters=0\n"
            second = "[12:34:57.000] INFO sealed height=43\n"
            path.write_text(first + second, encoding="utf-8")
            spec = MOD.LogSpec(path=path, offset=len(first.encode("utf-8")), size=path.stat().st_size)
            window = MOD.read_window(spec)
            self.assertEqual(window["events"][0]["kind"], "sealed")

    def test_normalize_seal_control_mode(self) -> None:
        self.assertEqual(MOD.normalize_seal_control_mode("manual-rpc"), "manual_rpc")
        self.assertEqual(MOD.normalize_seal_control_mode("manual_rpc"), "manual_rpc")
        self.assertEqual(MOD.normalize_seal_control_mode("auto"), "auto")

    def test_summary_counts(self) -> None:
        doc = MOD.make_doc(
            "step",
            {"ok": True},
            {"reachable": True},
            {
                "duration_ms": 5,
                "proposer": {"events": [{"kind": "manual_seal"}]},
                "attester": {"events": [{"kind": "cluster_attest"}]},
            },
        )
        self.assertTrue(doc["ok"])
        self.assertIn("manual_seal=1", doc["summary"])
        self.assertIn("cluster_attest=1", doc["summary"])


if __name__ == "__main__":
    unittest.main(verbosity=2)