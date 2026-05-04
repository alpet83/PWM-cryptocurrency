# Sprint 14 Slice 14 - testing report

Date: 2026-04-28
Repository: `P:/opt/docker/PWM-cryptocurrency`
Scope: validate slice14 (`rows -> accounts` rename + F7 hint removal)

## Verdict

**FAIL (not fully coherent yet)**

Reason: targeted runtime/tests are not fully green (`pwmd` has 3 failing tests), so check #1 fails.

## Check 1: runtime/tests coherence (`pwm-core`, `pwmd`, `pwm-cli`, `pwm-tui`)

Executed via CQDS host process runner:

- `cargo test -p pwm-core` -> **PASS**
  - Result: `71 passed; 0 failed`
  - Runtime: about 32s process time (`finished in 24.93s` for tests)
- `cargo test -p pwmd` -> **FAIL**
  - Result: `91 passed; 3 failed`
  - Failing tests:
    - `tx_policy::tests::burn_mark_guard_rejects_policy_invalid_beneficiary`
    - `tx_policy::tests::export_guard_rejects_policy_invalid_recipient`
    - `tx_policy::tests::burn_mark_guard_allows_same_shard_beneficiary`
- `cargo test -p pwm-cli` -> **PASS**
  - Result: `128 passed; 0 failed`
  - Runtime: long-tail case observed (~61.52s), completed successfully
- `cargo test -p pwm-tui` -> **PASS**
  - Result: `71 passed; 0 failed`
  - Runtime: `finished in 3.84s`

Conclusion for check #1: **FAIL** (because `pwmd` is red).

## Check 2: no mixed `rows/accounts` schema leftovers in active paths

Searches performed across `crates/**`:

- `\brows\b`
- `\baccounts\b`
- `"rows"`

Findings:

- No active JSON schema key `"rows"` usage found in runtime code paths.
- One `"rows"` key is present in a negative test fixture:
  - `crates/pwmd/src/lib.rs` (`schema_version: 2` unsupported-schema test fixture).
- Active schema paths use `accounts` (`funding.accounts`, `state.accounts`, `/v1/accounts`) consistently.

Conclusion for check #2: **PASS** for active paths, with one intentional legacy token in test fixture only.

## Check 3: TUI F7 hint/hotkey fully removed

Search for `F7` in code (`crates/**`) returns only two test references in `crates/pwm-tui/src/main.rs`:

- negative assertion that footer does **not** contain `"F7 inter-shard->CLI"`
- test function name `inter_shard_status_short_is_single_line_and_points_to_f7`

No runtime hint/hotkey text advertising F7 was found in active rendered strings; tests enforce absence in footer.

Conclusion for check #3: **PASS** functionally (runtime hint/hotkey removed), with minor leftover wording in test identifier/name.

## Cleanup

No background `pwmd`/`pwm-tui` daemons were started by this validation session.
Cleaned: **yes** (nothing to kill).
