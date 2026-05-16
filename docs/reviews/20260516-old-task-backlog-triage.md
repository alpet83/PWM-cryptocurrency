# Old Task Backlog Triage

Date: 2026-05-16  
Scope: stale `tasks/*.json` entries with `open`, `in_progress`, or `blocked` status after V2-9 and V3 closeout.

This report does not change task statuses. It classifies old tickets and recommends which ones can be archived, closed as stale, or re-sliced.

## Summary

Most old open tickets are not active blockers for the current codebase. They fall into four buckets:

- **Stale status, completed work:** the ticket contains completed delegations/artifacts but was never flipped to `done`.
- **Superseded by V2-9/V3:** the old acceptance path was replaced by RFC 16 cluster attestation, V3 snapshot/replay gates, or the public devnet closeout.
- **Still useful backlog:** the idea remains valid, but the old ticket should not remain `in_progress`; it should become explicit backlog or be re-sliced under V4/V5/V7.
- **Owner decision:** the old ticket encodes product/architecture tradeoffs and should not be silently disposed.

Recommended cleanup policy:

- For stale completed tickets: set `status: done`, add `notes` with the closing evidence.
- For superseded tickets: set `status: done` or `status: archived` with `superseded_by`.
- For live backlog: set `status: backlog` / `open` and remove the misleading `in_progress`.
- For owner-decision items: keep open but add `decision_required`.

## High Confidence: Close As Stale Completed

### `tasks/20260509-protocol-versioning-debug-controls.json`

Current status: `in_progress`.

Evidence: both planned implementation slices have full `pwm-coding`, `pwm-testing`, and `pwm-review` records with PASS/approve-with-nits:

- Slice 1: build-control logs and protocol semver guard.
- Slice 2: divergence dump controls and time-align seal.
- Final artifacts include `docs/reviews/20260509-slice1-semver-build-control-final-review.md` and `docs/reviews/20260509-slice2-dump-timealign-final-review.md`.

Recommendation: mark `done`. Residual risks are already documented in the ticket and do not justify active `in_progress`.

### `tasks/20260508-v2-sprint8-slice3-header-block-sync.json`

Current status: `in_progress`.

Evidence: coding and testing are recorded as done; artifacts exist for implementation and testing. The remaining `pwm-review` entry is `pending`, but later V2-8/V2-9 work superseded the old same-shard sync acceptance path.

Recommendation: close as `done` or `done_superseded`, with note that final acceptance moved into V2-9/RFC 16.

### `tasks/20260429-s14-slice27-remove-wallet-active-account.json`

Current status: `in_progress`.

Evidence from current code: wallet v3 read/write paths tolerate or strip `active_account_id_hex`; CLI/TUI tests cover v3 wallets without active account and legacy active-key ignoring. Relevant current paths:

- `crates/pwm-cli/src/wallet/store.rs`
- `crates/pwm-cli/src/wallet/mod.rs`
- `crates/pwm-cli/src/tests/mod.rs`
- `crates/pwm-tui/tests/wallet_roaming.rs`

Recommendation: mark `done` after a quick verification pass; the old ticket appears completed but not closed.

### `tasks/20260428-s14-slice14-rows-to-accounts-refactor.json`

Current status: `in_progress`.

Evidence from current code: genesis schema ingestion uses `gen_cfg.funding.accounts`, not legacy `rows`, and current V3 demo genesis path depends on this. Relevant path: `crates/pwmd/src/snapshot/genesis.rs`.

Recommendation: mark `done` after confirming docs/examples no longer use the old `rows` field as current schema.

### `tasks/20260422-pwmd-startup-port-print-and-tui-lag-audit.json`

Current status: `in_progress`.

Evidence from current code: `pwmd` logs and file-logs the effective listen URL and peer/runtime identity on startup in `crates/pwmd/src/lifecycle.rs`. TUI lag audit is not part of this narrow startup-port goal.

Recommendation: split. Close startup-port portion as `done`; if TUI lag remains relevant, create a fresh backlog ticket instead of keeping this mixed ticket active.

## High Confidence: Archive As Superseded

### `tasks/20260508-v2-sprint8-slice6-automated-waves.json`

Current status: `blocked`.

Evidence: the ticket itself says the old Wave A/B/C acceptance on legacy multi-sealer was transferred to V2-9. `docs/CONCEPT_ROADMAP.md` and `docs/plans/mvp_v2.md` also treat V2-8 Slice 6 as non-blocking and superseded by V2-9/RFC 16.

Recommendation: archive as superseded by `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`.

### `tasks/20260508-v2-slice6-hotfix-tip-hash-divergence.json`

Current status: `blocked`.

Evidence: the ticket already contains closure text: root cause was tied to competing seal with one validator identity and header nondeterminism; continuing as a hotfix-only line is not useful.

Recommendation: archive as superseded by V2-9/RFC 16. Keep diagnosis artifacts for history.

### `tasks/20260508-wave-a-hash-parity-followup.json`

Current status: `in_progress`.

Evidence: several partial attempts ended in harness/test-mode complexity. Later V2-9 introduced the single proposer + attestation path that replaced the legacy Wave A parity acceptance model.

Recommendation: archive as superseded by V2-9. If a future deterministic harness is desired, create a new testnet/fault-matrix backlog ticket with a fresh acceptance model.

### `tasks/20260508-peer-compat-and-wire-stabilization.json`

Current status: `in_progress`.

Evidence: the u128 wire decode issue was fixed and reviewed; residual Wave A hash divergence was split into follow-up and then superseded by V2-9.

Recommendation: mark `done_superseded`: implementation merged/usable, old partial gate superseded.

### S15 peer-churn chain

Files:

- `tasks/20260430-s15-slice3-12-1-peer-churn-and-foreign-lookup-remediation.json`
- `tasks/20260430-s15-slice3-12-2-peer-churn-and-foreign-lookup-rootcause-remediation.json`
- `tasks/20260430-s15-slice3-12-4-peer-protocol-churn-rootcause-fix.json`
- `tasks/20260430-s15-slice3-12-5-peer-only-micronode-harness.json`
- `tasks/20260430-s15-slice3-12-6-production-idle-read-fix.json`

Current status: mostly `in_progress`.

Evidence: these are pre-V2-8/V2-9 transport debugging slices. Later work added peer wire stabilization, RFC 16 cluster attestation, protocol semver, divergence dumps, and V3 public devnet closeout. The old chain contains valuable historical diagnosis, but the active acceptance criteria are stale.

Recommendation: archive the chain as superseded/stale, preserving references to review artifacts. If peer churn reappears in current CY lab, open a new `pwm-debug` task against current `crates/pwmd/src/transport/peer_session/**`, not these old tickets.

## Still Actionable But Must Be Re-Sliced

### `tasks/20260506-v2-5-backlog.json`

Current status: `open`.

Evidence from current code:

- `crates/pwm-core/src/state.rs` still has ignored test `snap_keep_imp_replay_guard`.
- The doc comment on `accrue_marks()` still describes the old per-block formula.

Recommendation: keep as actionable backlog or re-slice as a small cleanup. This is one of the few old tickets that still maps directly to current code.

### `tasks/20260515-backlog-sync-tail-poll-block-age.json`

Current status: `open`.

Evidence: this is explicitly marked backlog and depends on stable same-shard sync. It is an optimization, not stale work.

Recommendation: keep open/backlog. Do not mark `in_progress`.

### `tasks/20260421-phase1b-domain-index-sqlite-plan.json`

Current status: `in_progress`.

Evidence: current V3 domain policy work explicitly kept `crates/pwm-core/src/domain_index.rs` as runtime source of truth. Moving specifically to sqlite-backed domain index is now questionable: ClickHouse integration already exists and may become the main operational data source, while the real need for a database-backed domain registry appears closer to domain lease/auction mechanics.

Recommendation: defer as `deferred_until_domain_leasing`. Do not implement SQLite now. If the need returns, re-slice as a neutral “domain registry data source” decision that evaluates ClickHouse, static/runtime registry, embedded DB, or another store after V4/V5 domain policy ADRs decide how domain codes and leases are allocated.

### `tasks/20260428-s14-slice8-style-refactor.json`

Current status: `in_progress`.

Evidence: style gates were later added to agent prompts and used in several slices, but a broad production identifier refactor is risky and not tied to V3.

Recommendation: do not keep as active. Convert to backlog or close as superseded by prompt/checker discipline unless the owner wants a dedicated style sprint.

### `tasks/20260428-s14-slice17-logger-integration.json` and `tasks/20260429-s14-slice18-logging-minimal-integration.json`

Current status: `in_progress`.

Evidence: runtime logging has since moved beyond this shape: file logging, focused targets, and the V3 runtime log-control operator RPC now exist.

Recommendation: archive old logger tickets as superseded by the logging/control work, but extract any still-desired rotation/template naming requirement into a fresh ops backlog item.

### `tasks/20260429-s15-architecture-genesis-consistency-and-db-snapshots.json`

Current status: `in_progress`.

Evidence: parts were handled by V3 schema/replay and demo genesis. Optional DB snapshots and broader genesis consistency remain future architecture topics.

Recommendation: close as superseded/partially-carried. Create new backlog only for DB snapshot backend if still desired.

## Likely Obsolete Or Covered By Later Code

### `tasks/20260429-s14-slice23-recipient-init-gate.json`

Current status: `in_progress`.

Evidence from current code: `State::require_recipient()` rejects missing and uninitialized recipients; transfer/import tests assert no mutation on missing/uninitialized recipient.

Recommendation: mark `done` after a quick targeted test pass.

### `tasks/20260429-s14-slice20-tx-routing-and-state-integrity.json`

Current status: `in_progress`.

Evidence: the current state layer includes rollback/no-mutation tests for missing/uninitialized transfer/import recipients, self-transfer handling, export/import replay protection, and V2/V3 smoke coverage. The old broad bug report is likely fragmented across later fixes.

Recommendation: archive as superseded. If a current CY->DO balance loss is observed, open a new focused bug with current logs.

### `tasks/20260429-s14-slice19-snapshot-persistence-investigation.json`

Current status: `in_progress`.

Evidence from current code: epoch persistence and runtime/monolithic save tests exist, including `runtime_persist` and disk-lag sync cases in `crates/pwmd/src/snapshot/incremental.rs`.

Recommendation: mark `done_superseded` after optional targeted snapshot test. The original symptom is likely resolved by later snapshot work.

### `tasks/20260422-pwmd-snapshot-canonical-only-and-self-verified.json`

Current status: `in_progress`.

Evidence: V3 added explicit `Epoch Snapshot` manifest schema contract, unsupported-version rejection, replay determinism gate, and operator docs. The old “canonical-only pwm-data.json” ticket predates the newer epoch/manifest split.

Recommendation: archive as superseded by V3 snapshot schema/replay work. If stricter monolithic `pwm-data.json` validation remains desired, re-slice it as a V4/V5 storage-hardening item.

### `tasks/20260429-s14-slice21-snapshot-hex-refactor.json`

Current status: `in_progress`.

Evidence: current V3 path uses schema-versioned epoch manifest and genesis schema v4/v5; hex-string refactor is no longer the main snapshot correctness gate.

Recommendation: archive unless there is a current storage-size/perf issue. Re-slice only as storage format optimization.

### `tasks/20260428-s14-slice13-e2e-cy-do-investigation.json`

Current status: `in_progress`.

Evidence: broad CY->DO flow investigations were superseded by S15, V2-8, V2-9, and V3 public devnet smoke. The original issue is too old to use as active signal.

Recommendation: archive as stale. Reopen only as a fresh current-code E2E if a CY->DO bug is reproduced.

### `tasks/20260428-s14-slice12-docs-sync.json`

Current status: `in_progress`.

Evidence: V3 closeout updated roadmap/checklist/glossary/API docs, and stale port/API docs were revisited multiple times. The old docs-sync scope is too broad.

Recommendation: archive as superseded. Use narrow doc tickets for current inconsistencies.

## Owner Decision / Product Direction

### V2-1 docs-only slices

Files:

- `tasks/20260505-v2-s1-s1-rfc-normative-freeze.json`
- `tasks/20260505-v2-s1-slice-a-tx-schema-purpose-claim.json`
- `tasks/20260505-v2-s1-slice-c-policy-matrix.json`

Current status: `in_progress`.

Evidence: these contain real docs-only RFC work with PARTIAL/PASS review history and carry-over to later slices. Some concepts have since moved into V4/V5 planning.

Recommendation: do not silently archive. Add a single owner-decision note: either close them as superseded by the current `CONCEPT_ROADMAP.md`, or consolidate their still-relevant claim/burn/purpose/API details into a fresh V4/V5 RFC backlog.

### `tasks/20260430-s15-slice3-10-foreign-account-peer-lookup.json`

Current status: `in_progress`.

Evidence: the idea is still product-relevant for UX/explorer behavior, but current V3 did not require authoritative foreign account lookup. V2-9 shifted the live cluster architecture.

Recommendation: owner decision. Either archive with S15 peer-churn chain, or re-slice under V7 external explorer / cross-shard UX.

## Suggested Cleanup Batch

Safe first batch:

1. Mark clearly completed stale tickets as `done`: protocol debug controls, header-block sync, wallet active-account removal, rows-to-accounts, startup listen print.
2. Mark V2-8 legacy wave tickets as superseded by V2-9: automated waves, tip-hash hotfix, wave-a hash parity follow-up, peer-compat partial residual.
3. Archive S15 peer-churn chain with a single note: “historical diagnosis, superseded by V2-8/V2-9 transport and RFC16 path.”
4. Keep only two clearly actionable old backlog entries as live: `20260506-v2-5-backlog.json` and `20260515-backlog-sync-tail-poll-block-age.json`.
5. Ask owner on product-direction groups: domain registry data source near leasing (not SQLite now), V2-1 claim/purpose policy docs, foreign-account peer lookup UX.

