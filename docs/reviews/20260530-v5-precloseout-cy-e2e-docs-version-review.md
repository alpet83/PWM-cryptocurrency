# Review: V5 pre-closeout CY E2E + V5 doc/version alignment (post-umbrella)

**Date:** 2026-05-30  
**Agent:** pwm-review  
**Ticket:** `20260530-v5-precloseout-cy-e2e-docs-version-review`  
**Verdict:** PASS_WITH_NITS

---

## 1. Scope recap

Per ticked `doc_scope`, audit across 5 categories for V5 semantics correctness, gate status accuracy, stale ClaimTx references, and canonical evidence cross-links.

---

## 2. Per-file inventory

### 2.1. Plans — `docs/plans/mvp_v5.md`

| Check | Result |
|---|---|
| V5-9 gate reflects s1/s2-rerun/s3 PASS + umbrella done | **PASS** — lines 391–409: s2 = `20260531-v5-cy-e2e-s2-marks-saturation-soak-rerun`, s3 PASS, umbrella done (owner sign-off 2026-05-30) |
| V5-8 gate still has closeout scope content | **OK** — placeholder content is normative scope, not stale |
| No ClaimTx / legacy marks-accrual-on-seal claims | **PASS** — only formal retirement/removal references, no active path |
| Correct harness names, report paths | **PASS** — `tmp/cy-e2e-s1-20260528_220256.md`, `tmp/cy-e2e-s2-20260530_082418.md`, `tmp/cy-e2e-s3-20260530_141317.md` all listed |

**Verdict: OK ✅**

### 2.2. Runbooks — `docs/runbooks/v5-cy-cluster-precloseout-soak.md`

| Check | Result |
|---|---|
| Gate section ticket IDs | **STALE** — line 78: `20260529-v5-cy-e2e-s2` без `-rerun`. Живой тикет — `20260531-v5-cy-e2e-s2-marks-saturation-soak-rerun`. Fix: добавить `-rerun` суффикс и статус `PASS (PARTIAL: 2 staked)` |
| Harness scripts table | **MISSING** — нет `scripts/cy_cluster_marks_soak.py` (Python REST-only harness) и `scripts/cy_cluster_mass_burn_soak.ps1` (s3 burn batch). Fix: добавить оба скрипта в таблицу (раздел «Связанные скрипты») |
| TUI marks path cross-link | **PASS** — line 53: ссылка на `docs/runbooks/v5-tui-marks-operator-path.md` |
| No ClaimTx / legacy claims | **PASS** — чистый V5 runbook, marks accumulation, BurnMark, PIN |

**Verdict: STALE (2 fixes)**

### 2.3. Runbooks — `docs/runbooks/v5-tui-marks-operator-path.md`

| Check | Result |
|---|---|
| V5 operator path (S → wait → F5) | **PASS** |
| ClaimTx vs ClaimIPv4Batch distinction | **PASS** |
| Linked from soak runbook | **PASS** |

**Verdict: OK ✅**

### 2.4. Runbooks — `docs/runbooks/devnet-v5-operator-smoke.md`

| Check | Result |
|---|---|
| V5 semantics (lazy marks, blocks_per_hour, no ClaimTx) | **PASS** |
| ClaimIPv4Batch slice documented | **PASS** |
| Known Limitations accurate | **PASS** |

**Verdict: OK ✅**

### 2.5. Runbooks — `docs/runbooks/cy-cluster-policy-matrix-e2e.md`

| Check | Result |
|---|---|
| V4 runbook — no V5 stale claims | **PASS** — корректно ссылается на V4 policy matrix, zero V5 interference |
| CQDS MCP testing pattern | **PASS** |

**Verdict: OK ✅**

### 2.6. Checklists/Roadmap — `docs/MVP-checklist.md`

| Check | Result |
|---|---|
| §0v5 header | **STALE** — `(in progress)` должен быть близок к closeout. Fix: `(CY E2E passed; docs alignment in review)` или аналогично |
| V5-9 gate row | **MISSING** — нет строки для `[x]` V5-9 pre-closeout CY E2E gate (s1/s2-rerun/s3 PASS, umbrella done). Fix: добавить строку между V5-8 и следующим разделом |
| V5-1 … V5-8 rows | **PASS** — все `[x]`, с ревью-ссылками |
| V5-8 gate details | **PASS** — line 58: ссылки на smoke reports и review |

**Verdict: STALE + MISSING (2 fixes)**

### 2.7. — `docs/CONCEPT_ROADMAP.md`

| Check | Result |
|---|---|
| V5 status in overview table | **STALE** — line 16: `🔄 In Progress`. Должен быть близок к `✅` с пометкой о pending doc alignment. Fix: пометить как near-complete |
| V5-8 readiness criteria | **STALE** — line 470: `[ ] **V5-8** Integrated gate + closeout`. Уже done. Fix: пометить `[x]` |
| V5-9 CY E2E readiness | **MISSING** — нет строки для V5-9. Fix: добавить `[x] **V5-9** CY cluster multi-hour E2E: s1/s2-rerun/s3 PASS` в §V5 readiness criteria |

**Verdict: STALE (3 fixes)**

### 2.8. READMEs / guides — `README.md`, `README-ru.md`, `docs/pwm-cli.md`, `docs/tester-guide-env-errors-recovery.md`

| Check | Result |
|---|---|
| No stale ClaimTx / V4 marks references | **PASS** — поверхностный grep: без проблемных строк |

**Verdict: OK ✅** (глубокий audit этих файлов не проводился — только grep ClaimTx/marks; приоритет низкий)

### 2.9. Spec/RFC — `docs/rfc/12-claim-maturity-and-state-model.md`, `docs/GLOSSARY.md`

| Check | Result |
|---|---|
| RFC 0012 v2 содержит полную lazy model | **PASS** — marks_last_block, saturation, touch-semantics |
| GLOSSARY V5 entries | **PASS** — lazy marks, saturation, deferred, ClaimIPv4Batch |
| No active ClaimTx in normative docs | **PASS** — всё упоминание — retirement/removal |

**Verdict: OK ✅**

### 2.10. Prior E2E reviews — existence check

| File | Status |
|---|---|
| `docs/reviews/20260529-v5-cy-e2e-s2-marks-saturation-soak-review.md` | Exists ✅ |
| `docs/reviews/20260530-v5-tui-marks-operator-journey-review.md` | Exists ✅ |
| `docs/reviews/20260530-v5-tui-marks-copy-observability-post-review.md` | Exists ✅ |
| `docs/reviews/20260530-v5-marks-mechanics-proposer-log-review.md` | Exists ✅ |

### 2.11. Harness scripts — existence check

| Script | Status |
|---|---|
| `scripts/cy_cluster_marks_soak.ps1` | Exists ✅ |
| `scripts/cy_cluster_marks_soak.py` | Exists ✅ (additional, not in original ticket scope) |
| `scripts/cy_cluster_mass_burn_soak.ps1` | Exists ✅ |
| `scripts/cy_cluster_two_node_smoke.ps1` | Exists ✅ |
| `scripts/devnet_v5_operator_smoke.ps1` | Exists ✅ |

### 2.12. Canonical evidence files — existence check

| Report | Status |
|---|---|
| `tmp/cy-e2e-s1-20260528_220256.md` | Exists ✅ |
| `tmp/cy-e2e-s2-20260530_082418.md` | Exists ✅ |
| `tmp/cy-e2e-s3-20260530_141317.md` | Exists ✅ |

---

## 3. Fix summary (actionable nits)

| # | File | Section | Issue | Fix |
|---|------|---------|-------|-----|
| F1 | `docs/runbooks/v5-cy-cluster-precloseout-soak.md:78` | Gate | `20260529-v5-cy-e2e-s2` без `-rerun` | → `20260531-v5-cy-e2e-s2-marks-saturation-soak-rerun PASS (PARTIAL: 2 staked)` |
| F2 | `docs/runbooks/v5-cy-cluster-precloseout-soak.md:59-67` | Связанные скрипты | нет soak-скриптов | добавить `cy_cluster_marks_soak.py`, `cy_cluster_mass_burn_soak.ps1` |
| F3 | `docs/MVP-checklist.md:47` | §0v5 header | `(in progress)` устарел | → `(CY E2E passed; docs alignment in review)` |
| F4 | `docs/MVP-checklist.md:58-59` | §0v5 table | нет V5-9 строки | добавить `[x] **V5-9 pre-closeout CY E2E:** s1/s2-rerun/s3 PASS; umbrella done; reports tmp/cy-e2e-s{1,2,3}-*.md` |
| F5 | `docs/CONCEPT_ROADMAP.md:16` | Overview table | V5 `🔄 In Progress` | → `🔄 Near Complete (doc alignment pending)` |
| F6 | `docs/CONCEPT_ROADMAP.md:470` | V5 readiness | V5-8 `[ ]` | → `[x]` |
| F7 | `docs/CONCEPT_ROADMAP.md:470` | V5 readiness | нет V5-9 | добавить `[x] **V5-9** CY cluster multi-hour E2E: s1/bootstrap, s2/marks saturation soak (rerun), s3/mass burn batches PASS` |

---

## 4. Cross-link audit

| Link path | Status |
|---|---|
| Runbook → v5-tui-marks-operator-path | **PASS** (soak.md:53) |
| mvp_v5.md → V5-9 gate tickets | **PASS** (lines 399-404) |
| mvp_v5.md → canonical reports | **PASS** (line 407) |
| MVP-checklist → mvp_v5 plan | **PASS** (line 15) |
| MVP-checklist → V5-9 gate | **MISSING** (см. F4) |

---

## 5. Verification performed

- `grep ClaimTx/claim_mark` across docs/runbooks, docs/plans — only retirement references
- `grep e2e-s2` across docs/runbooks — found stale s2 reference
- `grep V5-9` across MVP-checklist — none found
- `grep V5.*completed/closeout` across CONCEPT_ROADMAP — found stale V5-8 and missing V5-9
- File existence checks for 4 reviews, 5 scripts, 3 evidence reports

---

## 6. Verdict

**PASS_WITH_NITS** — 2/12 files have actionable doc-fix issues (runbook gate stale ticket ID + missing harness scripts; MVP-checklist missing V5-9 row; CONCEPT_ROADMAP stale V5 status + missing V5-9). 7 nits total, all trivial copy/text edit, no product code required. No coding ticket needed — orchestrator can fix inline.

**Verdict line:** `PASS_WITH_NITS — 7 trivial doc nits (stale s2 ticket id, missing V5-9 in checklist/roadmap, harness scripts table); all fixable inline by orchestrator.`

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260530-v5-precloseout-cy-e2e-docs-version-review.md
token_usage:
  source: estimate
  input: 18000
  output: 4200
  total: 22200
  confidence: medium
```

---

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260530-v5-precloseout-cy-e2e-docs-version-review.md'
git commit -m 'docs(v5-precloseout): doc alignment review PASS_WITH_NITS'
```