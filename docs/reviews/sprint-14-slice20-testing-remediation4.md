# Sprint 14 - Slice 20 remediation4 testing

Repo: `P:/opt/docker/PWM-cryptocurrency`

## Verdict

PASS.

The remediation4 handoff flow passed the targeted coding-report command set. Source finalize now has a portable signed handoff path covered by the Slice20 e2e contract, target-side provenance registration is covered through the supported `POST /v1/export-provenance` / `pwm tx-handoff-register --handoff-json` path, and `tx-import` only succeeds after that registration.

## Required checks

- PASS - Source finalize emits a portable signed handoff. Covered by `slice20_two_shard_e2e_flows_contract`, which now exercises `finalize -> tx-handoff-register -> tx-import`.
- PASS - Target provenance registration works through the supported delivery path. Covered by `tx_handoff_register_posts_to_export_provenance` and the Slice20 e2e contract.
- PASS - `tx-import` succeeds after handoff registration. Covered by `slice20_two_shard_e2e_flows_contract` and `tx_import_retries_until_export_id_known`.
- PASS - Unknown/forged `export_id` import still rejects and does not credit. Covered by `v1_tx_rejects_import_unknown_export_id`, `tx_import_auto_init_does_not_mask_unknown_export_id`, and core import replay/provenance tests.
- PASS - `slice20_two_shard_e2e_flows_contract` uses the supported handoff path and passes.
- PASS - Snapshot restart integrity, CY/DO labels, and tx commit delta regressions still pass via `snapshot_restore_keeps_import_replay_guard`, `v1_tx_two_node_smoke_cy_to_do_with_negative_suite`, and `v1_tx_http_export_import_advances_head_height_via_sync_seal`.

## Commands run

All commands were run via CQDS `cq_process_ctl` host mode from `P:\opt\docker\PWM-cryptocurrency`.

```text
cargo fmt
PASS exit=0 duration=0.53s

cargo check
PASS exit=0 duration=0.36s

cargo test -p pwm-core import_ -- --nocapture
PASS exit=0 duration=0.33s
7 passed; 0 failed; 0 ignored

cargo test -p pwmd v1_tx_ -- --nocapture
PASS exit=0 duration=0.63s
24 passed; 0 failed; 0 ignored

cargo test -p pwm-cli tx_import_ -- --nocapture
PASS exit=0 duration=0.35s
6 passed; 0 failed; 0 ignored

cargo test -p pwm-cli tx_handoff_ -- --nocapture
PASS exit=0 duration=0.38s
2 passed; 0 failed; 0 ignored

cargo build -p pwmd --bin pwmd -p pwm-cli --bin pwm
PASS exit=0 duration=0.33s

cargo test -p pwmd slice20_two_shard_e2e_flows_contract -- --nocapture
PASS exit=0 duration=11.30s
1 passed; 0 failed; 0 ignored
```

## Notes

- No checklist rows were changed.
- Cleanup: yes; verified no `pwmd` or `pwm-tui` processes remained after the run.
- Tooling note: two narrow host `rg` checks for source mapping hung without output and were killed; verdict is based on the targeted cargo output above.
