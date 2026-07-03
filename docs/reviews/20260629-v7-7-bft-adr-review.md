# Review: V7-7 BFT ADR-gate decision document (e882a49)

- date: 2026-06-29
- ticket: `20260629-v7-7-bft-adr-review`
- coding_ticket: `20260629-v7-7-bft-adr`
- commit: `e882a49` (claimed by ticket; `git show` unavailable in review sandbox — static tree review)
- scope: `docs/adr/0015-bft-migration-gate.md` (docs-only — no `crates/` changes expected)

## 1. Scope recap

V7-7 per `docs/plans/mvp_v7.md` §V7-7 BFT ADR-gate:

| deliverable | path |
|-------------|------|
| BFT migration decision ADR | `docs/adr/0015-bft-migration-gate.md` |
| Decision | V7 continues incremental PoS Option A; CometBFT/ABCI preferred Phase 4 study; custom Rust BFT not default |
| Captured contracts | `Chain::seal` boundary, RFC16 per path, rollback plan, mandatory V7-1 pipeline criterion |

Done criterion: Accepted ADR with migration rationale — **no BFT runtime code in V7**.

**Filename note:** ticket brief cites `0014-bft-migration-gate.md`; actual file is **ADR 0015** because `0014-account-hot-index-and-lockfree-chain.md` already occupies 0014. Numbering is correct; ticket path string is stale.

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| 1. ADR format (Context, Decision, Consequences; correct number) | **PASS** with nit | Status, Context, Decision, Candidate paths, Mandatory pipeline criterion, Chain::seal contract, RFC16, Rollback, Consequences, References (`0015-bft-migration-gate.md`). Number **0015** is correct given existing 0014. |
| 2. Three candidate paths with pros/cons | **PASS** | §Candidate paths A (incremental PoS), B (CometBFT/ABCI), C (custom Rust BFT) — each with Pros/Cons (`:26-68`) |
| 3. Mandatory pipeline criterion (V7-1 pre-processing not bottlenecked) | **PASS** | §Mandatory pipeline criterion (`:70-82`): explicit V7-1 lesson, five spike acceptance criteria, CometBFT ABCI hot-path constraint |
| 4. RFC16 cluster compat per path | **PASS** | §RFC16 cluster compatibility (`:106-112`): Path A unchanged; B keeps RFC16 as intra-validator layer; C must not collapse cluster + BFT |
| 5. Chain::seal boundary contract | **PASS** | §Chain::seal boundary contract (`:84-104`): preserved in V7; allowed Phase 4 changes listed; forbidden changes require new ADR |
| 6. Rollback plan actionable | **PASS** | §Rollback plan (`:114-128`): spike-failure steps (4) + preview-network failure steps (4) |
| 7. Docs-only — no Chain::seal code changes | **PASS** | ADR states no runtime/wire changes in V7 (`:16`, `:136`); no BFT implementation files in slice scope |

**Decision rationale:** present in §Decision (`:18-22`) — Option A for V7 devnet, CometBFT as preferred Phase 4 study with pipeline proof gate, custom BFT explicitly not default.

## 3. ADR content analysis

### Decision summary

- **V7 runtime:** continue V6 incremental PoS / Option A (`Chain::seal` unchanged).
- **Phase 4:** CometBFT/ABCI integration study is the preferred BFT path if inter-validator BFT is needed.
- **Rejected default:** custom Rust BFT replacing `Chain::seal` without separate ADR and safety budget.

### Pipeline gate (blocker criterion — satisfied)

The ADR treats V7-1 throughput work as a **hard architectural constraint** for any future consensus engine:

- Pre-processing before proposal/commit boundary.
- No serialized CPU bottleneck in consensus callbacks.
- Ramp harness regression gate referenced (`:80`).

This directly satisfies the ticket blocker: pipeline criterion is explicit and normative.

### Cross-ADR consistency

- References ADR 0013 (tx pipeline SEDA) and 0014 (account hot index) — aligns with V7-1 parallel pre-processing story.
- ADR 0013 line 194 still says "ADR V7-6" for BFT selection — stale label (should be V7-7); out of this slice but worth a one-line fix in a follow-up docs nit.

### Index hygiene

`docs/adr/README.md` index table stops at ADR 0012. ADRs 0013, 0014, and 0015 exist on disk but are not indexed — documentation drift, not a decision blocker.

## 4. Style and module shape

Docs-only slice — no production Rust identifiers to audit. ADR 0015 uses English prose (consistent with 0013/0014 body text). Minor style nit: 0014 has YAML frontmatter; 0015 does not — optional harmonization.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). ADR explicitly states no wire compatibility impact in V7 (`:136`).

## 5. Safety

- Correctly separates intra-validator scale (RFC16/sentry/worker) from inter-validator BFT — reduces risk of conflating layers.
- Rollback plan covers both research spike failure and deployed preview failure.
- Forbidden changes (nondeterministic policy, consensus-engine state mutation) are explicit (`:100-104`).

## 6. Tests

No automated test for ADR documents (expected). Phase 4 spike criteria reference existing V7 ramp harness — appropriate traceability.

## 7. Concurrency / parallelism

Concurrency / parallelism: not in diff scope (spot-check only: no new shared-state surfaces observed). ADR correctly notes parallel pre-processing must remain outside serialized consensus callbacks — aligns with V7 parallelism goals without introducing runtime concurrency changes.

## 8. BLOCKERs

None. Mandatory pipeline criterion is present; decision rationale is explicit; three paths analyzed with pros/cons; RFC16 and `Chain::seal` contracts documented; rollback plan is actionable.

## 9. Nits (non-blocking)

1. **NIT-1:** Update `docs/adr/README.md` index to include ADRs 0013, 0014, 0015.
2. **NIT-2:** Ticket/coding brief path `0014-bft-migration-gate.md` → align to `0015-bft-migration-gate.md` in task metadata.
3. **NIT-3:** Optional YAML frontmatter on 0015 for parity with 0014 (`adr`, `status`, `date`, `related`).
4. **NIT-4:** Fix ADR 0013 consequence line "ADR V7-6" → "V7-7" for BFT cross-reference.

## 10. Verdict

**Approve with nits** — ADR 0015 fully satisfies V7-7 focus areas: standard sections, three-path analysis, mandatory V7-1 pipeline criterion (blocker met), RFC16 per path, `Chain::seal` contract, actionable rollback, docs-only scope. Numbering as 0015 is correct; README index and ticket filename string need sync.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-v7-7-bft-adr-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 28000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-7-bft-adr-review.md'
git commit -m 'docs(v7-7): BFT ADR-gate review (e882a49)'
```