# V7-S1 Slice 4 ramp results

- date: 2026-06-25
- code version: v0.1.69 workspace; live timing marker reported `pwmd/0.1.68`
- scope: tx hot-path lock cleanup, transfer ramp soak, ingress DoS probe, determinism check
- conclusion: sprint criterion not met in this run

## Code changes

- `crates/pwmd/src/api/handlers_tx.rs`: non-roaming `roaming_pool.lock_conflict_for` now uses a read lock and no longer calls `expire_by_height`; the seal loop already expires roaming intents once per block.
- `crates/pwmd/src/api/handlers_tx.rs`: non-roaming post-`try_send` flow trace no longer awaits a write lock. It uses `try_write()` to preserve the existing flow-trace contract when the lock is immediately available, and falls back to `info!` if not.
- `scripts/cy_cluster_transfer_ramp_soak.py`: `block_dt_overrun` threshold changed from a hard `1000ms` to `1000ms * --block-dt-overrun-mult`, default `1.15`, so small timer jitter does not stop a ramp.

No wire compatibility impact.

## Wallet precheck

`tmp/demo-genesis-wallet.yaml` contains 64 flags=0 accounts. Live RPC balance check found 63 accounts with balance at least 60 raw units; the ramp script selected 59 send-capable accounts.

## Ramp metrics

Command:

```text
python scripts/cy_cluster_transfer_ramp_soak.py --rpc http://127.0.0.1:3030 --wallet tmp/demo-genesis-wallet.yaml --out-prefix tmp/v7-s1-slice4-ramp-soak
```

Artifacts:

- `tmp/v7-s1-slice4-ramp-soak.md`
- `tmp/v7-s1-slice4-ramp-soak.client.jsonl`

Summary:

| metric | value |
|--------|-------|
| stop_reason | `head_stall` |
| max reached level | 34 tx/block |
| loaded_tx_ok | 597 |
| reject rate | 0% on all recorded batches |
| avg_tx_per_sec | 16.665 |
| baseline seal_slip p50 / p95 | 207.50ms / 373.55ms |
| highest recorded seal_slip in ramp | 2126ms |
| block_dt_overrun stop | no |

The ramp did not reach sustained 50 tx/s for 60 seconds. The run stopped at level 34 because head advancement stalled, not because of the old strict `block_dt > 1000ms` threshold. Block timing after the run showed cluster gate / attester delays up to about 8s around height 201996, while normal `prop_seal_commit` remained low on subsequent blocks.

## DoS probe

Artifacts:

- `tmp/v7-s1-slice4-dos-result.json`
- `tmp/v7-s1-slice4-dos-single-sender-result.json`

Results:

| scenario | requests | result |
|----------|----------|--------|
| 384 distributed CLI `tx-send` requests | 357 accepted, 27 policy rejects | server stayed alive; post-probe tx accepted |
| 640 single-sender nonce-race CLI `tx-send` requests | 640 accepted | server stayed alive; post-probe tx accepted |

The probe did not reproduce the expected 507 response. The server did not crash or deadlock and `/v1/status` stayed `ready`, but the explicit 507 saturation criterion remains unproven with the available CLI-based harness.

## Determinism

Command:

```text
cmd /c build_project.cmd test -p pwmd determinism
```

Result: PASS as an empty filter run; no test containing `determinism` is currently present in `pwmd` (`0 passed, 480 filtered out`).

## Gates

| command | result |
|---------|--------|
| `cmd /c build_project.cmd check --workspace` | PASS, with unrelated `pwm-cli` dead-code warning |
| `cmd /c build_project.cmd test -p pwm-core` | PASS |
| `cmd /c build_project.cmd test -p pwmd` | PARTIAL: 479 passed, `slice20_e2e_tests::slice20_dual_flow_ok` failed with `missing CY routing guard` |
| `python scripts/check_entity_name_segments.py crates/pwmd/src/api/handlers_tx.rs` | PASS |
| `python -m py_compile scripts/cy_cluster_transfer_ramp_soak.py` | PASS |
| `cmd /c build_project.cmd clippy -p pwmd --all-targets -- -W clippy::too_many_arguments -W clippy::too_many_lines -W clippy::cognitive_complexity -W clippy::module_inception` | PASS exit code, legacy warnings remain |
| `cmd /c build_project.cmd fmt --check` | FAIL on pre-existing unrelated formatting diffs in `crates/cqds-delegation-smoke`, `crates/pwm-cli`, and `crates/pwmd/src/lifecycle.rs` |

## Follow-up

- Investigate `head_stall` / cluster gate delay at and after level 34 before claiming the 50 tx/s soak criterion.
- Add a direct signed-tx HTTP flood harness or CLI dry-run JSON mode so ingress can be saturated without spawning hundreds of CLI processes.
- Resolve the existing `slice20_dual_flow_ok` routing guard failure separately; it is outside this hot-path lock slice but blocks a fully green `pwmd` gate.

## Re-run after head-wait fix

- date: 2026-06-25
- script change: the ramp now submits a batch at the current head and waits for `head > batch_height` before advancing to the next level.
- default wait change: `--stall-timeout-ms` increased from 5000ms to 8000ms.
- result: partial; nonce overlap was eliminated, but the sprint throughput criterion is still not met.

Artifacts:

- `tmp/v7-s1-rerun-soak.md`
- `tmp/v7-s1-rerun-soak.client.jsonl`
- `tmp/v7-s1-rerun-soak-2.md`
- `tmp/v7-s1-rerun-soak-2.client.jsonl`

| metric | first re-run | second re-run |
|--------|--------------|---------------|
| stop_reason | `block_dt_overrun` | `block_dt_overrun` |
| max_level | 9 | 18 |
| loaded_tx_ok | 47 | 173 |
| failed tx | 0 | 0 |
| nonce errors | 0 | 0 |
| avg_tx_per_sec | 2.901 | 5.413 |
| max seal_slip_ms | 4306 | 6677 |
| max pending_ticks_at_seal | 4 | 6 |
| sprint_criterion_met | no | no |

Interpretation:

The head-wait fix removed the bad-nonce overlap symptom: both re-run client logs had zero failed tx and zero nonce errors. The remaining blocker is no longer `head_stall`; it is a secondary seal path stall. Timing rows show multi-second `prop_seal_commit` at low and mid ramp levels (`210507`/`210508` in the first re-run, `210554`/`210555` in the second), followed by normal low-latency sealing after the script stops. This points to a seal/commit or runtime scheduling bottleneck under burst load rather than the original ramp overlap bug.
