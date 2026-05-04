# Sprint 14 Slice 21 Coding

## Implementation

- `pwm-data.json` now writes snapshot `version: 2`.
- Fixed `[u8; 32]` and `[u8; 64]` snapshot fields are emitted as lowercase hex strings without `0x`.
- Snapshot `u128` financial values are emitted as decimal strings; `u64/u32/u16` remain JSON numbers.
- The loader accepts v2 plus the short migration window for v1 canonical snapshots and v0 legacy snapshots, converting them into the current runtime snapshot model before validation.
- v2 hex and decimal parsing is strict and reports field paths such as `blocks[0].hdr.prev_hash` or `state.fee_pool`.
- Consensus replay, state root validation, roaming restore, and genesis snapshot checks are unchanged.

## Tests Run

- `cargo fmt`
- `cargo check -p pwmd`
- `cargo test -p pwmd snapshot_`

## Notes

- Save/autosnapshot paths use the same `save_snapshot` function, so both now write v2 only.
- Existing snapshot validation remains the behavioral gate after wire-format decoding.
