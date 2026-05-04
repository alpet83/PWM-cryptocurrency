# Sprint 14 - Slice 11 remediation2 testing

Date: 2026-04-28

## Verdict

PASS

Remediation2 fix for genesis-build invariant is validated: old failure reproduced on legacy artifact, regenerated genesis satisfies invariant, and `pwmd` startup with user params no longer fails with invariant error.

## Scope

Requested checks:
1. Reproduce previous failure scenario and confirm it is fixed.
2. Verify generated genesis always includes `validators.set[*].acct` in `funding.rows`.
3. Check `pwmd` startup with user params no longer fails with invariant error.
4. Run focused tests and provide exact command summary.

## Results

### 1) Previous failure scenario reproduced

Reproduced using the historical command and historical genesis artifact:

`cargo run -p pwmd -- --listen 127.0.0.1:3030 --state-root ./tmp/state-testnet-old --data-file ./tmp/state-testnet-old/pwm-data.json --genesis-file ./tmp/genesis-custom.json --genesis-passphrase 12345 --network-id testnet-qa --domain-hi 0x2C --cluster-id test-cluster-CY --node-id test-node-CY`

Observed failure (exit code 101):
- `genesis invariant: validators.set[0].acct must exist in funding.rows`

### 2) Fixed generation path verified

Generated new genesis through current `genesis-build`:

`cargo run -q -p pwm-cli -- genesis-build --wallet .tmp-test/slice9-wallet.yaml --wallet-passphrase slice9-pass --genesis-passphrase 12345 --out tmp/genesis-custom-remediation2.json`

Generator output:
- `genesis_path tmp/genesis-custom-remediation2.json`
- `genesis_rows 3`
- `genesis_schema 4`

Validated invariant in generated JSON:

`python -c "import json, pathlib; p=pathlib.Path('tmp/genesis-custom-remediation2.json'); d=json.loads(p.read_text(encoding='utf-8')); f={r['acct_hex'].lower() for r in d['gen_cfg']['funding']['rows']}; miss=[v['acct_hex'] for v in d['gen_cfg']['validators']['set'] if v['acct_hex'].lower() not in f]; print('validators',len(d['gen_cfg']['validators']['set'])); print('funding_rows',len(d['gen_cfg']['funding']['rows'])); print('missing',len(miss)); print('missing_list',miss)"`

Check output:
- `validators 1`
- `funding_rows 3`
- `missing 0`
- `missing_list []`

### 3) `pwmd` startup with user params

Started with user params and regenerated genesis:

`cargo run -p pwmd -- --listen 127.0.0.1:3030 --state-root ./tmp/state-testnet-rem2 --data-file ./tmp/state-testnet-rem2/pwm-data.json --genesis-file ./tmp/genesis-custom-remediation2.json --genesis-passphrase 12345 --network-id testnet-qa --domain-hi 0x2C --cluster-id test-cluster-CY --node-id test-node-CY`

Observed startup log:
- `pwmd startup phase: loading_snapshot (./tmp/state-testnet-rem2/pwm-data.json)`
- `pwmd startup phase: ready (no snapshot file)`
- `pwmd listening on http://127.0.0.1:3030 shard=CY state_ns=domain-hi-0x2c identity=(testnet-qa,0x2C,test-cluster-CY,test-node-CY) mode=shard_enforced(explicit-domain-config)`

No invariant panic observed on startup with these params.

Process cleanup:
- `Get-Process pwmd -ErrorAction SilentlyContinue | Stop-Process -Force; Get-Process pwmd -ErrorAction SilentlyContinue`
- Result: no running `pwmd` remained.

## Focused test runs (exact commands)

1. `cargo test -p pwm-cli genesis_build_adds_zero_balance_row_for_missing_validator_account` -> PASS (`1 passed; 0 failed`)
2. `cargo test -p pwm-cli genesis_build_generates_decoupled_v4_bundle` -> PASS (`1 passed; 0 failed`)
3. `cargo test -p pwm-core chain::tests::boot_rejects_missing_validator_funding_account` -> PASS (`1 passed; 0 failed`)
4. `cargo test -p pwmd genesis_json_v4_roundtrip_encrypted_validator_key` -> PASS (`1 passed; 0 failed`)

## Final assessment

- Historical failure mode is reproducible on old invalid genesis artifact.
- Remediation2 generation path now produces genesis where validator accounts are present in funding rows.
- `pwmd` startup with user-provided runtime params succeeds (no `validators.set[*].acct must exist in funding.rows` panic).
- Focused tests for generator/runtime contracts are green.
