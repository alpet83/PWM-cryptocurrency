## Sprint 14 Slice19 Remediation (coding)

- Root cause confirmed: `run_with` bootstrapped runtime `App` through identity path without passing `config.data_file`, so `App.data_file` stayed `None` and autosnapshot write path was skipped.
- Fix: extended identity bootstrap wiring to accept `data_file` and passed `Some(config.data_file.clone())` from `run_with`.
- Regression coverage: added async test that boots app with configured snapshot path, pushes one transfer, runs seal loop, and asserts snapshot file appears after first seal persistence point.
- Behavior unchanged otherwise: no changes to seal interval, snapshot format, or default path resolution logic.
