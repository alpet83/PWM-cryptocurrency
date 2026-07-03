# Sprint 14 — Slice 20 testing (end-to-end validation)

## Scope
- Repo: `P:/opt/docker/pwm-protocol`
- Goal: validate Slice20 after coding changes against required 6 checks.
- Stand: two `pwmd` nodes with explicit runtime labels:
  - CY: `127.0.0.1:4030`, `domain_hi=0x2C`, state `tmp/slice20-e2e/cy/pwm-data.json`
  - DO: `127.0.0.1:4040`, `domain_hi=0x32`, state `tmp/slice20-e2e/do/pwm-data.json`
- Genesis flow used: `pwm-cli wallet import-seed` + `pwm-cli genesis-build`.

## Commands (key steps)
- Wallet/genesis prep:
  - `cargo run -q -p pwm-cli -- key-gen` (CY/DO seeds)
  - `cargo run -q -p pwm-cli -- wallet import-seed --master <seed> --country CY --wallet-out tmp/slice20-e2e/wallet-cy.yaml --plaintext-dev`
  - `cargo run -q -p pwm-cli -- wallet import-seed --master <seed> --country DO --wallet-out tmp/slice20-e2e/wallet-do.yaml --plaintext-dev`
  - `cargo run -q -p pwm-cli -- genesis-build --wallet tmp/slice20-e2e/wallet-cy.yaml --out tmp/slice20-e2e/genesis.json --genesis-passphrase 12345 --premine-bal 1000000`
- Node boot (both from same genesis):
  - `target/debug/pwmd.exe --listen 127.0.0.1:4030 ... --domain-hi 0x2C ... --transport-peer-seed 127.0.0.1:4040`
  - `target/debug/pwmd.exe --listen 127.0.0.1:4040 ... --domain-hi 0x32 ... --transport-peer-seed 127.0.0.1:4030`
- TX checks:
  - local same-hi send on CY: `pwm --rpc http://127.0.0.1:4030 tx-send --wallet ... --to <CY account> --amount 10 --fee 1`
  - cross-shard send: `pwm --rpc http://127.0.0.1:4030 tx-send --wallet ... --to <DO account> --amount 100 --fee 1`
  - DO import attempt: `pwm --rpc http://127.0.0.1:4040 tx-import --wallet ... --to <DO account> --amount 100 --export-id <id>`
- Restart check:
  - stop both nodes, restart with same `--data-file` paths and inspect startup logs.

## Check-by-check verdict

1) Two-shard setup (CY + DO) boots with current genesis flow: **PASS**
- Both nodes started from generated `tmp/slice20-e2e/genesis.json`.
- Startup logs show explicit runtime labels:
  - CY: `pwmd listening ... shard=CY ...`
  - DO: `pwmd listening ... shard=DO ...`

2) Local transfer in CY (same-hi route) works and is not misrouted into roaming/invalid export: **PARTIAL / FAIL**
- Guard mode for local attempt was correct: `mode=same_domain_transfer` (not `invalid_export_same_domain`).
- But tx did not complete (`seal skip: tx: account not found` repeating), sender balance/nonce unchanged, receiver account absent.
- Conclusion: misroute regression is not reproduced, but functional "works" criterion failed.

3) Cross-shard CY->DO transfer via pwm-cli works through expected lifecycle: **FAIL**
- `tx-send` on CY created roaming intent and reached `exported` state (source debit observed: `1000000 -> 999899`, nonce `0 -> 1`).
- `tx-import` on DO failed with `invalid import: export_id is not known`.
- Lifecycle stops at source export; target import not completed in this run.

4) Snapshot persistence/restart without replay/state_root mismatch: **FAIL**
- Snapshot files were persisted on disk for both nodes:
  - `tmp/slice20-e2e/cy/pwm-data.json`
  - `tmp/slice20-e2e/do/pwm-data.json`
- After restart:
  - DO loaded snapshot successfully (`startup phase: ready (snapshot loaded)`).
  - CY failed replay check: `snapshot chain mismatch: block[24] state_root does not match replayed state`, then degraded fallback.

5) Routing guard log uses runtime labels (`CY`/`DO`) not legacy `A|B`: **FAIL**
- Observed logs still print legacy labels:
  - `tx routing guard: shard=A ...` on CY and DO flows.
- Required runtime label format was not met in this validation run.

6) Improved tx/balance-delta logs are present: **FAIL**
- Expected `tx commit delta: ... bal:x->y nonce:n->m` entries were not observed in captured runtime logs.
- Only guard/seal warnings and regular sealing logs were visible.

## Explicit overall verdict
**FAIL** — Slice20 end-to-end acceptance is not met on current tested head.

## Remaining blockers
- Local same-hi transfer remains non-functional in tested scenario (`seal skip: tx: account not found`).
- Cross-shard lifecycle is incomplete (`exported` on source, `export_id is not known` on DO import).
- CY snapshot replay mismatch persists after restart (`state_root` mismatch).
- Routing guard labeling still uses legacy `A|B`.
- Expected tx/balance-delta observability logs were not seen.

## Cleanup
- Test processes cleaned: **yes**.
- Stopped all spawned `pwmd` processes after validation.
