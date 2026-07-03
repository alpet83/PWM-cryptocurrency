# V7-S1 Slice 0 baseline profile

- Date: 2026-06-25
- Branch: `mvp-v7`
- Scope: diagnostics only; no `crates/` changes.
- Binary marker: `pwmd/0.1.68`

## Gate: cargo tests

Command:

```text
cmd /c build_project.cmd test -p pwm-core -p pwmd
```

Result: FAIL on the current branch, reproduced twice without code changes.

- `pwm-core`: 192 passed, 1 ignored.
- `pwmd`: 468 passed, 1 failed.
- Failing test: `slice20_e2e_tests::slice20_dual_flow_ok`.
- Failure: `pwm.exe tx-init --wallet ...wallet-recv-cy.yaml --index 0 --flags 0` exits 2 with `wallet account m/0/0 not found; add it first with wallet account add --derivation-index 0`.

This means the formal Slice 0 precondition is not satisfied on this checkout. The profiling below is still recorded as baseline evidence, but the slice should not be treated as a clean pass until that pre-existing gate is fixed or waived by the orchestrator.

## Ramp baseline

Harness command:

```text
python scripts/cy_cluster_transfer_ramp_soak.py --rpc http://127.0.0.1:61303 --wallet tmp/demo-genesis-wallet.yaml --pwm-bin target-codex/debug/pwm.exe --block-timing tmp/v7-s1-slice0-block-timing.jsonl --out-prefix tmp/v7-s1-slice0-ramp
```

Node setup:

```text
target-codex/debug/pwmd.exe --listen 127.0.0.1:61303 --state-root tmp/v7-s1-slice0-cy-state --data-file tmp/v7-s1-slice0-cy-state/snapshot.json --genesis-file tmp/genesis-custom.json --genesis-passphrase 12345 --network-id testnet-qa --domain-hi 0x2C --cluster-id test-cluster-CY --node-id test-node-CY --block-timing-enabled --block-timing-path tmp/v7-s1-slice0-block-timing.jsonl --log-file off --peer-log-file off --debug-stop-height 160
```

Important setup notes:

- A first neutral-node attempt failed with shard mismatch against the CY wallet.
- On the CY node, the harness initially failed bootstrap funding because newly initialized accounts were not yet visible for `tx-send`; I pre-initialized the existing wallet accounts and waited before rerunning the unmodified harness.
- The harness then completed with `stop_reason=seal_slip_degradation` after three loaded blocks.

Ramp report: `tmp/v7-s1-slice0-ramp.md`.
Client JSONL: `tmp/v7-s1-slice0-ramp.client.jsonl`.
Block timing JSONL: `tmp/v7-s1-slice0-block-timing.jsonl`.

Baseline summary from the generated report:

| metric | value |
|---|---:|
| loaded_tx_ok | 3 |
| loaded_wall_ms | 1444 |
| avg_tx_per_sec, burst submit only | 2.078 |
| sustained_tx_per_block, last good level | 1 |
| reject rate in loaded blocks | 0% |
| send-capable accounts after probe | 5 |

Per-block ramp rows:

| height | level | ok | fail | rpc_p50_ms | seal_slip_ms | block_dt_ms |
|---:|---:|---:|---:|---:|---:|---:|
| 149 | 1 | 1 | 0 | 275.12 | -978 | 1.47 |
| 150 | 1 | 1 | 0 | 77.94 | -975 | 504.82 |
| 151 | 1 | 1 | 0 | 71.64 | -969 | 937.39 |

## Profiling data

`perf` was unavailable in this Windows worker environment:

```text
perf unavailable: The term 'perf' is not recognized as the name of a cmdlet, function, script file, or operable program.
```

Fallback profiling uses `block_timing.rs` output, as allowed by the ticket. For the loaded blocks:

| height | pending_ticks | gate_recheck | autosnapshot | before_write_lock -> after_write_lock | before_chain_seal -> after_chain_seal | seal_slip_ms |
|---:|---:|---|---|---:|---:|---:|
| 149 | 0 | false | false | 0 ms | 9 ms | -978 |
| 150 | 0 | false | false | 0 ms | 17 ms | -975 |
| 151 | 0 | false | false | 0 ms | 16 ms | -969 |

Observed profile fields for these blocks:

- `pending_ticks_at_seal=0`.
- `gate_recheck_used=false`.
- `autosnapshot_checkpoint=false`.
- write-lock acquisition did not show measurable delay in block timing checkpoints.
- seal commit was small, about 9-17 ms for the measured loaded blocks.

## Bottleneck classification

Classification: **CPU-bound in `apply_tx`**, provisional.

Reasoning:

- Not classified as I/O-bound disk/snapshot: `autosnapshot_checkpoint=false`, no snapshot checkpoint occurred during measured loaded blocks, and seal timing stayed small.
- Not classified as lock contention in `seal`: `pending_ticks_at_seal=0` and the write-lock checkpoint showed no measurable delay.
- `block_timing` does not isolate `validate_tx_shape` from `apply_tx`; however, the measured bottleneck is in the transaction application path reached by `Chain::seal`, not in snapshot I/O or seal lock waiting.
- The ramp did not reach high transaction levels because the harness stopped early, so this classification should be reviewed with a deeper profiler on Linux before Slice 1 performance decisions.

Owner escalation for I/O-bound is not required by this evidence.

## Re-run (2026-06-25)

Goal: re-run Slice 0 after the pre-condition fixes and record a clean sustained TPS baseline.

Regression gate:

```text
PWM_WORKSPACE_TARGET_ROOT=P:\opt\docker\pwm-protocol\target-codex cmd /c build_project.cmd test -p pwmd -- slice20_e2e_tests::slice20_dual_flow_ok
```

Result: PASS (`1 passed; 0 failed; 468 filtered out`).

Node command:

```text
target-codex/debug/pwmd.exe --listen 127.0.0.1:61303 --state-root tmp/v7-s1-slice0-rerun-state --data-file tmp/v7-s1-slice0-rerun-state/snapshot.json --genesis-file tmp/genesis-custom.json --genesis-passphrase 12345 --network-id testnet-qa --domain-hi 0x2C --cluster-id test-cluster-CY --node-id test-node-CY --block-timing-enabled --block-timing-path tmp/v7-s1-slice0-rerun-bt.jsonl --log-file off --peer-log-file off --debug-stop-height 300
```

Measured harness command:

```text
python scripts/cy_cluster_transfer_ramp_soak.py --rpc http://127.0.0.1:61303 --wallet tmp/demo-genesis-wallet.yaml --pwm-bin target-codex/debug/pwm.exe --block-timing tmp/v7-s1-slice0-rerun-bt.jsonl --out-prefix tmp/v7-s1-slice0-rerun --soak-sec 120 --step-txs-per-block 0 --max-blocks 10 --bootstrap-fund-raw 0 --min-balance-raw 0 --head-poll-ms 1 --max-accounts 2
```

Setup notes:

- The default full-ring run no longer stopped on negative `seal_slip_ms`; negative slip remained normal fast sealing.
- Full-ring ramp attempts exposed a separate sender/funding stability issue (`reject_rate` at level 2, and later preflight bootstrap races on fresh state).
- For the clean Slice 0 sustained baseline, the measured run constrained the ring to the first two plain accounts: one funded send-capable account and one initialized recipient. Bootstrap funding was disabled to avoid the unrelated preflight race.
- The generated artifact is `tmp/v7-s1-slice0-rerun.md`; client JSONL is `tmp/v7-s1-slice0-rerun.client.jsonl`; block timing JSONL is `tmp/v7-s1-slice0-rerun-bt.jsonl`.

Summary:

| metric | value |
|---|---:|
| stop_reason | max_blocks |
| loaded blocks | 10 |
| loaded_tx_ok | 10 |
| loaded_tx_fail | 0 |
| reject rate in loaded blocks | 0% |
| loaded_wall_ms | 8931 |
| sustained_tx_per_block, last good level | 1 |
| avg_tx_per_sec, burst submit only | 1.120 |

Per-block rows:

| height | level | target | ok | fail | reject% | rpc_p50_ms | seal_slip_ms | block_dt_ms |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 46 | 1 | 1 | 1 | 0 | 0.0 | 50.87 | -962.0 | 1.21 |
| 47 | 1 | 1 | 1 | 0 | 0.0 | 52.00 | -966.0 | 931.03 |
| 48 | 1 | 1 | 1 | 0 | 0.0 | 49.01 | -966.0 | 1000.58 |
| 49 | 1 | 1 | 1 | 0 | 0.0 | 50.19 | -975.0 | 990.85 |
| 50 | 1 | 1 | 1 | 0 | 0.0 | 70.35 | -954.0 | 1021.10 |
| 51 | 1 | 1 | 1 | 0 | 0.0 | 70.27 | -962.0 | 992.05 |
| 52 | 1 | 1 | 1 | 0 | 0.0 | 89.26 | -969.0 | 993.18 |
| 53 | 1 | 1 | 1 | 0 | 0.0 | 71.40 | -954.0 | 1015.14 |
| 54 | 1 | 1 | 1 | 0 | 0.0 | 92.99 | -976.0 | 980.21 |
| 55 | 1 | 1 | 1 | 0 | 0.0 | 49.30 | -968.0 | 1005.39 |

Conclusion: the re-run artifact records a clean 1 tx/block Slice 0 baseline with `stop_reason=max_blocks`, 10 loaded blocks, zero rejects, and average submit throughput of 1.120 tx/s. The broader multi-sender/full-ring ramp still needs a separate fix for sender eligibility/funding stability before it can be used as a higher-load baseline.

## Commands run

```text
cmd /c build_project.cmd test -p pwm-core -p pwmd
cmd /c build_project.cmd test -p pwm-core -p pwmd
python scripts/cy_cluster_transfer_ramp_soak.py --rpc http://127.0.0.1:61303 --wallet tmp/demo-genesis-wallet.yaml --pwm-bin target-codex/debug/pwm.exe --block-timing tmp/v7-s1-slice0-block-timing.jsonl --out-prefix tmp/v7-s1-slice0-ramp
python -c "...pre-initialize demo wallet accounts..."
python scripts/cy_cluster_transfer_ramp_soak.py --rpc http://127.0.0.1:61303 --wallet tmp/demo-genesis-wallet.yaml --pwm-bin target-codex/debug/pwm.exe --block-timing tmp/v7-s1-slice0-block-timing.jsonl --out-prefix tmp/v7-s1-slice0-ramp
perf --version
```

## Conclusion

Slice 0 baseline artifact is recorded, but the ticket result should be **PARTIAL** rather than PASS because the required `cargo test -p pwm-core -p pwmd` precondition fails reproducibly on the current branch.
