# V2 Slice 5 hotfix — повторная валидация (testing rerun)

**Дата:** 2026-05-08  
**HEAD:** `23a183fafc761822aae8feed72e3673c88de7456` (`23a183f` — test(pwmd): align tx_batch_profile_drop with profile_mismatch reason)

## Команды

| Команда | Результат |
|--------|-----------|
| `cargo check -p pwmd` | PASS |
| `cargo test -p pwmd tx_batch_profile_drop` | PASS (1 passed) |
| `cargo test -p pwmd peer_session::tests` | PASS (15 passed, 0 failed) |

## Вывод

Ожидание `profile_mismatch` в `tx_batch_profile_drop` согласовано с нормализованным reason; регрессия по `peer_session::tests` не воспроизводится на указанном коммите.

## Связанные тикеты

- `tasks/20260508-v2-slice5-hotfix-profile-drop-test.json`
- `tasks/20260508-v2-sprint8-slice5-observability-chaos-docs.json`

## Подтверждение (pwm-testing, повторный прогон)

Повторно на **2026-05-08** (субагент pwm-testing): `git rev-parse HEAD` = `23a183fafc761822aae8feed72e3673c88de7456`. Выполнены те же проверки, что в таблице выше — все **PASS**; счёт тестов: `tx_batch_profile_drop` — 1 passed; `peer_session::tests` — 15 passed, 0 failed.
