# Sprint 14 — Slice 13 E2E testing (CY -> DO anomaly)

Repo: `P:/opt/docker/PWM-cryptocurrency`  
Date: 2026-04-28  
Scope: full reproduction from scratch for user-reported anomaly: cross-shard transfer `CY -> DO` drains sender, recipient unchanged, no visible history after restart, and suspected missing `pwm-data`/logs persistence.

## Verdict

`confirmed bugs (multiple), not just operator/config issue`

Observed behavior is reproducible with two independent `pwmd` nodes and `pwm-cli`:
- sender on `CY` is debited (`999899` after `amount=100, fee=1`);
- recipient on `DO` remains unchanged (`0`);
- roaming intent stays in `exported` and never reaches `imported`;
- after restart, runtime state returns to genesis values and roaming intent disappears;
- no `/v1/history` API (404 before/after restart).

## Reproduction log (from clean sandbox)

Working dir for artifacts: `tmp/slice13-e2e`.

1) Generate seeds and wallets
- `cargo run -p pwm-cli -- key-gen` (CY + DO seeds)
- `cargo run -p pwm-cli -- wallet import-seed --master <seed> --country CY --wallet-out tmp/slice13-e2e/wallet-cy.yaml --plaintext-dev`
- `cargo run -p pwm-cli -- wallet import-seed --master <seed> --country DO --wallet-out tmp/slice13-e2e/wallet-do.yaml --plaintext-dev`
- Resulting accounts:
  - CY: `2cefb8302c0075919555900d191972b3a975cd0068127936ebd49c85cb96edc3`
  - DO: `3285bf578800b96a8a29bd3776bf3f280d6636cb5ee181487716cab7872a82ed`

2) Build custom genesis with premine on CY wallet
- `cargo run -p pwm-cli -- genesis-build --wallet tmp/slice13-e2e/wallet-cy.yaml --out tmp/slice13-e2e/genesis-cy.json --genesis-passphrase 12345`
- Output: `genesis_schema 4`, `genesis_rows 2`

3) Start separate nodes
- CY node:
  - `target/debug/pwmd.exe --listen 127.0.0.1:4030 --state-root tmp/slice13-e2e/cy --data-file tmp/slice13-e2e/cy/pwm-data.json --genesis-file tmp/slice13-e2e/genesis-cy.json --genesis-passphrase 12345 --network-id testnet-s14 --domain-hi 0x2C --cluster-id cluster-CY --node-id node-CY --transport-real --transport-peer-seed 127.0.0.1:4040`
- DO node:
  - `target/debug/pwmd.exe --listen 127.0.0.1:4040 --state-root tmp/slice13-e2e/do --data-file tmp/slice13-e2e/do/pwm-data.json --genesis-file tmp/slice13-e2e/genesis-cy.json --genesis-passphrase 12345 --network-id testnet-s14 --domain-hi 0x32 --cluster-id cluster-DO --node-id node-DO --transport-real --transport-peer-seed 127.0.0.1:4030`

4) Init + transfer via CLI
- `target/debug/pwm.exe --rpc http://127.0.0.1:4030 tx-init --wallet tmp/slice13-e2e/wallet-cy.yaml --index 0 --flags 0` -> `204 No Content`
- `target/debug/pwm.exe --rpc http://127.0.0.1:4040 tx-init --wallet tmp/slice13-e2e/wallet-do.yaml --index 0 --flags 0` -> `204 No Content`
- `target/debug/pwm.exe --rpc http://127.0.0.1:4030 tx-send --wallet tmp/slice13-e2e/wallet-cy.yaml --to <DO acct> --amount 100 --fee 1`
  - creates roaming intent `6b2ef3...4c89f`
  - status repeatedly prints `exported` and does not progress.

## State checks (before restart)

- CY account (`/v1/account/<cy>`): `balance_pwm=999899`, `nonce=1` (debit confirmed).
- DO account (`/v1/account/<do>` on DO node): `balance_pwm=0`, `nonce=1` (no import credit).
- Intent status (`/v1/roaming-intents/<id>`): `status=exported`.
- `/v1/history` on both nodes: `404 Not Found`.

## State checks (after restart)

- Restarting nodes with same `--state-root`/`--data-file` shows:
  - startup log: `ready (no snapshot file)` on both nodes.
  - CY account returns to genesis value: `balance_pwm=1000000`, `nonce=0`.
  - DO account not found unless re-init.
  - previous intent id returns `roaming intent not found`.

Filesystem observation:
- `tmp/slice13-e2e/cy` and `tmp/slice13-e2e/do` remained empty; no `pwm-data.json` produced during this run.

## Root-cause analysis

### 1) Cross-shard transfer path is incomplete at runtime (confirmed bug)

Evidence:
- Runtime intent stalls in `exported` and never reaches `imported`.
- Source-level contract in tests explicitly states manual handoff model:
  - `crates/pwmd/src/lib.rs` contains comment:
    - “Two-node operator handoff ... target imports only after provenance is delivered to target state”.

Conclusion:
- Current implementation does not provide automatic end-to-end relay/import completion for two live nodes in this scenario.
- This is product/runtime behavior gap, not operator typo in command syntax.

### 2) Persistence behavior is broken in this scenario (confirmed bug)

Evidence:
- Runtime mutates state (CY debit + DO init), but restart restores genesis baseline.
- No snapshot file appears at configured `--data-file`.
- Startup always logs `ready (no snapshot file)`.

Conclusion:
- Effective persistence for this flow is not working (either snapshot write path not reached in active binary, or write fails and state is not durable).

### 3) History visibility issue (confirmed bug / missing feature for reported expectation)

Evidence:
- `/v1/history` returns 404 on both nodes before and after restart.

Conclusion:
- “No history after restart” is partially expected because history endpoint is absent; this is a UX/API coverage gap relative to operator expectation.

### 4) Additional repo health issue affecting reproducibility (confirmed bug)

During investigation, fresh `cargo run` for `pwmd`/`pwm-cli` intermittently failed with compile errors:
- mismatch between `funding.accounts` vs `funding.rows`,
- mismatch between `GenCfg.accounts` vs `GenCfg.rows`.

Conclusion:
- Workspace currently contains inconsistent schema migration edits; this increases operator confusion and can mask runtime behavior.

## Bug list and minimal fix recommendations

1. **BUG-1: Roaming intent stuck at exported in two-node flow**
- Minimal fix:
  - implement/enable automatic provenance relay + target import execution path, or
  - make CLI/operator contract explicit and provide first-class command to complete handoff deterministically.

2. **BUG-2: State changes not durable across restart for this flow**
- Minimal fix:
  - add hard check + explicit error surface for snapshot write success after tx/intent path;
  - add integration test “tx-init + cross-domain export then restart -> state restored from `--data-file`”.

3. **BUG-3: Missing history API for operator visibility**
- Minimal fix:
  - expose minimal `/v1/history` (or equivalent tx/event feed) so TUI/CLI can show transfer lifecycle.

4. **BUG-4: Schema field drift causes compile instability (`accounts` vs `rows`)**
- Minimal fix:
  - unify schema naming across `pwm-core`, `pwm-cli`, `pwmd` snapshot/genesis code;
  - add CI check building all binaries in clean environment.

## Confirmed bug vs operator/config

- **Confirmed bug(s):** BUG-1, BUG-2, BUG-3, BUG-4 above.
- **Not primary operator/config issue:** commands/flags were valid enough to consistently reproduce the anomaly.
- **Operator expectation mismatch noted:** if current design still requires manual export/import handoff, docs and CLI should state this explicitly; current behavior appears as failure for normal user flow.

## Cleanup

- cleaned: `yes`
- stopped all spawned `pwmd` processes at end of run.
