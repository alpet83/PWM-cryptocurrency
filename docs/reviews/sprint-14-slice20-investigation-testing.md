# Sprint 14 Slice 20 — Deep Investigation (testing)

## Scope and setup
- Repo: `P:/opt/docker/PWM-cryptocurrency`
- Controlled setup: two `pwmd` nodes from the same genesis (`tmp/genesis-custom.json`)
  - CY node: `127.0.0.1:3030`, `--domain-hi 0x2C`, state `tmp/slice20-investigation/cy/pwm-data.json`
  - DO node: `127.0.0.1:3131`, `--domain-hi 0x32`, state `tmp/slice20-investigation/do/pwm-data.json`
- Client path for reproduction: `pwm-cli` first; then mapping to `pwm-tui` behavior.

## Executive verdict (bugs vs config)
- **Confirmed bug #1 (code):** intra-CY send can be misrouted into roaming export path and fail as `invalid export`.
- **Confirmed bug #2 (code, critical):** CY->DO export can drain sender balance immediately (including full balance) before import finalization; no compensation path.
- **Confirmed bug #3 (code, critical):** snapshot becomes non-loadable after roaming export/import due state/chain mismatch (`state_root` mismatch on replay).
- **Confirmed bug #4 (code, observability):** routing guard log uses legacy `shard=A|B` label in tx guard trace even when runtime shard is `CY/DO`.
- **Not a pure config issue:** all above reproduced on clean controlled setup with explicit runtime identity; behavior is deterministic from code paths.

## Reproduction evidence

### A1) Intra-shard CY symptom mapped via CLI -> TUI message path
1. Same-domain export submission (CY hi-byte to CY hi-byte) via CLI:
   - Command: `pwm tx-export ... --target-domain 11515`
   - Result: `tx submit: HTTP 400 ... tx rejected: invalid export`
2. Node tx-guard log for this request:
   - `mode=invalid_export_same_domain`
3. TUI mapping:
   - `pwm-tui` formats any 400/invalid body from roaming-intent create as:
   - `Cross-domain send rejected: invalid request for roaming intent. details: ...`
4. Therefore user-visible TUI string is consistent with the same server rejection body (`tx rejected: invalid export`).

### A2) CY->DO transfer drains sender balance
1. Start state (`/v1/account/<CY-account>`): `balance_pwm=1000000, nonce=0`.
2. Run:
   - `pwm tx-send --rpc http://127.0.0.1:3030 --wallet tmp/cy-wallet.yaml ... --to <DO address> --amount 999999 --fee 1`
3. Result:
   - roaming intent created (`status=exported`) and stays in progress.
   - Sender becomes `balance_pwm=0, nonce=1`.
4. This confirms source-side debit occurs immediately on export, before target import completion.

### A3) Snapshot corruption / restart rejection
1. After roaming export, persisted snapshot (`tmp/slice20-investigation/cy/pwm-data.json`) contains:
   - sender balance already debited,
   - `exported_registry_size=1`,
   - latest blocks with empty tx lists.
2. Restart CY node with same snapshot:
   - startup warning/error: `snapshot chain mismatch: block[31] state_root does not match replayed state`
   - node falls back to `ready_degraded` with genesis state.
3. This reproduces user report: snapshot file exists but is rejected at load.

### A4) Legacy shard label in logs
- In the same CY runtime session logs:
  - startup logs show `shard=CY`
  - tx guard log shows `tx routing guard: shard=A ...`
- Confirms mixed labeling in runtime observability.

## Root causes in code paths

### D1) Routing decision mismatch (full domain vs hi-byte domain class)
- TUI route selection:
  - `crates/pwm-tui/src/main.rs` -> `is_cross_domain_route(from,to)` compares full `u16` domains.
- Server export validity:
  - `crates/pwm-core/src/tx.rs` -> `export_context_is_valid` treats same **hi-byte** as same domain for export prohibition.
- Guard semantics:
  - `crates/pwmd/src/tx_policy.rs` transfer guard uses hi-byte logic and classifies same-hi as local transfer.
- Effect:
  - addresses like `CY/FB -> CY/00` are same shard-class (hi `0x2C`) but TUI can decide "cross-domain", trigger roaming export, and get `invalid export`.

### D2) Non-atomic roaming commit pipeline (state mutate first, chain commit later)
- In both `/v1/tx` (EXPORT/IMPORT branch) and `/v1/roaming-intents` create:
  - state mutation done by `g.chain.st.apply_tx(&tx)` first,
  - then `g.chain.seal(vec![])` with **empty tx list**.
- Because block is sealed without tx payload while state already changed:
  - block replay cannot reconstruct state root from tx history.
  - persisted snapshot fails validation on next startup.

### D3) Rollback absence on post-apply failure
- After `apply_tx`, failure paths (`seal`, snapshot persist) return HTTP errors/degraded state without reverting in-memory state mutation.
- This creates externally visible partial commits (debit done, no matching replay-safe chain record).

### D4) Legacy guard label leakage
- `tx_policy` guard log prints `local_shard.as_str()` (`A|B`) instead of runtime label (`CY|DO`), while startup/status paths already use runtime shard label.

## Prioritized bug list and minimal fix suggestions

1. **P0: Roaming commit atomicity / chain-state consistency**
   - Fix: never call `apply_tx` out-of-band for roaming tx.
   - Route roaming tx through `chain.seal(vec![tx])` (or equivalent atomic staged state + commit).
   - Ensure block tx list and state root are replay-consistent.

2. **P0: Add rollback guard for all post-apply failures**
   - If staged apply fails at seal/persist, restore previous state (or avoid mutating live state before durable commit).
   - Return error without side effects visible to account balances/nonces.

3. **P1: Align cross-domain decision semantics**
   - TUI/CLI cross-domain routing should use hi-byte domain class (or call a shared helper used by server policy).
   - Prevent false roaming path for same-hi transfers (e.g., CY/FB -> CY/00).

4. **P1: Observability label unification**
   - Replace guard log field `shard=A|B` with runtime shard label helper (`CY|DO`), keep A/B only as optional debug field if needed.

5. **P2: Regression tests**
   - Add integration test: roaming export then restart from snapshot must load cleanly.
   - Add test: same-hi transfer must not choose export path.
   - Add test: failed roaming commit must not change sender balance/nonce.

## Confirmed vs config checklist
- Intra-CY `invalid export` under roaming path: **confirmed code bug** (routing semantics mismatch).
- CY->DO balance drain before import: **confirmed code behavior/bug risk** (no transactional protection).
- Snapshot non-loadable after roaming tx: **confirmed critical code bug** (state/chain divergence).
- `shard=A` guard log in CY runtime: **confirmed code bug** (labeling inconsistency).
- Misconfiguration as primary cause: **not confirmed** in this controlled repro.
