# Independent review: MVP V3 Sprint 4 — public devnet closeout (full V3 foundation)

**Date:** 2026-05-16  
**Ticket:** `tasks/20260516-v3-sprint4-public-devnet-closeout.json`  
**Scope (docs and process):** integrated smoke outcome, `docs/api-v1.md`, `docs/adr/*`, snapshot/replay operator docs, demo genesis scripts and runbook, plan `docs/plans/mvp_v3.md`, `CHANGELOG.md`, `tasks/*` traceability, `docs/GLOSSARY.md` sprint-final pass.

**Production Rust (`crates/`):** not re-audited in this pass; snapshot/replay code was gated in Sprint V3-2 review.

## 1. Scope recap

This review closes **MVP V3** as a **foundation** slice: versioned Epoch Snapshot manifest contract and replay determinism gate (V3-2), public `/v1/*` API freeze skeleton (V3-1), demo genesis package and CY runbook (V3-3), and **Sprint V3-4** integrated **public devnet** smoke plus final checklist/glossary traceability.

Referenced checklists: ticket `mvp_checklist` fields point to `docs/plans/mvp_v3.md` Sprint V3-4 and `docs/CONCEPT_ROADMAP.md` MVP V3 readiness language. `docs/MVP-checklist.md` still has **no explicit V3 rows** (see Findings).

## 2. Requirements fit

**Met (themes from orchestrator):**

- **`/v1/*` API skeleton / freeze boundary:** `docs/api-v1.md` separates public stable routes, operator routes, and dev-only routes; versioning policy documented; smoke examples aligned with `{ "accounts": [...] }` envelope (runbook and script echoes per ticket notes).
- **ADR IPv4 / offchain / cleanup-chain foundation:** `docs/adr/README.md` plus ADR 0002–0004 state **Draft (V3 foundation)** and each file has explicit **Deferred implementation boundaries (not part of V3)** — no accidental promise of V4/V5/V7 runtime in those ADRs.
- **Epoch Snapshot schema + replay gate:** `docs/guide-node-storage-and-snapshot.md` documents `schema_v = 1` contract, rejection of unsupported manifest versions, and `cargo test -p pwmd --lib v3_replay_det_gate_ok` with honest scope (fixture replay path, not full `epochs/` I/O). Aligns with prior V3-2 review conclusions.
- **Demo genesis with 21B PWM premine:** runbook premine math and `21000000000000000` raw target; verifier script in scope; `CHANGELOG` records Sprint 3 package.
- **Integrated public devnet smoke:** per ticket notes and updated `tasks/20260516-v3-sprint4-public-devnet-smoke.md`, retest **PASS** after deterministic demo wallet path: clean genesis path, premine verify, 3-node CY, `/v1/status`, `/v1/head`, `/v1/accounts`, `/v1/account/:id`, cleanup with zero remaining `pwmd`.

**Partial / gap:**

- **Smoke does not exercise `POST /v1/tx`** (only GET family documented in tester log). Acceptable for declared minimal smoke if intentional; recommend treating tx submission as **V4+** external integration or a follow-up smoke line item — not a V3 foundation blocker given api-v1 already marks tx as part of stable surface with template only.

## 3. Style and module shape

- **Docs/scripts slice:** naming and structure are consistent with existing repo docs; runbook clearly labels demo-only posture and public demo material (master seed, passphrase `12345`).
- No production Rust naming policy scan in this sprint-final (no new `crates/` review scope).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- **Public devnet assumptions:** runbook and API doc stress **non-production** security; demo passphrase and deterministic seed are **explicitly public dev material**, not secrets — satisfies review goal.
- **Trust boundaries:** integrated smoke uses local loopback and isolated `tmp/` state paths per notes; no new RPC trust claims beyond existing devnet posture.
- **Residual product risk (informational):** optional `-UseCountryBruteforce` path can still be slow or fragile; default path is deterministic — acceptable for foundation.

## 5. Tests

- **Reviewed as evidence (not re-executed here):** pwm-testing smoke report and ticket notes; guidance for replay gate and epoch manifest tests taken from `docs/guide-node-storage-and-snapshot.md` and Sprint V3-2 review.
- **Gap vs aspirational roadmap text:** `docs/CONCEPT_ROADMAP.md` MVP V3 readiness bullets still use checkbox wording (e.g. **schema_version** / migration genesis v4→v5) that does not literally match the shipped **`schema_v`** manifest contract — **documentation debt**, not a failed gate if the team treats the plan/tickets as authoritative.

## 6. Findings (severity order)

1. **Low — roadmap/checklist traceability:** `docs/CONCEPT_ROADMAP.md` section **MVP V3 — Критерии готовности** remains unchecked and uses vocabulary that may confuse readers (`schema_version` vs manifest `schema_v`; «одна команда» vs near-one-command scripts). **Owner decision:** update checkboxes and wording to match delivered V3 artifacts or add a pointer to `docs/plans/mvp_v3.md` as the execution source of truth.

2. **Low — `docs/MVP-checklist.md`:** no V3 entries found. **Owner decision:** add a V3 block or a single cross-link row if this file remains the program checklist.

3. **Low — smoke vs API freeze:** `POST /v1/tx` is in the stable list but not part of the recorded integrated smoke. **Mechanical / optional:** extend smoke doc with a one-line «not exercised in S4 harness» or a minimal negative/positive tx example in a later slice.

4. **Mechanical (addressed in this review pass):** `docs/plans/mvp_v3.md` listed Sprint V3-4 task as «будущий» filename; corrected to `tasks/20260516-v3-sprint4-public-devnet-closeout.json` and frontmatter `in_progress`. Smoke report header reconciled with retest PASS. `docs/api-v1.md` status line updated to reflect foundation closeout date.

## 7. Verdict

**PASS_WITH_NITS** — V3 foundation acceptance themes are satisfied; remaining items are **documentation/checklist housekeeping** and optional **tx smoke**, not regressions or unsafe public-devnet promises in reviewed artifacts.

---

## Participation / token estimate

```text
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/sprint-v3-4-public-devnet-closeout-review-20260516.md
token_usage: { "source": "estimate", "input": 45000, "output": 9000, "total": 54000, "confidence": "low" }
```

## Glossary

Updated `docs/GLOSSARY.md` with MVP V3 foundation section (public devnet, demo genesis, premine 21B raw, API freeze, Epoch Snapshot, Bootstrap Snapshot, cleanup-chain, replay determinism gate, ADR package) and alphabet index entries.

---

_End of report._
