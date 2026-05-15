# Slice 6 (V2-8) testing report — Wave A / `--debug-stop-height`

**Validated commit:** `e74dbf355f1f98ad30e7858505f8565f14d48cda` (`e74dbf3`, same as `HEAD` at validation time).

**Tester:** pwm-testing (delegated slice validation).

---

## Commands and results

| Check | Command / action | Result | Notes |
|--------|-------------------|--------|--------|
| Preflight target size | `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1` | PASS | ~216 MiB `target/debug` vs threshold 4096 MiB |
| Build gate | `cargo check -p pwmd` | PASS | Completed in ~0.3 s (incremental) |
| Binaries for harness | `cargo build -p pwmd -p pwm-cli` | PASS | Artifacts resolved via workspace target root |
| Snapshot bench compile | `cargo bench -p pwmd --bench snapshot_load --no-run` | PASS | ~30 s compile to bench profile |
| Prod naming (≤4 segments) | `python scripts/check_rust_fn_name_segments.py` on touched `pwmd` `src/*.rs` | PASS | Empty `violations` for listed files |

### Wave A harness

| Run | Command | Exit | Approx. duration | Hang watchdog |
|-----|---------|------|-------------------|---------------|
| 1 | `python scripts/wave_a_same_shard_stop.py` | 0 | ~403 s | Not triggered |
| 2 | `python scripts/wave_a_same_shard_stop.py` | 0 | ~403 s | Not triggered |
| 3 | `python scripts/wave_a_same_shard_stop.py --keep-artifacts` | 0 | ~403 s | Logs under temp `pwm_wave_a_*` |

All runs emitted stderr note: `tip_hash differs across nodes; account/canonical invariants still match`.

---

## `--debug-stop-height` controlled verification

Artifacts from run 3: `pwm_wave_a_8ogvel5h/logs/node1.log` and `node2.log` on host temp.

Observed sequence (both nodes):

1. WARN: `debug-stop-height active (test-only): node will trigger graceful stop at height>=200`
2. INFO: `debug-stop-height reached; graceful shutdown triggered at height=200 stop_h=200`

Conclusion: test-only flag is wired correctly; graceful shutdown aligns with manifest `canonical_h`/stop target 200 observed in Wave reports.

---

## Invariant quality (assessment)

**Strong (enforced, exit non-zero on failure):**

- Both `pwmd` children exit code 0 after stop.
- `canonical_h` matches across nodes and is ≥ `stop_height_target` (200).
- `checkpoint_height` in `pwm-data.json` matches each node’s manifest `canonical_h`.
- Epoch manifest: same `epochs` length, `epoch_span`, `schema_v`; same **last epoch file name**.
- Key accounts (sender/receiver): `balance_pwm`, `nonce`, `initialized` match across snapshots.

**Weak / informational only (harness does not fail):**

- `tip_hash_equal`: consistently `false` in observed runs; stderr documents this.
- `last_epoch_hash_equal`: consistently `false` while `last_epoch_file` name matches (`block_e0.json` in runs above). Byte-level epoch payload differs per node but is not asserted.

**Gap vs ticket wording:** ticket `artifacts.wave_a.checks` mentions “manifest/epoch file consistency by hash/height”. Height and manifest structure hold; **per-file epoch body hash parity is reported but not required for PASS.** Recommend pwm-review: either tighten the harness (fail if `last_epoch_hash_equal == false`) or narrow the acceptance text to match intentional scope (canonical + account parity + structural manifest match).

---

## Runbook (`docs/runbook-same-shard-sync-v1.md` §6)

Section 6 matches the harness entrypoint and stop-height rule; it correctly distinguishes `tip_hash_equal` as diagnostic. Recommend one-line clarification that `last_epoch_hash_equal` in the JSON may be false while exit code stays 0, so operators do not confuse “printed field” with “release gate invariant” unless policy is updated.

---

## Reproducibility

- Structural outcomes reproduced across runs: `stop_height_target` 200, `snap_chk_blk_iv` 100, same fee model effects on balances (`999989` / `10`), same manifest row counts / span / schema.
- Cryptographic fingerprints (`tip_hash`, `manifest_sha256`, epoch file SHA-256) **vary run-to-run** in this environment; relying on literal hash equality without defining determinism assumptions would be brittle. For CI, prefer asserting the enforced rows above unless product guarantees identical epoch serialization across peers.

---

## Machine handoff (`pwm-testing`)

```
agent: pwm-testing
result: PARTIAL
rationale: Wave A exits 0 on ≥2 runs; `--debug-stop-height` confirmed in logs; prod naming clean; checkpoint/account/manifest-structure invariants solid. Epoch file byte-hash and tip_hash divergence are visible but non-failing — misaligned with strict reading of ticket “hash” consistency unless scope is narrowed.
artifacts:
  - docs/reviews/20260508-v2-8-slice6-testing.md
  - tasks/20260508-v2-sprint8-slice6-automated-waves.json (delegation + artifacts.testing_md)
commands:
  - preflight_target_debug.ps1 — PASS (~216 MiB reported)
  - cargo check -p pwmd — PASS
  - cargo build -p pwmd -p pwm-cli — PASS
  - cargo bench -p pwmd --bench snapshot_load --no-run — PASS
  - wave_a_same_shard_stop.py ×2 + --keep-artifacts — PASS (exit 0)
  - check_rust_fn_name_segments.py (touched pwmd sources) — PASS
cleanup: no stray `pwmd` after runs; temp wave dirs: run 3 left `F:\Temp\pwm_wave_a_8ogvel5h` (`--keep-artifacts`); runs 1–2 removed temp dirs on success
preflight_target_debug: powershell script; removed: no; size under threshold
snapshot_benches: compiled_only PASS (--no-run)
hang_watchdog: not triggered
token_usage: { "source": "estimate", "input": null, "output": null, "total": 14000, "confidence": "low" }
```

Pending: **pwm-review** (ticket remains `in_progress`).
