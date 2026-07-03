# Sprint 14 — Slice 17 Remediation Testing

Date: 2026-04-29  
Repository: `P:/opt/docker/pwm-protocol`  
Scope: focused `pwmd` logging remediation retest

## Command

- `cargo test -p pwmd logging::tests:: -- --nocapture`
- Result: **PASS**
- Duration: ~0.75s
- Totals: `9 passed; 0 failed; 0 ignored; 0 measured; 109 filtered out`

## Verification verdict

1. Rotate IO failures are handled safely (no silent truncate risk): **PASS**  
   Evidence: `logging::tests::rotate_error_does_not_truncate_active_log` passed; injected rename failure aborts rotate and previously written active log content remains readable.

2. Mode behavior (`required` hard-fail, `on` degrade-with-warning): **PASS**  
   Evidence:
   - `logging::tests::required_mode_panics_after_rotate_error` passed (hard-fail path active in `required`).
   - `logging::tests::on_mode_degrades_after_rotate_error` passed; warning emitted: `file logger degraded after write/rotate error... continuing in console-only mode`.

3. Retention still works in happy path: **PASS**  
   Evidence: `logging::tests::rotation_triggers_and_keeps_retention_cap` passed; rotation keeps `.1`/`.2` and enforces cap (no `.3`).

4. Docs note about `RUST_LOG` filtering present: **PASS**  
   Evidence in `docs/pwmd.md`:
   - File sink note says it is filtered by common `RUST_LOG` / `EnvFilter`.
   - ENV section states `RUST_LOG` defines shared stream filter for console/file sinks.

## Conclusion

Slice 17 remediation checks are green for the requested logging behaviors; no regressions observed in the focused `pwmd` logging test pack.
