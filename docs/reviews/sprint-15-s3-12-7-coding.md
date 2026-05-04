# Sprint 15 — S3.12.7 coding review

## Scope delivered

- Added a narrow decode-compatibility shim for wire `u128` fields used by peer frames:
  - `AccountViews.rows[].balance_pwm`
  - `CrossShardFacts.facts[].amount`
- Compatibility is decode-first and backward-safe:
  - accepts decimal string (full `u128` range),
  - accepts numeric JSON where feasible (non-negative integer numbers that fit parser path),
  - does not change wire frame shape or trust/handshake/reconnect policy logic.

## Code changes

- `crates/pwmd/src/wire_serde.rs`
  - new helper `de_u128_compat` for serde decode of `u128` from string or non-negative integer.
- `crates/pwmd/src/state.rs`
  - `PeerAccountViewWire.balance_pwm` now uses `#[serde(deserialize_with = "...de_u128_compat")]`.
- `crates/pwmd/src/ledger.rs`
  - `CrossShardFact.amount` now uses `#[serde(deserialize_with = "...de_u128_compat")]`.
- `crates/pwmd/src/transport.rs`
  - extracted `decode_wire_msg_payload` from read path for shared decode surface.
  - added focused regressions:
    - non-empty `AccountViews` decode with `u128` payload,
    - non-empty `CrossShardFacts` decode with `u128` payload,
    - negative invalid `u128` decode case.

## API/version marker

- `pwmd` public API contract/version marker: **unchanged**.
- No API behavior requiring version marker bump in this slice.
