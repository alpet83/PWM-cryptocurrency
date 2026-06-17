# Orchestrator notes (MVP v6+)

Дневник оркестратора: выжимка по слайсам для оптимизации handoff. Аудит — в `tasks/*.json`; здесь — уроки процесса.

## Шаблон записи (копировать на слайс)

| Поле | Значение |
|------|----------|
| `slice_id` | |
| `delegation_mode` | `worktree_bridge` \| `sync_task` |
| `conveyor_cycles` | |
| `agents` | |
| `token_estimate` | input / output / total |
| `efficiency_rating` | A \| B \| C |
| `reasoning_waste` | none \| low \| high |
| `lesson` | |

**Рейтинг:** A = 1 цикл; B = 2 или оправданный worktree; C = 3+ или плохой выбор режима.

---

## 20260605 — V6-1 spec / ADR freeze

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260605-v6-sprint1-spec-adr-freeze.json` |
| `delegation_mode` | `sync_task` |
| `conveyor_cycles` | 1 (+ nit-fix pass оркестратором) |
| `agents` | orchestrator (docs); pwm-review |
| `token_estimate` | review ~32.5k (estimate) |
| `efficiency_rating` | A |
| `reasoning_waste` | low |
| `lesson` | Spec-freeze: сразу фиксировать seal-tick refund и u128 JSON cross-ref в addenda — review поймал оба. |

**Артефакты:** ADR 0009–0011; `docs/rfc/addenda/v6-rfc*`; review `docs/reviews/20260605-v6-sprint1-spec-adr-freeze-review.md`.

---

## 20260605 — V6-2 core model (umbrella, открыт)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260605-v6-sprint2-core-model.json` |
| `delegation_mode` | `worktree_bridge` (git worktree вручную; bridge `create_worktree_branch` недоступен) |
| `worktree_root` | `P:/opt/docker/PWM-cryptocurrency-worktrees/v6-sprint2-core-model` |
| `branch` | `v6/20260605-v6-sprint2-core-model` |
| `conveyor_cycles` | slice 1 coding in progress |
| `agents` | pwm-coding (slice 1) |
| `lesson` | — |

**Слайсы:** 1 GenCfg → 2 ActivatePolicy wire → 3 state types → 4 reject stubs → 5 snapshot v4.

| Поле | Значение |
|------|----------|
| `slice_id` | umbrella `20260605-v6-sprint2-core-model` (closed) |
| `conveyor_cycles` | 5 (по 1 на слайс) |
| `efficiency_rating` | B |
| `reasoning_waste` | low |
| `lesson` | Worktree + последовательные слайсы держат diff узким; ручной git worktree ок при отсутствии bridge action. |

**Merge:** `d96f18c` на `main`. **Cleanup:** worktree и ветка удалены 2026-06-03 (`git worktree remove`, `git branch -d`).

---

## 20260605 — V6-3 stake admission (закрыт)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260605-v6-sprint3-stake-admission-coding.json` |
| `delegation_mode` | `worktree_bridge` + **bridge `share_ticket`** |
| `worktree_root` | `P:/opt/docker/PWM-cryptocurrency-worktrees/v6-sprint3-stake-admission` |
| `agents` | bridge `pwm-coding_32320` → pwm-review → pwm-testing |
| `conveyor_cycles` | 1 |
| `efficiency_rating` | A |
| `lesson` | Bridge + worktree сработали; review/testing оркестратором после submit. |

**Merge:** `2b1c7d5` + метаданные `8f8ce7d` на `main`. **Cleanup:** worktree и ветка удалены 2026-06-03.

---

## 20260603 — V6-4 leader rotation + V6-4b failover (закрыт)

| Поле | Значение |
|------|----------|
| `slice_id` | umbrella `20260603-v6-sprint4-leader-rotation-coding` + `20260606-v6-sprint4b-leader-failover-coding` |
| `merge` | `fad86d8` (rotation) + main `6d802b0` (failover, no worktree) |
| `delegation_mode` | bridge coding on main; review Cursor; testing bridge retest |
| `agents` | codex_coding_companion → pwm-review (Cursor) → copilot_testing_companion |
| `conveyor_cycles` | 2 (V6-4 partial + V6-4b tail) |
| `efficiency_rating` | B |
| `reasoning_waste` | low |
| `lesson` | Failover на main без worktree ок при одном файле; retest-тикет отдельно от coding bridge ticket; miss_skip вне default test_project.sh — явно в AC. |

**Follow-ups (non-blocking):** quorum-timeout miss; sync height gap (`issues-report.md`).

---

## 20260607 — V6-5 Mode B escrow (закрыт)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260607-v6-sprint5-mode-b-escrow-coding.json` |
| `merge` | `65768ba` (937bb83 + 835abdd + 7601287) |
| `agents` | codex_coding_companion → pwm-review (Cursor) → copilot_testing_companion |
| `conveyor_cycles` | 1 (+ orchestrator recovery submit) |
| `efficiency_rating` | B |
| `lesson` | Companion submit gap на coding; testing submit OK; smoke path `.\build_project.cmd`; worktree cleanup после merge. |

**Follow-ups:** pwmd preflight nit; federation V6-10.

---

## 20260607 — V6-6 COSIGN_NON_DISABLEABLE (закрыт)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260607-v6-sprint6-cosign-flags-coding.json` |
| `merge` | `34e121f` (b3750cf + fd08c52) |
| `agents` | codex_coding_companion → pwm-review → copilot_testing_companion |
| `conveyor_cycles` | 1 |
| `efficiency_rating` | A |
| `lesson` | Coding submit OK on second try; testing submit OK; no recovery needed. |

---

## 20260607 — V6-7 emergency sweep (закрыт)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260607-v6-sprint7-emergency-sweep-coding.json` |
| `merge` | `5339173` (85241e9 + 550434e) |
| `agents` | codex_coding_companion → pwm-review → copilot_testing_companion |
| `conveyor_cycles` | 1 |
| `efficiency_rating` | A |
| `lesson` | Coding submit OK; merge conflict только в tasks JSON (review commit дублировал файл). |

---

## 20260607 — V6-8 conservation delay (закрыт)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260607-v6-sprint8-conservation-coding.json` |
| `merge` | `f8c6ecf` (b9e0e1c + 1c0e8d0) |
| `agents` | codex_coding_companion → pwm-review → copilot_testing_companion |
| `conveyor_cycles` | 1 |
| `efficiency_rating` | A |
| `lesson` | Worker submit без git commit — orchestrator recovery commit b9e0e1c; merge conflict только в tasks JSON. |

---

## 20260608 — V6-9 slashing + peer score (закрыт)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260608-v6-sprint9-slashing-peers-coding.json` |
| `merge` | `f86368a` (7086434 + abde394) |
| `agents` | codex_coding_companion → pwm-review → copilot_testing_companion |
| `conveyor_cycles` | 1 (+ testing companion recovery) |
| `efficiency_rating` | A |
| `lesson` | Coding/testing submit без git commit / missing_reactive_inline — orchestrator recovery; merge conflict tasks JSON. |

---

## 20260608 — V6-10 CY soak (закрыт)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260608-v6-cy-e2e-umbrella.json` |
| `status` | done (2026-06-15) |
| `delegation_mode` | `operator_soak` |
| `efficiency_rating` | B |

| Волна | Итог | Урок |
|-------|------|------|
| **s1** bootstrap | PASS | Companion без reactive inline → bridge `failed/` при PASS-отчёте |
| **s2** legacy cross-shard | superseded | Roaming TTL ≠ Mode B refund на `unlock_height` |
| **s2c** Mode B refund | PASS | Короткий timeout + loader fix `cross_shard_lock_timeout_blocks` |
| **s3** conservation | PASS (retest) | Loader fix `conservation_delay_blocks` (`eaa288e`) |
| **s4** emergency sweep | PASS | `tmp/cy-e2e-v6-s4-20260615_170449.md` |

---

## 20260615 — V6-11 sprint-final closeout (done)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260615-v6-sprint11-closeout.json` |
| `status` | done — sprint gates only |
| `lesson` | Sprint-final ≠ publication. Owner path: 50k stability + rust audit + docs → sign-off → mirror. |

---

## 20260603 — V6 pre-publication (pending, owner-driven)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260603-v6-prepublication-umbrella.json` |
| `phases` | stability 50k → rust audit → docs/manuals → publication |
| `runbook` | `docs/runbooks/v6-owner-stability-soak-50k.md` |
| `audit_template` | `docs/reviews/20260528-v5-mvp-rust-code-audit-review.md` |

### 20260616 — phase `v6-prepub-rust-audit` done

| Поле | Значение |
|------|----------|
| `ticket` | `tasks/20260616-v6-mvp-rust-code-audit-review.json` |
| `artifact` | `docs/reviews/20260616-v6-mvp-rust-code-audit-review.md` |
| `window` | `522bcf1..3019528` (37 `.rs`) |
| `verdict` | needs attention — 0 Critical, 3 High, 5 Warning, 4 Note |
| `top_risks` | conservation no reserve + silent drain drop; mid-chain empty active set; failover without evidence/epoch hooks |
| `pending` | owner 50k soak + conservation delayed-tx spot-check; docs/manuals phase; High follow-ups before sign-off |

---

## 20260619 — trust-load fastpath (design catch-up)

| Поле | Значение |
|------|----------|
| `slice_id` | `tasks/20260619-pwmd-trust-load-fastpath-proposer-validation.json` |
| `delegation_mode` | `sync_task` (Cursor conveyor) |
| `conveyor_cycles` | 1 (+ boundary nit follow-up coding) |
| `agents` | pwm-coding → pwm-review → pwm-coding → pwm-testing |
| `efficiency_rating` | A |
| `lesson` | V6-3 persist `active_validator_indices` в snapshot v4, но trust-load до слайса игнорировал это и делал `1..tip` replay — beta cold start 15–20 min @125k. Догнали замысел: O(tail) + `trust_validate` progress. Owner: блоки почти сразу после рестарта post-fix. |

**Артефакты:** `crates/pwmd/src/snapshot/{io,incremental}.rs`; guide §Design alignment; RFC4 §9; review `docs/reviews/20260619-pwmd-trust-load-fastpath-proposer-validation-review.md`.

---

## Worktree lifecycle (норма V6)

Bridge + дефолты проекта (`.cqds/worktrees/`). Handoff без MCP-args. V6-2/V6-3 — ошибочный sibling-путь, не повторять.

Merge в main → метаданные → cleanup worktree и `v6/*` → sanity `git worktree list` → запись здесь.
