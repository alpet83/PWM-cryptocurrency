# Review: devnet terminology rename across docs (8e25afa)

- date: 2026-06-29
- ticket: `20260629-devnet-terminology-review`
- coding_ticket: `20260629-devnet-terminology`
- commit: `8e25afa` (per ticket; static tree review — `git show` unavailable in sandbox)
- scope: docs-only rename — `public testnet` → `devnet` in V7 context; genesis/runbook filenames; CONCEPT_ROADMAP boundary note

## 1. Scope recap

Coding ticket `20260629-devnet-terminology` at `8e25afa`:

| deliverable | expected | observed |
|-------------|----------|----------|
| Genesis manifest | `configs/devnet-genesis.json` | present; `chain_id`: `pwm-devnet-1` |
| Onboarding runbook | `docs/runbooks/devnet-validator-onboarding.md` | present; internal text uses devnet terminology |
| Phase boundary | CONCEPT_ROADMAP devnet vs Phase 4 public testnet | blockquote at line 26 |
| V7 docs consistency | no stale V7 `public testnet` / old filenames | see findings |
| Code/wire | none | no `crates/` string references to old filenames |

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | V7 `public testnet` / `testnet` → `devnet` in docs | **PASS** with nits | Grep `docs/` for `testnet-genesis`, `testnet-validator`, `pwm-testnet-1`, `v7-6-testnet`: **0 hits**. V7 artifacts renamed: `20260629-v7-6-devnet-review.md`, `20260629-v7-6-devnet-ramp.md`. ADR 0015, CHANGELOG v0.7.0, MVP-checklist §0v7 use devnet. **Nits:** `perf-optimization-spectrum.md:12` still says «публичного testnet»; ADR 0012 `:55` still «testnet-only». |
| 2 | Phase boundary in CONCEPT_ROADMAP | **PASS** | `CONCEPT_ROADMAP.md:26` — devnet = reproducible single-operator 21B genesis launch; public testnet = Phase 4 multi-validator BFT. Reinforced at `:516`, `:570-571`, V7 table row `:22`. |
| 3 | Genesis rename + code string refs | **PASS** | `configs/devnet-genesis.json` exists; `configs/testnet-genesis.json` absent. Grep `crates/` for `testnet-genesis` / `devnet-genesis`: **0 hits** (genesis loaded by operator path, not hardcoded). |
| 4 | Runbook rename + internal text | **PASS** | `devnet-validator-onboarding.md` title and body reference `pwm-devnet-1`, `configs/devnet-genesis.json`. Old `testnet-validator-onboarding.md` not present. |
| 5 | CHANGELOG v0.7.0 | **PASS** | `CHANGELOG.md:15` — «Devnet prep: `configs/devnet-genesis.json` …»; deferred section uses «live devnet ramp rerun». |
| 6 | Internal consistency post-rename | **PASS** with nit | CONCEPT_ROADMAP, MVP-checklist §0v7, CHANGELOG, reviews aligned. **Nit:** `mvp_v7.md` frontmatter still has `v7-6-devnet-launch` `in_progress`, `v7-7-bft-adr` / `v7-closeout` `pending`, and sprint table shows V7-6/V7-7 not closed while CONCEPT_ROADMAP marks V7 ✅ closed. |

## 3. Rename inventory (spot-check)

| path | status |
|------|--------|
| `configs/devnet-genesis.json` | renamed; `purpose`: `devnet-launch-candidate` |
| `docs/runbooks/devnet-validator-onboarding.md` | renamed + updated |
| `docs/reviews/20260629-v7-6-devnet-review.md` | renamed from testnet review naming |
| `docs/reviews/20260629-v7-6-devnet-ramp.md` | renamed; references `devnet-genesis.json` |
| `docs/adr/0015-bft-migration-gate.md` | V7 wording uses devnet throughout |
| `CHANGELOG.md` §v0.7.0 | devnet terminology |

**Intentionally retained `testnet` (non-V7 scope):** `mvp_v1_testnet_multi-sprint.md`, WHITE_SPEC v1 testnet sections, RFC v1 baseline, historical reviews (`v1-testnet-*`, `sprint-14-testnet-*`), MVP-checklist intro «v0/v1 testnet», Phase 4 qualified «public testnet» in GLOSSARY/MVP-checklist — correct per ticket notes.

## 4. Style and module shape

Docs-only slice — no Rust identifiers.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 5. Safety

No runtime or wire impact. Terminology change reduces operator confusion between V7 single-operator devnet and Phase 4 BFT public network.

## 6. Tests

No automated doc-link checker in repo. Manual grep confirms no broken `docs/` links to removed `testnet-genesis` / `testnet-validator-onboarding` paths.

## 7. Concurrency / parallelism

Concurrency / parallelism: not in diff scope (spot-check only: no new shared-state surfaces observed).

## 8. BLOCKERs

None.

## 9. Nits (non-blocking)

1. **NIT-1:** `docs/plans/perf-optimization-spectrum.md:12` — «публичного testnet» → «devnet» (V7 throughput gate context).
2. **NIT-2:** `docs/adr/0012-emergency-stake-evacuation.md:55` — `testnet-only` → `devnet-only` for V7 rollout default.
3. **NIT-3:** `docs/plans/mvp_v7.md` YAML todos + sprint table status stale vs CONCEPT_ROADMAP/CHANGELOG closeout (V7-6/7/closeout should be `done`).
4. **NIT-4:** `docs/CONCEPT_ROADMAP.md:97` V4 closeout sentence uses «публичного testnet announcement» without explicit Phase 4 qualifier (minor; later rows clarify).

## 10. Verdict

**Approve with nits** — V7 terminology rename is complete for primary deliverables (genesis, runbook, reviews, CHANGELOG, CONCEPT_ROADMAP boundary). No code references to old filenames. Two stray V7-context strings and `mvp_v7.md` status drift are documentation nits only.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-devnet-terminology-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 22000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-devnet-terminology-review.md'
git commit -m 'docs(v7): devnet terminology rename review (8e25afa)'
```