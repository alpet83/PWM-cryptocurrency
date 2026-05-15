# Review: Sprint V2-3 Slice 0 — schema prep и design freeze

**Дата:** 2026-05-06  
**Тикет:** `tasks/20260506-v2-sprint3-emission-whales.json`  
**Скоуп коммитов:** `d34e64a` (feat: freeze emission schema), `dab0f5a` (fix: pwm-cli genesis-build test vs schema v5)  
**Статус pwm-testing:** retest PASS (после hotfix по `GENESIS_SCHEMA_VERSION`)

## 1. Scope recap

Slice 0 по `docs/reviews/sprint-v2-3-slice0-design-freeze.md` и `docs/plans/mvp_v2.md` (Sprint V2-3): закрепить поля `GenCfg` под будущую эмиссию, зафикстить детерминизм входов для Slice 1+, поднять целевой genesis JSON schema до версии **5** с приёмкой **4/5** в `pwmd`, оставить **без изменения** текущую бизнес-логику награды в runtime.

Проверенные следы изменений: дизайн-файл фриза, `pwm-core` genesis, загрузчик genesis в `pwmd`, `pwm-cli` genesis-build и связанный тест, косвенно — отсутствие ветвлений по новым полям в `Chain::seal`.

## 2. Requirements fit

Соответствие заявленным целям **хорошее**.

- **`GenCfg` в pwm-core:** добавлены `policy_ver`, `pwm_stake_min`, `marks_stake_min`, `season_enabled`, `season_coeff_ppm` с serde-default и локальными default-функциями; legacy-политика зафиксирована константой `LEGACY_POLICY_VER == 1`; значения по умолчанию совпадают с таблицей в design-freeze (порог PWM 100_000, marks min 1, сезонность выкл., ppm 1_000_000). `dev_net()` заполняет новые поля явно — повторяемость фикстур сохранена.
- **Backward rule 4/5 в pwmd:** парсер принимает `schema_version` 4 и 5, при отсутствии ключей в JSON срабатывают `serde(default)` и default-строки для u128-полей — эквивалент «v4 без новых полей → legacy-safe defaults». Для v5 поля сериализуются/читаются как часть `gen_cfg`.
- **pwm-cli:** `GENESIS_SCHEMA_VERSION = 5`, билд записывает новые поля с legacy-safe значениями (`LEGACY_POLICY_VER`, константы из core). После `dab0f5a` тест `gen_build_schema5_bundle` согласован с константой schema — замыкает регресс, выявленный pwm-testing.

**Зазоры:** в тикете pwm-testing корректно отмечалось отсутствие отдельной фикстуры JSON schema v5 в `pwmd`-тестах; для Slice 0 это согласовано с non-goals (тесты формулы — Slice 1/2). Риск остаётся низким благодаря serde-default пути для v4.

## 3. Style and module shape

- Короткие идентификаторы в затронутом диффе соблюдены; прогон `python scripts/check_rust_fn_name_segments.py` по указанным путям — **нарушений нет**.
- Модульные баннеры `//!` в затронутых файлах на месте.
- **Нит:** имена типов и функций в `pwmd` по-прежнему несут суффикс `V4` / `parse_genesis_v4`, хотя контракт уже 4 **и** 5 — только читаемость, не функциональный дефект; при следующем рефакторинге имеет смысл переименовать без изменения сериализуемой формы JSON.

English в комментариях заголовков — ок.

## 4. Safety

- Новые поля только расширяют конфиг; загрузчик genesis сохраняет существующие проверки (валидаторы, KDF/iters cap, AEAD, соответствие ключей строкам множества).
- Не добавлены новые `unwrap` на горячих путях консенсуса в этом слайсе.

**Примечание контекста (не регресс слайса):** `Chain::seal` по-прежнему берёт timestamp из `SystemTime` для локального уплотнения блоков — это отдельно от заявленных будущих детерминированных входов расчёта эмиссии по `header.ts`; в Slice 0 новые поля в расчёт награды не встраиваются.

## 5. Tests

- pwm-testing зафиксировал PASS после правки тестов: `cargo test -p pwm-cli gen_build`, полный `-p pwm-cli`, `pwmd genes_`.
- Семантика «schema v5 + legacy defaults в bundle» покрыта тестом `gen_build_schema5_bundle`.

Отсутствие отдельного интеграционного парса v5-only JSON в pwmd можно принять как осознанный долг Slice 1 при появлении фикстур.

## 6. Verdict

**Approve with nits**

Приоритеты:

1. (Низкий) Рассмотреть переименование «v4-only» символов в `pwmd` genesis loader, когда удобно, чтобы имя отражало 4/5.
2. (Низкий, опционально для Slice 1) Мини-фикстура или unit на парс минимального v5 genesis в pwmd, если понадобится жёсткая регрессия формата.

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS
artifacts: docs/reviews/sprint-v2-3-slice0-review-20260506.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 12000, "confidence": "low" }
```

---

**Краткий вердикт для оркестратора:** **PASS (approve with nits)** — Slice 0 соответствует design freeze; runtime reward path остаётся legacy; тестовый зазор pwm-cli закрыт в `dab0f5a`.
