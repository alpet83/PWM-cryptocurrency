# Sprint 15 — S3.17: closeout отладки межшардового roaming (completion)

**Дата:** 2026-05-01  
**Предпосылки:** линия **S15-S3.16** (cycle1 + cycle2): баланс/credit, relay observability, TUI step 5, расследование DO snapshot и журналов релея.

## 1) Что зафиксировано как рабочее по дизайну

- Полный путь **EXPORT → handoff → relay → Import**: зачисление на target воспроизводимо на двух нодах; **визуально подтверждены** lifecycle в TUI и **шаг 5** (сверка кредита с учётом того, что fee не входит в сумму зачисления).
- Оператор понимает разницу между **статусом intent** (`relayed`) и **фактическим Import tx**, который должен быть отправлен клиентом и отработан на target.

## 2) Артефакты отладки (архив)

| Документ | Назначение |
|----------|------------|
| [ROAMING_COMPLETION.md](../ROAMING_COMPLETION.md) | Консолидация симптомов, причин и чек-листа приёмки |
| [sprint-15-s3-16-cycle2-testing.md](sprint-15-s3-16-cycle2-testing.md) | Прогоны Rounds 1–4, автотесты, live сводки |
| [sprint-15-s3-16-cycle2-relay-journal-review.md](sprint-15-s3-16-cycle2-relay-journal-review.md) | Индекс логов и пробелы наблюдаемости |
| [sprint-15-s3-16-do-snapshot-root-cause.md](sprint-15-s3-16-do-snapshot-root-cause.md) | Класс ошибок `state_root` mismatch при загрузке снапшота DO |

## 3) Документы, актуализированные при S3.17

- [ROAMING-SAMPLE.md](../ROAMING-SAMPLE.md) — one-window + автоматический Import, env target RPC, шаг 5 TUI.
- [pwm-tui.md](../pwm-tui.md) — cross-shard send, `PWM_TUI_TARGET_RPC`, шаг подтверждения баланса.
- [pwmd.md](../pwmd.md) — краткая операторская заметка по логам межшарда / диагностике снапшота.
- [rfc/9-crossdomain-roaming.md](../rfc/9-crossdomain-roaming.md) — дополнение по фактическому Sprint 15 UX (Import после relayed).
- [tester-guide-cli-tui-scenarios.md](../tester-guide-cli-tui-scenarios.md) — проверка межшард-приёмки.
- [MVP-checklist.md](../MVP-checklist.md) — ссылка на completion-док.
- [sprint-15-checklist.md](sprint-15-checklist.md) — секция S3.17.

## 4) Тикеты / слайсы

- `tasks/20260430-s15-slice3-16-cycle2-xshard-credit-tui-step5.json` — закрыт приёмкой оператора (completed по согласованию).
- `tasks/20260501-s15-slice3-17-roaming-completion-closeout.json` — метка closeout S3.17.

## 5) Verdict

Слайс **S15-S3.17** (документация + синхронизация операторских материалов после успешной live-приёмки) считается **completed**. Перенос в **S15-S4** (snapshot abstraction) не блокируется данным closeout.

```yaml
participation:
  slice: S15-S3.17
  verdict: completed
  live_acceptance: confirmed_by_operator_2026-05-01
```
