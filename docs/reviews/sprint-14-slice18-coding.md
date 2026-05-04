# Sprint 14 Slice 18 — coding report

## Scope delivered

- Added logging contract doc: `docs/LOGGING_STYLE.md`.
- Integrated minimal logger object in `pwmd` (`NodeLogger`) for startup info/error paths.
- Updated default logging path pattern to `logs/{date}/{log_name}_{time}.log`.
- Kept size-based file rotation and retention options, aligned docs/defaults.
- Added DEBUG tx inclusion logging for validator-sealed block txs with per-address balance changes.

## Code changes

- `crates/pwmd/src/logging.rs`
  - default fallback filter is now `debug` when `RUST_LOG` is unset;
  - added `NodeLogger` with `info/error/debug_tx`;
  - console writer routes `INFO` and below to `stdout`, `WARN/ERROR` to `stderr`;
  - kept file sink plain-text and rotating.
- `crates/pwmd/src/config.rs`
  - defaults: `log_dir=logs`, `file_template={date}/{log_name}_{time}.log`.
- `crates/pwmd/src/main.rs`
  - CLI defaults aligned with new pattern;
  - startup error/info paths routed through logger object after logging init.
- `crates/pwmd/src/lifecycle.rs`
  - startup phase and listen lines routed through logger object;
  - added DEBUG generation of `tx_included` events with balance diffs for touched addresses;
  - added test coverage for tx balance-diff event generation helper.
- `docs/pwmd.md`
  - updated defaults/options and DEBUG tx inclusion notes.

## Validation

- `cargo fmt`
- focused `pwmd` tests for logging/lifecycle (see run output in task handoff)

## Open risks

- Current formatter still relies on tracing default body format; strict visual contract details (full custom `[HH:MM:SS.mmm] #TAG: ...`) may need a dedicated custom `tracing` event formatter pass.
