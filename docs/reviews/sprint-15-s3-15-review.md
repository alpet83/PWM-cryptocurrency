# Sprint 15 — S3.15: Cross-shard step 3 relay / handoff — Review

## Scope

Отладка цепочки `relay_handoff` и UX pwm-tui шага 3 при `exported` + `last_error`; структурированные логи `RelayTrace` и `http_body_log_snippet`.

## Requirements fit

Цели тикета достигнуты: причина «шаг 3 не relayed» сводится к поведению `mark_relay_error` без смены статуса; TUI больше не теряет `last_error` и показывает FAIL с текстом relay при `exported` + ошибке.

## Safety

**Логи:** snippet JSON без ключей, усечение — приемлемый баланс.

**Сообщения в `last_error` / TUI:** после доработки оркестратором полное тело HTTP **не** вкладывается в `RelayErr.message`; используются те же snippet, что и в warn-логах; для `accepted: false` — усечённая причина из ack.

Остаточный риск: произвольные строковые значения в JSON всё ещё могут попасть в snippet до 200 символов — операционно приемлемо для MVP.

## Verdict

**APPROVE with nits** — при желании добавить юнит-тест на формат сообщения ошибки relay и интеграционный тест «exported + last_error».

---

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-15-review.md
token_usage:
  source: estimate
  total: 9500
  confidence: low
```
