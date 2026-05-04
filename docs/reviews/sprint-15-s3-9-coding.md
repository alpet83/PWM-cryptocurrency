# Sprint 15 S15-S3.9 Coding Review

Implemented a bounded cross-shard export/import ledger for the current MVP node runtime.

- Scope stays inside shard nodes; no global chain or full block mirroring was added.
- Runtime now records export, handoff registration, import, amount, domains, ids, and status facts.
- `/v1/status` exposes compact totals and per-domain `cross_shard_summary`.
- Snapshots persist the bounded fact ledger alongside existing roaming state.
- Stateful peer transport exchanges fact snapshots after trusted handshakes; untrusted inbound sessions do not merge peer-provided facts.
- Nodes log a compact `export/import summary` every 500 sealed blocks.

Validation target:

- `cargo fmt`
- focused `pwmd` tests for ledger recording, snapshot persistence, summary formatting, and status exposure

## Remediation

Review found two blockers:

- snapshot-save rollback did not restore `cross_shard`;
- trusted peer observations were merged into summary without an explicit scope label.

Fixes:

- `CommitBak` now includes `cross_shard`, so failed snapshot save rolls back ledger facts together with chain/state/roaming/flow.
- `cross_shard_summary` now reports `scope=local_plus_trusted_peer_observations`.
- Summary/log output includes `trusted_peer_observed_count`.
- Snapshot wire preserves fact `origin`.
- Added focused tests for peer-observation labeling and export rollback without phantom ledger facts.
