# Sprint V2-2 — Slice 0: account API freeze

Date: 2026-05-06  
Agent: `pwm-coding`  
Scope: freeze only, no behavior change

## Frozen response shape (`GET /v1/account/:id`)

The public contract is `AcctOut` from `crates/pwmd/src/api/types.rs` and is built in `acct_out_for_runtime` (`crates/pwmd/src/api/common.rs`). Field names and serialized types are frozen for this slice:

- `id`: string (hex-encoded account id).
- `balance_pwm`: string (decimal amount, legacy compatibility field; user-facing PWM balance compatibility).
- `local_state_balance`: string (decimal amount from local shard state view).
- `authoritative_home_balance`: optional string (decimal amount from authoritative home shard view when available).
- `authoritative_home_initialized`: optional boolean (authoritative initialization flag when available).
- `home_lookup_status`: optional string (`local`, `ok`, `not_found`, `stale`, `unavailable` depending on home-shard lookup status).
- `spendable_on_this_shard`: optional string (decimal amount spendable on current shard; omitted for foreign accounts).
- `local_view_only`: boolean (`true` when account is foreign for this shard; local values are informational).
- `staked`: string (decimal amount).
- `marks`: string (decimal amount, serialized from numeric state as string).
- `initialized`: boolean.
- `nonce`: number (`u64` in Rust JSON serialization).

For `GET /v1/accounts`, each item in `accounts[]` uses the same `AcctOut` shape and semantics.

## `marks` semantics (single user-visible marks balance)

`marks` is the single user-visible marks balance in REST for v2. Slice 0 freezes that no separate burnable/quota marks field is exposed in public account responses.

## Note on `marks_quota` (legacy snapshot-only)

After Slice 1 (`6c52b71`), `pwm_core::State` has no runtime `marks_quota` mirror: the only consensus marks counter is `Account.marks`. Legacy `marks_quota` is preserved only in old snapshot JSON on load path (strict validation in `pwmd`) and is not part of the public REST account contract.

## Cross-shard and local view behavior (no change in Slice 0)

Cross-shard/account locality behavior remains exactly as already implemented via `local_view_only`, `home_lookup_status`, `authoritative_home_balance`, `authoritative_home_initialized`, and `spendable_on_this_shard`. Slice 0 introduces no runtime or policy changes for these fields; it only freezes their contract.

## Non-goals (Slice 0)

- No `pwm-core` state logic changes.
- No account handler behavior changes.
- No REST field additions/removals/renames.

## Review gate (pwm-review)

**Date:** 2026-05-06

### Verdict: **PASS** (approve)

### Scope recap

Slice 0 freezes the public `GET /v1/account/:id` and list-item shape as `AcctOut` (`crates/pwmd/src/api/types.rs`), with `acct_out_for_runtime` in `common.rs` as the builder, per `docs/plans/mvp_v2.md` Sprint V2-2 Slice 0 (API form only; no second user-visible marks field).

### Requirements fit

- **Plan alignment:** Matches `mvp_v2.md` §V2-2 — one user-facing marks number in API (`marks`); no second burnable/quota field in account JSON.
- **`AcctOut` spot-check:** Field set, optionality, and names match this freeze document (`id`, `balance_pwm`, `local_state_balance`, optional home/shard fields, `local_view_only`, `staked`, `marks`, `initialized`, `nonce`). `marks` is a `String` on the wire; `nonce` is `u64` (JSON number). No `marks_quota` (or similar) on `AcctOut`.
- **Builder spot-check:** `acct_out_for_runtime` fills `marks` from `ac.marks` only; no parallel marks/burnable field emitted.

### Style / safety / tests (Slice 0)

- **Style:** Documentation-only slice for behavior; `AcctOut` carries a pointer comment to this freeze file.
- **Safety:** No new trust-boundary or serialization change beyond naming the frozen contract; runtime behavior explicitly out of scope for Slice 0.
- **Tests:** Not in scope for this documentation freeze; follow-up slices own API/core tests per plan.

### Participation (orchestrator / ticket)

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-v2-2-slice0-account-api-freeze.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 3500
  confidence: medium
done_at: 2026-05-06T12:00:00+03:00
```

