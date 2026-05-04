# S15-S3.16 — testing notes

## Наблюдаемый симптом

Кросс-шард в TUI: все шаги зелёные, на source списание видно (например 0.01), ожидание роста баланса на target не подтверждается визуально (остаётся «1»).

## Анализ цепочки

1. **`POST /v1/tx` с `Import` на source** при `is_foreign_import`: выполняется только `relay_import` → HTTP на target `/v1/tx`; локальный `seal` на source **не** вызывается.
2. **`mark_imported_by_export_id`** раньше вызывался только в ветке локального seal после Import — для relay-пути на source **не** вызывался, хотя импорт на target уже прошёл.
3. Следствие: снимок и опрос **`GET /v1/roaming-intents`** на source могли не отражать терминальное **imported** для UX/диагностики; возможна рассинхронизация ожиданий оператора относительно фактического состояния цепочки на target.

## Рекомендации проверки вручную (ноды запущены)

- После перевода: **`GET {target_rpc}/v1/accounts`** или **`GET …/v1/account/{recipient}`** — смотреть **`balance_pwm`** / **`local_state_balance`** для **target-домена** (не legacy поле для foreign на обратной стороне).
- Лог target `pwmd`: строки `tx commit delta` / `imported` после relay.

## Автотесты

`cargo test -p pwmd --lib` — регрессия пройдена после правки.

---

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-16-testing.md
```
