# Owner Decision Report: Old Tickets

Date: 2026-05-16  
Context: after V2-9 RFC16 closeout and V3 foundation closeout.

This report expands only the old tickets that need owner/product decision. It intentionally does not list every stale ticket.

## Decision Matrix

| Ticket / group | What it was about | Current reading | Recommended owner decision |
|---|---|---|---|
| `20260428-s14-slice6-v3-default-and-bruteforce-resume` | Wallet schema v3 by default + brute-force resume | Blocked by review because create path could merge old sensitive fields into a new wallet | Close as `wontfix/superseded` unless strict overwrite v3 create path is still desired |
| `20260429-s14-slice20-tx-routing-and-state-integrity` | Broad CY/DO routing + state rollback bug hunt | Most underlying state safety is now covered by later code/tests; original report is too broad | Archive as stale unless a current CY->DO loss can be reproduced |
| `20260430-s15-slice3-12-4-peer-protocol-churn-rootcause-fix` | Fix peer churn after S3.12.3 diagnostics | Historical failure was followed by S3.12.5/6, V2-8 wire hardening, V2-9 RFC16, V3 smoke | Archive historical ticket; open a fresh `pwm-debug` only if churn reappears now |
| `20260508-v2-sprint8-slice3-header-block-sync` | Header-first same-shard sync baseline | Coding/testing done; final review pending, then later V2-9 changed acceptance model | Close as `done_superseded`, no need to spend review budget now |
| V2-1 docs-only claim/purpose/policy package | Early RFC freeze for BurnMark purpose, ClaimTx, policy matrix | Contains useful design fragments, but later roadmap moved these topics into V4/V5 | Consolidate relevant parts into future V4/V5 RFC backlog; close old slice tickets as carried-forward |
| `20260430-s15-slice3-10-foreign-account-peer-lookup` | Authoritative foreign account balance/init via peer lookup | Still product-relevant, but not a V3/V4 foundation requirement | Move to V7/explorer/cross-shard UX backlog, not current runtime work |
| `20260421-phase1b-domain-index-sqlite-plan` | SQLite source for domain index | Questionable now: ClickHouse exists and may become main data source; domain DB need appears near lease mechanics | Park as `deferred_until_domain_leasing`; do not implement SQLite now |
| `20260422-pwmd-snapshot-canonical-only-and-self-verified` | Strict canonical/self-verified `pwm-data.json` | V3 solved current Epoch Snapshot schema/replay; Bootstrap Snapshot remains future work | Carry concept into Bootstrap Snapshot/cleanup-chain backlog, archive old Phase1 ticket |

## Details

### 1. Wallet v3 default + brute-force resume

Ticket: `tasks/20260428-s14-slice6-v3-default-and-bruteforce-resume.json`  
Current status: `blocked`.

Original intent:

- make wallet schema v3 the default creation path;
- align stdout wording with v3 terms;
- add address brute-force resume from an existing wallet.

Why it blocked:

- the conveyor ran (`pwm-coding`, `pwm-testing`, `pwm-review`);
- review returned `BLOCK`;
- the concrete risk was that the create path used merge-save and could inherit stale or sensitive fields when `wallet_out` already existed;
- proposed remediation was strict overwrite v3 for create path plus tests.

What changed later:

- wallet-first paths evolved;
- current code tolerates/strips legacy `active_account_id_hex`;
- V3 demo genesis introduced deterministic public demo wallet generation and did not depend on this old create-path change.

Options:

- **A. Close as `wontfix/superseded`**: accept that the old v3-default create-path ticket is no longer the right shape.
- **B. Re-slice strict wallet-create overwrite**: only if you still want a product rule that new wallet creation must hard-overwrite/clean legacy fields under a dedicated flag or command.

Recommendation: **A**. The original ticket mixed schema defaults, stdout UX, brute-force resume, and sensitive merge semantics. If strict wallet creation matters, it deserves a small fresh ticket, not resurrection of this blocked one.

### 2. S14 tx routing and state integrity

Ticket: `tasks/20260429-s14-slice20-tx-routing-and-state-integrity.json`  
Current status: `in_progress`.

Original intent:

- investigate intra-shard CY reject;
- investigate CY->DO balance loss;
- remove shard guard legacy behavior;
- add safe commit/rollback;
- prove E2E CLI flow.

What makes it hard to close blindly:

- this was a broad bug bucket with no completed artifact recorded in the ticket;
- it names serious symptoms, especially possible balance loss;
- it predates later cross-shard and sync work, so the original repro may no longer map to current code.

Current evidence:

- current `pwm-core` state layer rejects missing/uninitialized recipients without mutation;
- transfer/import paths now have rollback/no-drift tests;
- V2-8/V2-9 and V3 introduced much newer sync/attestation/public-devnet gates.

Options:

- **A. Archive as stale/superseded**: old symptoms are no longer actionable without a current repro.
- **B. Run a fresh current-code CY->DO E2E audit**: if you still suspect the old balance-loss class can survive.

Recommendation: **A by default**, **B only if you want assurance before V4**. If B is chosen, create a new task with current acceptance: “CY->DO transfer/import no balance loss under current RFC16/V3 devnet”, not this S14 umbrella.

### 3. S15 peer churn root cause

Ticket: `tasks/20260430-s15-slice3-12-4-peer-protocol-churn-rootcause-fix.json`  
Current status: `in_progress`.

Original intent:

- fix live peer churn after S3.12.3 diagnostics;
- stop `protocol_error` / `heartbeat_read_failed` after successful trusted session open;
- keep foreign-account unknown/unavailable semantics.

What happened:

- coding returned PASS;
- testing returned FAIL: focused checks passed, but live CY/DO smoke still repeated `heartbeat_read_failed`;
- review kept the history and suggested follow-up isolation via peer-only micro-node harness;
- follow-up S3.12.5/6 and later V2-8/V2-9 work moved the architecture forward.

What changed later:

- peer wire u128 stabilization happened;
- protocol semver and debug dump controls were added;
- RFC16 cluster attestation changed the acceptance model;
- V3 public devnet closeout passed for its target scope.

Options:

- **A. Archive as historical diagnosis**: preserve the artifact chain, stop treating the failed live smoke as current work.
- **B. Create current peer-stability task**: only if current CY lab still exhibits churn.

Recommendation: **A**. The old ticket should not remain `in_progress`. Any current churn should be investigated under a new `pwm-debug` ticket against current `crates/pwmd/src/transport/peer_session/**`.

### 4. V2-8 header-first sync slice

Ticket: `tasks/20260508-v2-sprint8-slice3-header-block-sync.json`  
Current status: `in_progress`.

Original intent:

- implement baseline same-shard native peer sync;
- announce tips/headers;
- fetch/apply blocks safely;
- enforce profile/shard gating and queue caps.

What happened:

- `pwm-coding`: done;
- `pwm-testing`: done;
- `pwm-review`: pending only because review was supposed to be rerun after testing artifacts appeared.

What changed later:

- V2-8 Slice 6 legacy wave acceptance was superseded;
- V2-9 RFC16 became the actual acceptance gate for cluster behavior;
- V3 foundation did not depend on this pending review.

Options:

- **A. Close as `done_superseded`**: implementation existed, later acceptance moved on.
- **B. Spend a short review to close historically cleanly**.

Recommendation: **A**. The review budget is better spent on current V4/V5 work. If there is a current sync defect, open a current-code bug.

### 5. V2-1 claim/purpose/policy docs package

Tickets:

- `tasks/20260505-v2-s1-s1-rfc-normative-freeze.json`
- `tasks/20260505-v2-s1-slice-a-tx-schema-purpose-claim.json`
- `tasks/20260505-v2-s1-slice-c-policy-matrix.json`

Original intent:

- freeze early RFC inputs for `purpose`, maturity/claim, free/paid claim, policy matrix, API taxonomy;
- docs-only, no product Rust edits.

What happened:

- useful docs and matrices were produced;
- several reviews returned PARTIAL/PASS;
- the tickets intentionally carried questions forward to Slice B/C/D and later code work.

What changed later:

- V3 clarified foundation scope and did not implement claim runtime;
- V4 is now policy engine / corporate INIT;
- V5 owns IPv4 distribution and tokenomics hardening.

Options:

- **A. Consolidate and close old V2-1 slices**: extract useful bits into a future V4/V5 RFC index/backlog.
- **B. Continue the V2-1 chain**: only if you want to preserve the old slice taxonomy.

Recommendation: **A**. These are not garbage, but they should become source material, not active sprint tickets.

### 6. Foreign account peer lookup

Ticket: `tasks/20260430-s15-slice3-10-foreign-account-peer-lookup.json`  
Current status: `in_progress`.

Original intent:

- avoid showing false zero balance for foreign accounts;
- query the home shard through peer path;
- show `unknown/???` when unavailable;
- keep local foreign view non-authoritative.

Why it still matters:

- product UX still needs a clean answer for cross-shard account visibility;
- future explorer/external API will need the same distinction.

Why not now:

- V3 public devnet did not require authoritative foreign-account reads;
- V4 focus is policy engine;
- this fits better with V7 external API/explorer or a future cross-shard UX slice.

Options:

- **A. Archive with old S15 chain**: if this UX is no longer important.
- **B. Re-slice under V7/explorer/cross-shard UX backlog**.

Recommendation: **B**. Keep the idea, discard the old active ticket.

### 7. Domain index via SQLite

Ticket: `tasks/20260421-phase1b-domain-index-sqlite-plan.json`  
Current status: `in_progress`.

Original intent:

- avoid hardcoding large domain lists in Rust;
- introduce SQLite-backed domain data and lookup API.

What changed:

- ClickHouse integration exists and may become the main operational data source;
- V3 explicitly kept `docs/DOMAINS.md` and `crates/pwm-core/src/domain_index.rs` as current runtime source of truth;
- domain allocation policy is still evolving: IT reserve clusters, `domain_lo = 0`, lease/auction lifecycle;
- real need for a database-backed domain registry appears closer to domain leasing, not now.

Options:

- **A. Close/defer SQLite specifically**: mark as `deferred_until_domain_leasing`, not an active implementation target.
- **B. Keep generic “domain registry data source” backlog**: without committing to SQLite.

Recommendation: **A + B combined**. Do not implement SQLite now. Replace the old ticket with a future neutral item: “domain registry data source for lease/auction era; evaluate ClickHouse vs embedded/static registry vs other store.”

### 8. Canonical-only/self-verified snapshot

Ticket: `tasks/20260422-pwmd-snapshot-canonical-only-and-self-verified.json`  
Current status: `in_progress`.

Original intent:

- reject unverifiable `pwm-data.json`;
- define canonical-only snapshot contract;
- safely migrate or refuse legacy/invalid data.

What changed:

- V3 added explicit Epoch Snapshot manifest `schema_v`;
- unsupported versions are rejected;
- replay determinism gate exists;
- cleanup-chain / Bootstrap Snapshot is explicitly deferred by ADR.

Options:

- **A. Archive old Phase1 ticket as superseded by V3 Epoch Snapshot work**.
- **B. Carry concept into future Bootstrap Snapshot hardening**.

Recommendation: **A for the old ticket, B for the concept**. This should not remain an active Phase1 task.

## My Proposed Owner Decisions

If you want a clean backlog with minimal ceremony:

1. Archive old wallet v3 default ticket as `wontfix/superseded`; create fresh strict-wallet-create ticket only if needed.
2. Archive S14 tx-routing umbrella unless you want one current CY->DO assurance smoke before V4.
3. Archive S15 peer-churn chain; investigate only current repros.
4. Close V2-8 header sync as `done_superseded`.
5. Consolidate V2-1 docs package into V4/V5 RFC backlog, then close old slice tickets.
6. Re-slice foreign account lookup under V7/explorer/cross-shard UX.
7. Defer SQLite domain index; later define a neutral domain registry data-source decision near lease/auction implementation, considering ClickHouse.
8. Archive old canonical-only snapshot ticket; carry the concept into Bootstrap Snapshot backlog.

