# Sprint 15 S3.6 transport handshake diagnostics testing

Date: 2026-04-30

## Verdict

**PASS**

## Scope

Validated the S15-S3.6 fix for HTTP seed-based real transport handshakes and diagnostics. The run used fresh local ports/state roots and did not depend on `3030/3031`.

## Live node checks

### Reciprocal HTTP seed handshake

Started two local `pwmd` nodes:

- Node A: `127.0.0.1:18956`, state root `.tmp-test/s15-s3-6-node-a`, seed `127.0.0.1:18957`.
- Node B: `127.0.0.1:18957`, state root `.tmp-test/s15-s3-6-node-b`, seed `127.0.0.1:18956`.

Both used `--transport-real`, matching `network_id=s15-s3-6`, matching devnet genesis, and distinct explicit node identities.

Observed via `GET /v1/status`:

- Node A: `ready=true`, `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health=ok`, `last_peer_error=null`, `effective_genesis_hash=678c973671ef3fc404b65895af1e6a55683ef0112c2016e846fed33b37803f46`.
- Node B: `ready=true`, `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health=ok`, `last_peer_error=null`, `effective_genesis_hash=678c973671ef3fc404b65895af1e6a55683ef0112c2016e846fed33b37803f46`.

Result: **PASS**.

### Genesis mismatch diagnostic

Started Node C on `127.0.0.1:18958`, state root `.tmp-test/s15-s3-6-node-c-mismatch`, with `tmp/genesis-custom.json` and seed `127.0.0.1:18956`.

Observed via `GET /v1/status`:

- `live_peer_count=0`
- `peer_relay_health=no_trusted_seed`
- `genesis_guard=blocked`
- `genesis_mismatch_total=6`
- `genesis_mismatch_expected_hash=9ab080cbfc8a9216fc274e3f4c29ee7e4a9da56c076835d7ad1325f22935453d`
- `genesis_mismatch_received_hash=678c973671ef3fc404b65895af1e6a55683ef0112c2016e846fed33b37803f46`
- `last_peer_error=seed 127.0.0.1:18956 genesis_mismatch expected_genesis_hash=9ab080cbfc8a9216fc274e3f4c29ee7e4a9da56c076835d7ad1325f22935453d received_genesis_hash=678c973671ef3fc404b65895af1e6a55683ef0112c2016e846fed33b37803f46`

Observed WARN log:

```text
#WARN: peer seed status rejected seed=127.0.0.1:18956 reason=genesis_mismatch expected_genesis_hash=9ab080cbfc8a9216fc274e3f4c29ee7e4a9da56c076835d7ad1325f22935453d received_genesis_hash=678c973671ef3fc404b65895af1e6a55683ef0112c2016e846fed33b37803f46 ...
```

Result: **PASS**.

### Connect refused/timeout diagnostic

Started Node D on `127.0.0.1:20259`, state root `.tmp-test/s15-s3-6-node-d-refused`, with seed `127.0.0.1:20260` where no peer was listening.

Observed via `GET /v1/status`:

- `live_peer_count=0`
- `peer_relay_health=no_trusted_seed`
- `last_peer_error=seed 127.0.0.1:20260 connect_timeout: error sending request for url (http://127.0.0.1:20260/v1/status)`
- `peer_error_at_ms` populated
- `next_seed_due_ms` populated

Result: **PASS**. The failure is surfaced as a specific `last_peer_error`, not only as `live_peer_count=0`.

## Automated regression checks

All commands ran in `P:\opt\docker\PWM-cryptocurrency`.

- `cargo test -p pwmd tests::real_transport_tick_connects_seed_and_accepts_handshake -- --exact` -> PASS, `1 passed`.
- `cargo test -p pwmd tests::real_transport_tick_rejects_genesis_mismatch_and_tracks_reason -- --exact` -> PASS, `1 passed`.
- `cargo test -p pwmd tests::v1_status_exposes_genesis_guard_diagnostics -- --exact` -> PASS, `1 passed`.
- `cargo test -p pwmd tests::real_transport_tick_respects_retry_backoff_on_connect_timeout -- --exact` -> PASS, `1 passed`.
- `cargo test -p pwmd inbound_hello_does_not_mark_relay_ok -- --nocapture` -> PASS, `1 passed`.
- `cargo test -p pwmd network_mismatch_sets_status_diagnostic -- --nocapture` -> PASS, `1 passed`.
- `cargo test -p pwmd real_transport_tick_reports_status_decode_failure -- --nocapture` -> PASS, `1 passed`.
- `cargo test -p pwmd v1_export_provenance -- --nocapture` -> PASS, `4 passed`.
- `cargo test -p pwmd tests::v1_roaming_intent_no_seed_stays_exported_with_relay_error -- --exact` -> PASS, `1 passed`.
- `cargo test -p pwmd tests::v1_status_reports_neutral_relay_baseline_without_alias_shard -- --exact` -> PASS, `1 passed`.
- `cargo check -p pwmd` -> PASS.
- `cargo build -p pwmd --bin pwmd` -> PASS.

Notes:

- The first attempt at exact filters omitted the `tests::` prefix and ran `0 tests`; the corrected exact commands above passed.
- `v1_export_provenance_rejects_handoff_after_inbound_node_hello` is included in the `v1_export_provenance` group and confirms inbound/dev hello still does not grant provenance trust.
- S15-S3.4 one-window trusted relay core checks did not regress: self-attested handoff rejection, inbound-hello no-trust rejection, configured trusted peer acceptance, genesis guard failure, no-seed relay error state, and neutral relay baseline status all passed.

## Cleanup

- Cleaned: yes. All `pwmd` processes started for this validation were stopped, and `Get-Process pwmd` returned no remaining processes.
- Artifact cleanup: removed `target/debug/incremental`, reclaiming about `694.8 MB`.

## Open risks

No blockers found. The live reciprocal smoke, mismatch diagnostics, connect failure diagnostics, inbound trust boundary, and S15-S3.4 regression checks all passed.
