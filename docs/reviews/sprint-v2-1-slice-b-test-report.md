# Sprint V2-1 — Slice B: testing gate report (docs-only)

**Дата:** 2026-05-05  
**Агент:** `pwm-testing`  
**Входы:** [sprint-v2-1-slice-b-state-freeze.md](./sprint-v2-1-slice-b-state-freeze.md), [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md)

---

## Verdict: **PARTIAL**

**Обоснование:** шесть заявленных осей freeze дают однозначные, проверяемые предикаты для будущих тестов (unit/replay/integration). При этом в тест-матрице остаются семантические/политические хвосты (округление N-MAT-5, полная политическая матрица mempool/apply, стабильные коды API — C/D), а исполняемые тесты в `crates/*` в рамках этого слайса не добавлялись. Для «чистого» PASS потребовались бы зафиксированное в B правило округления `P-MAT-06` и закрытые коды/трассы отклонений (D).

---

## 1. Тестопригодность freeze по осям

| Ось | Оценка | Комментарий |
|-----|--------|--------------|
| **Staked balance semantics (B-STATE-1)** | Testable | `B(h) = staked_pwm_units(h)` задаёт единственный базис для maturity; отрицательные кейсы: движение `liquid`/marks без смены stake не должны сбрасывать непрерывность по B. |
| **`anchor_ref` monotonicity (B-STATE-2)** | Testable | Предикаты: `anchor_ref <= inclusion_height`, `anchor_ref >= last_claim_anchor_ref`, несовместимость с state на высоте включения → отклонение. |
| **`claim_units` bounds (B-STATE-3)** | Testable | `0 < claim_units <= matured_units_available(...)`; пост-условия на `marks`, `last_claim_anchor_ref`, уменьшение `matured_credit` — детерминированы при заданной модели интервалов. |
| **Reset on any balance change (B-STATE-4)** | Testable | Любое `B(h) != B(h-1)` обрывает интервал; покрывает частичные дельты, slashing при изменении `staked_pwm_units`. Закрывает placeholder **P-RST-03** на уровне нормы (полный сброс, не пропорциональная модель). |
| **UTC-day chain-time marker (B-STATE-5)** | Testable | `utc_day = floor(block_unix_time_utc / 86400)`; wall-clock/node/client timezone вне правил. **P-FRE-06** намеренно замыкается на chain time; точные коды ошибок — Slice D. |
| **Reorg replayability (B-STATE-6)** | Testable | `claim_state` и free-marker — replayable state с полным откатом; orphan-ветка не оставляет эффектов; детерминизм по canonical префиксу. **P-REO-01–03** получают baseline; **P-REO-04** (детали формулировки отката в краевых порядках) — уточнение в C при реализации. |

---

## 2. Что закрыто в Slice B (для тест-дизайна)

- Релевантный баланс для maturity: только `staked_pwm_units`.
- Семантика `anchor_ref` (опорная высота, монотонность относительно `last_claim_anchor_ref`, верхняя граница inclusion).
- Семантика `claim_units` как целой материализуемой дельты с ограничением сверху доступным matured-credit и атомарным обновлением state.
- Единое правило сброса непрерывности при любом ненулевом изменении `B` (включая частичное).
- Канонический free-day маркер и правило «одна free за `utc_day`» от chain time.
- Baseline reorg/rollback: полный replay canonical ветки, без побочных эффектов orphan-блоков.
- Явный список инвариантов state machine (§4 freeze) для свойств-тестов и replay-инвариантов.

---

## 3. Пробелы, перенесённые в Slice C / D

**Slice C (ожидается):**

- **P-MAT-06 / N-MAT-5:** в B не зафиксировано окончательное правило округления «усечение в пользу сети» vs «sub-quantum remainder» — без этого нельзя зашить эталонные численные ожидания в тестах.
- Полная **policy-validation matrix:** комиссии, mempool vs apply vs preflight ordering, комбинации с claim/free (строки вида P-MAT-*, P-RST-02, P-FRE-* на стыке исполнения).
- Углубление **P-REO-04:** порядок краевых событий при reorg (если появятся дополнительные policy-ограничения поверх B-STATE-6).

**Slice D (ожидается):**

- Стабильные **коды и формы ошибок** для отклонений claim/free (`FREE_CLAIM_DAILY_LIMIT`, anchor/дельта, и т.д.) — **P-PUR-07**, отрицательные ветки P-FRE/P-MAT с симметрией API.
- Трассы/поля для rejection path, чтобы негативные тесты CLI/API были однозначны.

**Не является пробелом B:** отсутствие кода — ожидаемо для docs-only слайса; тестопригодность спецификации по шести осям признана достаточной для проектирования тестов.

---

## Participation / token estimate (`pwm-testing`)

```yaml
agent: pwm-testing
result: PARTIAL
artifacts:
  - docs/reviews/sprint-v2-1-slice-b-test-report.md
  - tasks/20260505-v2-s1-slice-b-state-maturity-freeclaim.json
token_usage:
  source: estimate
  input: null
  output: null
  total: 3100
  confidence: low
```

_Оценка по объёму норм B и сопоставлению с матрицей; без провайдера токенов._
