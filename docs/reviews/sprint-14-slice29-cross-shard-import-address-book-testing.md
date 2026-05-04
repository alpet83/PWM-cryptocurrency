# Sprint 14 Slice29: cross-shard tx/import/address-book (testing)

## Scope

- Repo: `P:/opt/docker/PWM-cryptocurrency`
- Goal: reproduce and localize:
  - cross-shard send/import flow behavior,
  - address-book add/persist/readback,
  - TUI receiver visibility path.

## Controlled setup (CY + DO)

Used isolated local pair on custom ports to avoid collision with already running nodes:

- CY node: `127.0.0.1:3130`, `--domain-hi 0x2c`, `--state-root tmp/slice29/state-a`
- DO node: `127.0.0.1:3131`, `--domain-hi 0x32`, `--state-root tmp/slice29/state-b`

Start command (git-bash, prebuilt binary):

```bash
nohup ./target/debug/pwmd.exe --listen 127.0.0.1:3130 --state-root tmp/slice29/state-a --network-id devnet --domain-hi 0x2c --cluster-id local-cy --node-id node-cy > tmp/slice29/logs/pwmd-cy.log 2>&1 &
nohup ./target/debug/pwmd.exe --listen 127.0.0.1:3131 --state-root tmp/slice29/state-b --network-id devnet --domain-hi 0x32 --cluster-id local-do --node-id node-do > tmp/slice29/logs/pwmd-do.log 2>&1 &
```

Health snapshots:

- `GET /v1/status` (CY): `roaming_relay_mode=manual_handoff_required`, `state_namespace=domain-hi-0x2c`
- `GET /v1/status` (DO): `roaming_relay_mode=manual_handoff_required`, `state_namespace=domain-hi-0x32`

## Reproduction evidence

### 1) tx-init baseline

Commands:

```bash
./target/debug/pwm.exe --rpc http://127.0.0.1:3130 tx-init --wallet tmp/slice29/wallets/cy.yaml --index 0 --flags 0
./target/debug/pwm.exe --rpc http://127.0.0.1:3131 tx-init --wallet tmp/slice29/wallets/do.yaml --index 0 --flags 0
```

Result:

- both returned `204 No Content`.

### 2) Cross-shard send behavior

Command:

```bash
./target/debug/pwm.exe --rpc http://127.0.0.1:3130 tx-send --wallet tmp/slice29/wallets/cy.yaml --to <DO_PRETTY> --amount 10
```

Observed response:

- CLI printed note:
  - `target recipient preflight is unavailable in this source-RPC flow; target tx-import will reject missing/uninitialized recipients`
- then failed with:
  - `cross-domain send failed with HTTP 500 Internal Server Error. details: seal after roaming tx failed: tx: insufficient balance`

Implication:

- request is treated as roaming path (not immediate hard reject solely for cross-domain),
- but here final failure happened earlier on source due balance, so full export/handoff/import chain did not complete in this isolated run.

### 3) Address-book add/persist/readback

Command:

```bash
./target/debug/pwm.exe wallet book-add --wallet tmp/slice29/wallets/cy.yaml --address "<DO_PRETTY>" --label slice29-do
./target/debug/pwm.exe wallet show --wallet tmp/slice29/wallets/cy.yaml
```

Observed:

- `book-add` -> `ok`
- `wallet show` contains:
  - `address_book_count 1`
  - `address_book[0] <DO_PRETTY> label=slice29-do`
- raw YAML persisted canonical entry under `address_book` with label.

Conclusion:

- write path (`append_wallet_yaml_address_book`) and readback path are working in CLI.

## Code-path verification (root-cause localization)

### A) Save/read path

- `crates/pwm-core/src/address_book.rs`:
  - `append_wallet_yaml_address_book(...)` parses, validates, appends canonical entry, writes YAML.
- `crates/pwm-tui/src/main.rs`:
  - `book_prompt Enter` calls `append_wallet_yaml_address_book(...)`, then `choose_identity(...)` reloads wallet.

So "add succeeded but not saved" is **not reproduced** in this run.

### B) TUI visibility/filtering path

- `owner_and_receivers(...)` in `crates/pwm-tui/src/main.rs` builds receiver list from `w.address_book` **but filters out owned accounts**:

```rust
w.address_book
    .iter()
    .filter(|b| !w.owned_accounts.iter().any(|a| a.id == b.id))
```

This can produce "address exists in wallet file, but not visible in Receivers" when the added target is also present in `owned_accounts`.

This matches the reported symptom pattern.

### C) Import lifecycle not reaching DO

- Node status explicitly advertises manual mode:
  - `roaming_relay_mode=manual_handoff_required`
  - hint says operator must deliver provenance and submit IMPORT on target.
- RFC/runbook confirm MVP contract: no auto-relay; required chain is `EXPORT -> finalize -> handoff register -> IMPORT`.

Therefore "no apparent import handling in DO logs" is expected if operator stopped at source-side send/export and did not perform handoff/register/import.

## Verdict

## 1) Confirmed bug

- **TUI receiver invisibility for some added entries**: confirmed as behavior-level bug/UX trap.
  - Persisted entry can be hidden by `owned_accounts` filter, with no explicit explanation in UI.

## 2) Operator-flow mismatch (not runtime bug)

- **No auto-import on DO after cross-shard send**: expected in current MVP manual-handoff mode.
  - Requires explicit finalize/handoff-register/import steps.

## 3) Not confirmed in this isolated run

- "sender debited while cross-shard should be blocked" was not fully reproduced due insufficient source balance before export completion.
  - Needs funded source account in the same isolated setup to prove debit-before-import semantics end-to-end.

## Minimal fix plan

1. **TUI clarity fix (low-risk)**
   - In Receivers panel, show address-book entries even if owned (with tag like `[owned]`) OR add status line: `N address_book entries hidden as owned`.
2. **Operator guidance fix**
   - On cross-domain submit success/fail path, always print compact next-step checklist:
     - `finalize -> tx-handoff-register -> tx-import`.
3. **Optional guardrail**
   - If `manual_handoff_required`, surface explicit badge in TUI main screen before send (not only in docs).
4. **Repro harness**
   - Add deterministic funded two-shard e2e scenario for slice29 to assert:
     - source debit/export state,
     - target import absent until manual handoff,
     - post-handoff import success.

## Cleanup

Started processes were explicitly terminated after investigation.
