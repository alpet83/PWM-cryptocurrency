# Sprint 15 S3.12.3 Diagnostics Review

## Verdict
`request changes`

## Findings

### High
`healthy_session_skip` сейчас логируется как `info` на каждом healthy skip-loop. При sleep floor это может давать постоянный шум в нормальной работе и визуально выглядеть как новый reconnect churn.

Required:
- оставить counter/last reason;
- live log делать только на transition/rollup, либо понизить до trace/debug/rate-limited.

### Medium
- Новый production identifier `account_stream_freshness_window_ms` слишком длинный для локального style contract. Нужно переименовать или явно обосновать compatibility reason.
- Testing gate дал `PARTIAL`: `pwm-cli::tx_import_auto_init_does_not_mask_unknown_export_id` упал.

## Covered
- TCP connect / handshake / session open / session close / reconnect decision диагностируются стабильными reason codes.
- Trust boundary сохранён: trusted decisions не расширены диагностикой.
- Unknown/unavailable semantics в intent сохранены.

## Required Remediation
1. Stop frequent `healthy_session_skip` info logs.
2. Rename `account_stream_freshness_window_ms`.
3. Fix or explain failing CLI regression test.
4. Re-run focused tests and update testing artifact.

## Final Remediation Result
`approve with nits`

Closed:
- `healthy_session_skip` no longer spams live `info` logs.
- `account_stream_freshness_window_ms` renamed to `account_fresh_window_ms`.
- CLI regression `tx_import_auto_init_does_not_mask_unknown_export_id` passes.
- Follow-up style blocker `DRAIN_READ_TIMEOUT_CAP_MS` renamed to `DRAIN_TIMEOUT_CAP_MS`.

Remaining nit:
- Live smoke still shows real churn causes (`protocol_error` / `heartbeat_read_failed`), but they are now observable with reason-coded diagnostics and can be handled by the next remediation slice.
