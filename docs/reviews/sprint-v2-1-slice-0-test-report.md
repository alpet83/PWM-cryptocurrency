# Sprint V2-1 / Slice 0 — Testing gate (audit/docs)

Дата: 2026-05-05  
Агент: `pwm-testing`  
Артефакты слайса: `docs/reviews/sprint-v2-1-slice-0-checklist.md`, `docs/reviews/sprint-v2-1-slice-0-spec-impl-audit.md`.

## Вердикт: **PARTIAL (gate open)**

Документы достаточны, чтобы следующий кодовый слайс имел понятный scope и критерии приёмки. **PARTIAL** из‑за: (1) практический чеклист слайса не синхронизирован с выполненной работой (все пункты всё ещё `[ ]`); (2) негативные сценарии для будущих тестов описаны преимущественно нарративом, без явной матрицы кейсов.

## Что проверено

| Критерий | Результат |
|----------|-----------|
| Чёткие acceptance-пункты | Да — блок «Acceptance checklist» в audit-отчёте + предложения по узким next slices |
| Ключевые риски v2 (`marks` vs `marks_quota`, эмиссия, API) | Покрыты findings с severity и impacted files |
| Согласованность с `pwm-core` / `pwmd` | Выборочно сверено с текущими исходниками; противоречий не найдено |
| RFC-first пороги (~100k PWM, ~1 marks stake) | Сформулированы как требования к конфигу/политике, без преждевременной привязки к числам в коде |
| Прогон автотестов | Не выполнялся (docs-only slice; ограничение владельца) |

### Сверка с кодом (high level)

- `state.rs`: `TxBody::BurnMark` использует `marks_quota_of`, уменьшает `marks_quota`, поле `Account.marks` в этой ветке не изменяется — совпадает с finding High #1.
- `state.rs`: `accrue_marks` — формула `staked * coeff / 1_000_000`, затем `normalize_marks_quota` — совпадает с Medium #5.
- `chain.rs`: `seal` → `accrue_marks(cfg.marks_coeff)` затем `reward_producer(..., cfg.block_reward)` без порога стейка — совпадает с Medium #4–5.
- `genesis.rs`: `dev_net()` задаёт `block_reward: 100`, `marks_coeff: 10_000` — согласуется с audit.
- `pwmd` `AcctOut` / `acct_out_for_runtime`: сериализуется `marks: ac.marks.to_string()`, квота наружу не экспонируется — совпадает с Medium #2–3.

## Пробелы (тестопригодность / процесс)

1. **`docs/reviews/sprint-v2-1-slice-0-checklist.md`**: scope/evidence/output чеклисты не отмечены выполненными при готовом audit — риск рассинхрона для ревью.
2. **Явная матрица негативных тестов** для следующего кода отсутствует (желательно добавить в Slice 1 spec или отдельный test-plan doc).
3. **Кросс-доменные / edge-кейсы** для burn (после миграции на `marks`): квота ниже `marks` после `normalize`, гонки с начислением marks — упомянуты косвенно, без фиксированных expected error/инвариантов.

## Минимальный test-plan на следующий кодовый слайс

Предполагается слайс, который вводит политику эмиссии и/или «burn from marks».

1. **Политика наград (pure function или модуль):** table-driven тесты на границах `pwm_min_stake_for_emission` и `marks_min_stake_for_emission` (ниже / ровно / выше порога; нулевой стейк; saturating arithmetic).
2. **`Chain::seal` / интеграция:** после внедрения policy — один тест «один блок, известный стейк продюсера» с детерминированным ожиданием PWM/marks delta (без flaky time, при необходимости фикстура `ts`/height если войдёт в контракт).
3. **`apply_tx` + `BurnMark`:** успешное списание с `marks`; `InsufficientMarks` при недостаточном `marks`; сохранение инварианта nonce; при сохранении legacy-квоты на переходный период — отдельный `#[cfg]` или feature-gated кейс по согласованию с владельцем.
4. **API (`AcctOut`):** после решения по exposure — serde snapshot или полевой тест «marks == spendable» или явное доп. поле + тест на его наличие.
5. **Регрессия:** обновить существующие тесты в `state.rs`, которые сейчас закрепляют поведение `marks_quota` (finding Low #6 в audit).

## Команды

- Автотесты: не запускались (`cargo test` / preflight n/a по запросу).
- Статический обзор: чтение артефактов + точечные фрагменты `crates/*` без изменений product code.
