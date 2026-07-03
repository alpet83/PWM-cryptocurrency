# Review: MVP V5-8 Integrated Devnet Gate + Closeout

**Date:** 2026-05-28  
**Agent:** pwm-review  
**Ticket:** `20260524-v5-s8-slice5-closeout`  
**Parent:** `20260524-v5-sprint8-operator-closeout`  
**Commits reviewed:** fd94191, c930024, f5d4535, f21f243  

---

## 1. Scope recap

This is the **sprint-final review** for MVP V5-8. The sprint umbrella (`tasks/20260524-v5-sprint8-operator-closeout.json`) covers an end-to-end operator smoke harness for all four V5 feature lanes, followed by docs/traceability closure in this slice (5):

- **Slice1** — `scripts/devnet_v5_operator_smoke.ps1` skeleton: marks/inflation growth (`marks_last_block` cursor advance, `marks_saturated`).
- **Slice2** — Deferred policy activation: height-gated `tx-policy-set --activation deferred --activate-at-height`.
- **Slice3** — `ClaimIPv4Batch` happy path with test registry fixture and `ipv4_claimed_phase` assert; required ipv4 smoke-fix commit (f5d4535) after initial `E_POLICY_SCHEMA_INVALID` failure.
- **Slice4** — `pwm account-info` marks output: `marks_stored`, `marks_effective`, `marks_sat_pct`, `marks_last_block`, `staked`.
- **Slice5 (this slice)** — Docs-only: `docs/MVP-checklist.md`, `docs/CHANGELOG.md`, `docs/GLOSSARY.md`, this review report, umbrella ticket closure.

Checklist anchors: `docs/MVP-checklist.md §0v5 V5-8`, `docs/plans/mvp_v5.md#sprint-v5-8-integrated-devnet-gate-и-closeout`.

---

## 2. Requirements fit

| Acceptance criterion | Status |
|---|---|
| MVP-checklist V5-8 row marked `[x]` with links to PASS smoke reports | PASS — updated with all four report links and commit hashes |
| `docs/CHANGELOG.md` V5-8 entry (marks/deferred/ipv4/account-info operator smoke gate closed) | PASS — added as `V5-8 (2026-05-28)` section |
| `docs/GLOSSARY.md` new/refreshed terms for V5-8 operator flow | PASS — added section `## MVP V5: токеномика` with 8 new terms; alphabetical index updated in both Latin and Cyrillic blocks |
| Final review note in `docs/reviews/20260524-v5-sprint8-closeout-review.md` | PASS — this document |
| Update umbrella ticket status/notes/delegations for sprint closeout readiness | PASS — status set to `done`, `current_slice` updated, delegations extended |
| No product Rust changes (docs/tasks only in slice5) | PASS — confirmed; no crates/ edits |

All four preceding smoke reports confirm PASS at operator level:

| Slice | Report | Result | Key evidence |
|---|---|---|---|
| 1 marks | `tmp/devnet_v5_operator_smoke_20260524_192234.md` | PASS | `marks=saturated(4294967295) marks_last_block=0->1 head=1` |
| 2 deferred | `tmp/devnet_v5_operator_smoke_20260525_143518.md` | PASS | `activate_at=20 stored_active_policies=0 head=20 activate_exit_before=2` |
| 3 ipv4 | `tmp/devnet_v5_operator_smoke_20260528_080852.md` | PASS | `phase=7 delta=1000000 ipv4_claimed_phase==7` |
| 4 account-info | `tmp/devnet_v5_operator_smoke_20260528_085451.md` | PASS | `marks_effective=4294967295 marks_sat_pct=100 marks_last_block=1 staked=1000000000` |

---

## 3. Style and module shape

Slice5 is **docs/tasks only** — no Rust production code modified. Not applicable for naming policy check.

Smoke script (`scripts/devnet_v5_operator_smoke.ps1`) was created in slices 1–4 (already reviewed in `docs/reviews/20260524-v5-s8-slice2-op-smoke-deferred-review.md` and related). No new script added in slice5.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice; docs-only closeout).

---

## 4. Safety

- No production Rust code changed in this slice.
- Smoke reports reside under `tmp/` (gitignored ephemeral). PASS_EVIDENCE lines do not contain secrets — only public account hex hashes matching the demo genesis wallet, nonce values, and balance deltas.
- Glossary additions are prose-only; no code, no secrets.
- One residual **non-blocker**: smoke report `20260528_085451.md` ends with `WARNING: taskkill cleanup failed: ERROR: Access denied` — OS-level cleanup hiccup on Windows when processes exit before the kill command. This does not affect test validity; the node has already exited at that point. Recommend adding a `Stop-Process -ErrorAction SilentlyContinue` guard in the harness script (backlog nit, not V5-8 scope).

---

## 5. Tests

- Slices 1–4 constitute operator-level integration smoke tests (live devnet, full cargo build + run). All pass.
- Slice3 required a fix commit (f5d4535: genesis ipv4 phases loader + registry wallet + `ipv4_claimed_phase` API) before smoke passed — appropriate cycle, result is green.
- Unit / lib tests for the underlying V5 features (marks engine, deferred activation, ClaimIPv4Batch validate/apply) were covered in sprints V5-2 through V5-5; not re-run here (out of scope for closeout slice).
- No new tests needed for docs-only slice5.

---

## 6. Verdict

**APPROVE** — V5-8 sprint gate is complete.

All four operator smoke scenarios pass with machine-verifiable PASS_EVIDENCE tokens. Docs traceability (checklist, changelog, glossary) is updated. No product Rust changes in closeout slice. The single residual nit (taskkill cleanup warning) is a Windows process lifecycle detail, not a correctness issue.

**Verdict line:** `APPROVE — V5-8 integrated gate PASS; docs closed; no open blockers.`

---

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260524-v5-sprint8-closeout-review.md
token_usage:
  source: estimate
  input: 12000
  output: 1800
  total: 13800
  confidence: medium
```

---

## 8. Glossary traceability (sprint-final)

This is the **sprint-final review** for V5-8 (last sprint of MVP V5). `docs/GLOSSARY.md` updated:

New terms added in section `## MVP V5: токеномика (марки, инфляция, IPv4 claim)`:
- `Lazy marks / Ленивые марки` (`#term-lazy-marks`)
- `marks_last_block` (`#term-marks-last-block`)
- `Float inflation / compute_block_reward` (`#term-float-inflation`)
- `ClaimIPv4Batch` (`#term-claim-ipv4-batch`)
- `ipv4_claimed_phase` (`#term-ipv4-claimed-phase`)
- `Deferred policy activation` (`#term-deferred-policy`)
- `PASS_EVIDENCE` (`#term-pass-evidence`)
- `AccountInfoOnly` smoke mode (`#term-account-info-only`)

Alphabetical index (Latin + Cyrillic blocks) updated accordingly. Footer timestamp updated to 2026-05-28.

---

## 9. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260524-v5-sprint8-closeout-review.md'
git add 'docs/MVP-checklist.md'
git add 'docs/CHANGELOG.md'
git add 'docs/GLOSSARY.md'
git add 'tasks/20260524-v5-sprint8-operator-closeout.json'
git add 'tasks/in_progress/20260524-v5-s8-slice5-closeout.json'
git commit -m 'docs(v5-8): sprint closeout review, checklist, changelog, glossary'
```
