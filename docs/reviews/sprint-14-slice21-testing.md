# Sprint 14 Slice 21 Testing

Date: 2026-04-29

## Verdict

PASS.

## Changes

- Extended `snapshot_roundtrip_blocks_and_state` to assert representative saved v2 binary fields are lowercase hex strings, not byte arrays:
  - `genesis_accounts[].acct`, `genesis_accounts[].pubkey`
  - `blocks[].hdr.prev_hash`, `tx_root`, `state_root`, `sig`
  - `state.accounts[].id`, `state.accounts[].account.signing_pubkey`
- Extended `snapshot_v2_rejects_malformed_hex_and_decimal` with additional malformed inputs:
  - uppercase hex, short hex, `0x`-prefixed hex
  - signed decimal, leading-zero decimal, `u128` overflow decimal

## Required Checks

- `cargo fmt`: PASS.
- `cargo check -p pwmd`: PASS.
- Snapshot tests: PASS, `25 passed; 0 failed; 0 ignored`.
- New saved `pwm-data.json` contract: PASS. The save path writes `version: 2`; tests assert representative binary fields are lowercase hex strings and representative `u128` values are decimal strings.
- v1/legacy read path: PASS. Existing focused tests cover v0 legacy migration and v1 load followed by v2 save.
- Malformed hex/decimal rejection: PASS. Focused tests assert field-path errors for malformed hex and decimal values.

## Commands

```text
cargo fmt
exit 0
duration: 7.8s
```

```text
cargo check -p pwmd
exit 0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.86s
duration: 8.0s
```

```text
cargo test -p pwmd snapshot_
exit 0
25 passed; 0 failed; 0 ignored; 115 filtered out
duration: 10.5s
```

## Notes

- Checklist rows updated: none.
- Cleanup: yes; no long-lived test server was started.
- Open risks: none found in this slice.
