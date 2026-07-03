# Sprint 14: testnet startup investigation

## Scope

- Repo: `P:/opt/docker/pwm-protocol`
- Target command:
  `cargo run -p pwmd -- --listen 127.0.0.1:3030 --state-root ./tmp/state-testnet --data-file ./tmp/state-testnet/pwm-data.json --genesis-file ./tmp/genesis-custom.json --genesis-passphrase 12345 --network-id testnet-qa --domain-hi 0x2C --cluster-id test-cluster-CY --node-id test-node-CY`

## Reproduction

- Reproduced locally with the exact arguments.
- Observed panic:
  `genesis invariant: validators.set[0].acct must exist in funding.rows`

## Data inspection (`tmp/genesis-custom.json`)

- `gen_cfg.funding.rows[*].account` field is absent (schema v4 uses `acct_hex`).
- `gen_cfg.validators.set[*].acct` field is absent (schema v4 uses `acct_hex`).
- Effective values in file:
  - funding `acct_hex`:
    - `32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5`
    - `2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e`
  - validator set `acct_hex`:
    - `8b156ec0000ab8efd52949577c1a965d495b9cc7b767c85f771a2c2b5a674dab`
- Validator `acct_hex` is missing from funding rows, so `Chain::boot` invariant fails as designed.

## Root cause category

**Verdict: generation-side bug (`pwm-cli genesis-build`)**.

Why:
- Import/runtime parser in `pwmd` correctly parses schema v4 (`acct_hex`/`pubkey_hex`) and builds `GenCfg`.
- Runtime invariant in `pwm-core` is consistent with reward/account assumptions.
- Generator currently creates validator account id from validator key path (`m/.../1`) but funding rows from wallet account ids, and does not ensure validator account is present in funding.

## Minimal workaround (user can run now)

PowerShell from repo root:

```powershell
python -c "import json, pathlib; p=pathlib.Path('tmp/genesis-custom.json'); d=json.loads(p.read_text(encoding='utf-8')); f=d['gen_cfg']['funding']['rows']; v=d['gen_cfg']['validators']['set'][0]; exists=any(r.get('acct_hex')==v.get('acct_hex') for r in f); (f.append({'acct_hex':v['acct_hex'],'pubkey_hex':v['pubkey_hex'],'der_idx':v['der_idx'],'bal':'0'}) if not exists else None); p.write_text(json.dumps(d, ensure_ascii=False, indent=2)+'\n', encoding='utf-8'); print('ok')"
```

Then start daemon with same command.

This adds a minimal funding row for validator account and satisfies invariant without changing encrypted validator keys.

## Minimal patch plan (code bug fix)

1. In `crates/pwm-cli/src/main.rs` (`build_genesis_v4_wallet`):
   - Ensure validator account appears in funding rows when bundle is generated.
   - Minimal safe option: append validator row to funding when missing (bal can be configured; default `0` is enough for invariant).
2. Add regression test in `crates/pwm-cli/src/main.rs` tests:
   - Assert every `gen_cfg.validators.set[*].acct_hex` exists in `gen_cfg.funding.rows[*].acct_hex`.
3. Optional hardening in `crates/pwmd/src/snapshot.rs`:
   - Early validation with clear `Err(...)` before `Chain::boot` panic path.

## Impacted files (for code fix)

- `crates/pwm-cli/src/main.rs` (generator logic + tests)
- `crates/pwmd/src/snapshot.rs` (optional UX validation)
