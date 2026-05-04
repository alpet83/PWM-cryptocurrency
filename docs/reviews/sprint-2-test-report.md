# Sprint 2 Test Report (Execution)

**Scope:** независимая testing-верификация после coding pass Sprint 2 для `pwmd` guard-контрактов и recipient prefilter deterministic checks.  
**Baseline refs:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/1-address-format.md`, `docs/rfc/6-policy-engine.md`, `docs/pwmd.md`, `docs/reviews/sprint-2-checklist.md`.  
**Verdict:** `pass`.

## 1) Executed commands and outcome

- `cargo test -p pwmd` -> **PASS** (`23 passed; 0 failed; 0 ignored`).
- Run time: fast local execution (single run, no retries required).

## 2) Contract checks closed in this execution gate

- Recipient prefilter deterministic checks are present and passing in `crates/pwmd/src/lib.rs`:
  - `v1_tx_rejects_reserve_recipient_prefilter` -> `400 BAD_REQUEST`, body includes stable substrings `recipient domain` + `reserve`.
  - `v1_tx_rejects_witness_recipient_prefilter` -> `400 BAD_REQUEST`, body includes stable substrings `recipient domain` + `witness-only`.
  - `v1_tx_rejects_unknown_recipient_prefilter` -> `400 BAD_REQUEST`, body includes stable substrings `recipient domain` + `not recognized`.
- Existing guard-contract checks remain intact and passing:
  - `v1_tx_rejects_wrong_shard_for_sender_domain_hi` -> `409 CONFLICT`, body includes `tx belongs to process shard`.
  - `v1_tx_rejects_cross_shard_transfer_on_local_path` -> `409 CONFLICT`, body includes `cross-domain transfer is disabled`.

## 3) Gate status vs Sprint 2 checklist

- Mandatory negative classes from Sprint 2 matrix are covered by explicit automated tests in `pwmd`.
- Kickoff `partial` state for recipient prefilter evidence is resolved for automated gate.
- Testing gate for this Sprint 2 pass is **PASS** for the listed `pwmd` contracts.

## 4) Residual risks / carry-over

- Guard checks are unit/integration style (`Router::oneshot`), not long-running multi-process A/B runtime proof.
- Error contract depends on stable message substrings; large future text refactors can cause false failures without behavior drift.
- Heavy soak/performance and cross-shard finality remain outside Sprint 2 testing scope by design.
