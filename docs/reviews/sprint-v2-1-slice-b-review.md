# Sprint V2-1 — Slice B: independent review gate (state freeze)

**Дата:** 2026-05-05  
**Ревьюер:** `pwm-review`  
**Входы:** [sprint-v2-1-slice-b-state-freeze.md](./sprint-v2-1-slice-b-state-freeze.md), [sprint-v2-1-slice-b-test-report.md](./sprint-v2-1-slice-b-test-report.md), `tasks/20260505-v2-s1-slice-b-state-maturity-freeclaim.json`  
**Ограничение:** docs-only; `crates/*` не рассматривались как предмет изменений.

---

## 1) Scope recap

Слайс B заявляет RFC-freeze **state-семантики** для maturity / claim / free-day: релевантный баланс `B(h) = staked_pwm_units(h)`, поля `anchor_ref` и `claim_units`, сброс непрерывности при любом ненулевом изменении `B`, канонический `utc_day` от chain time, baseline reorg/rollback и инварианты state machine. Связь с Slice A (схема ClaimTx на tx-уровне) не противоречит freeze: семантика полей дополняет черновик схемы. В `mvp_checklist` тикета указан блок «§1 Спецификация и решения» — содержательно B закрывает state-часть дорожной карты до уровня тестопригодных предикатов, оставляя исполнение/policy/API на C/D (как и заявлено в §5 freeze и в тест-отчёте).

---

## 2) Requirements fit

**Соответствие цели слайса:** высокое. Шесть нормативных блоков (B-STATE-1..6) дают проверяемые правила для проектирования replay и негативных веток без обращения к коду.

**Пробелы (ожидаемые, согласованы с pwm-testing):** правило округления P-MAT-06 / N-MAT-5, полная policy-validation matrix (mempool/apply/preflight), уточнение краевых порядков P-REO-04, стабильные коды/trace отклонений (Slice D) — **вне замыкания B**, но должны быть явно захвачены в C/D, иначе регресс в трассируемости требований.

**Внутренние уточнения freeze (не блокеры для handoff, см. findings):** согласование типа маркера free-day между §2 и §3; явность предиката «несовместимости» anchor; крайний случай первой высоты/инициализации для правила `B(h) != B(h-1)`.

---

## 3) Style and module shape

Продакшн-Rust в scope слайса отсутствует. Документ структурирован последовательно (цель → нормы → модель → инварианты → связь с матрицей). Рекомендация для Slice C: при добавлении policy-матрицы сохранить тот же уровень **предикатной** ясности (условие → ожидаемый исход), чтобы не размыть B-STATE-*.

---

## 4) Safety

**Позитивно:** явный отказ от wall-clock/timezone клиента для free-day; требование полного отката `claim_state` и free-marker при reorg; монотонность anchor и cap по `matured_units_available` снижают классические классы ошибок (двойной claim сверх кредита, недетерминизм по времени).

**Риски спецификации (не уязвимости кода):** пока предикат отклонения по «несовместимости anchor с state» не формализован, возможны расхождения реализации между нодами — это переносится в Slice C как часть validation matrix (согласуется с PARTIAL от pwm-testing).

---

## 5) Tests

Исполняемые тесты в `crates/*` слайсом не добавлялись — **ожидаемо** для docs-only. Тест-отчёт pwm-testing признаёт шесть осей freeze **testable** и фиксирует перенос численных эталонов и API-негатива на C/D. С позиции ревью: этого **достаточно** для gate B; отсутствие эталонных чисел по округлению — осознанный пробел до P-MAT-06.

---

## 6) Findings by severity

### Low

- **L1.** В B-STATE-5 для `last_free_claim_utc_day` указано `u32|u64`, в §3 baseline — только `u64`. Для freeze wire/state лучше единый выбранный тип или явное «wire: …, internal: …».
- **L2.** Перекрёстные ссылки на `P-*` в §5 можно в следующих слайсах выровнять с точными идентификаторами строк test-matrix (косметика трассируемости).

### Medium

- **M1.** Формулировка B-STATE-2 «при несовместимости anchor с каноническим state на высоте включения» не раскрывает минимальный набор предикатов (что именно сравнивается с чем). Handoff на Slice C должен либо ссылаться на явную таблицу отказов, либо расширить норму B одним абзацем предикатов — иначе риск разночтений между implementers.
- **M2.** Правило B-STATE-4 в форме `B(h) != B(h-1)` не оговаривает инициализацию при первом появлении `B` / высоте genesis: для детерминированного replay полезно одно предложение baseline (например, трактовка отсутствующего `h-1` или явный «bootstrap height» в state schema).

### High

- **Нет** (для объёма docs-only и заявленного scope блокеров не выявлено).

---

## 7) Verdict

**Approve with nits** — документ пригоден как **state freeze baseline** для Sprint V2-1; противоречий с Slice A не выявлено; открытые хвосты корректно маршрутизированы в C/D и отражены в тест-гейте.

---

## 8) Release recommendation (спринтовый gate)

**Разрешить переход к Slice C** при условии, что в C фиксируются как минимум: P-MAT-06 / N-MAT-5 (округление), расширенная policy-validation matrix (включая уточнение P-REO-04 при необходимости), и явная формализация предикатов отказов по anchor/state из finding M1. Slice D оставить для стабильных кодов и полей trace при отклонениях claim/free.

Промышленный **binary/network release** из одного docs-only слайса не заявлялся — рекомендация относится к **процессу RFC** и следующему слайсу.

---

## 9) Participation / token estimate (`pwm-review`)

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-v2-1-slice-b-review.md
  - tasks/20260505-v2-s1-slice-b-state-maturity-freeclaim.json
token_usage:
  source: estimate
  input: null
  output: null
  total: 4300
  confidence: low
```

_Оценка по объёму прочитанных freeze/test/ticket и объёму отчёта; провайдер токенов недоступен._

---

## 10) Git handoff для оркестратора

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/sprint-v2-1-slice-b-review.md'
git add 'tasks/20260505-v2-s1-slice-b-state-maturity-freeclaim.json'
git commit -m 'docs(v2-1): Slice B review gate and ticket traceability'
```

---

_End of review report._
