## S15-S3.3 Review: inter-shard connectivity и observability

## Verdict
`request changes`

## Что подтверждено
- Текущий межшардовый путь в runtime — **manual_handoff_required**: после `exported` нужны ручные `finalize -> register provenance -> import`.
- Readiness/preflight и fail-closed контракты работают корректно.
- Если эти шаги не доводить, поток закономерно застревает на `exported`, а затем может уйти в `expired`.

## Почему target может "молчать"
1. Не выполнены `finalize/register/import` после экспорта.
2. Intent истекает до завершения ручного цикла.
3. Ошибки/дрейф стартовых параметров нод (seed/flags/identity/genesis) маскируются тем, что ноды продолжают локально seal'ить блоки.

## Приоритетные рекомендации
1. **P0:** добавить в `/v1/status` агрегаты "stuck intents":
   - `exported_without_finalize`
   - `relayed_without_register`
   - `registered_without_import`
   - `oldest_age_blocks`
2. **P0:** добавить явный `absence alert` в логах/статусе (что именно не произошло в SLA-окне).
3. **P1:** расширить диагностическую сводку межнодовой связности (`live_peer_count`, `last_hello_ok_at`, top reject reason).
4. **P1:** добавить startup lint/check на битые PowerShell-команды, дубли флагов и невалидные peer-seed.
5. **P2:** e2e пакет "target silent" с автоматической классификацией причины.

## Роли сущностей (без смешения)
- `genesis-custom.json` — bootstrap состояния/валидаторов для `pwmd`.
- `wallet.yaml` — клиентские ключи/адреса для `pwm-cli`/`pwm-tui`.
- Нода не запускается "из кошелька"; кошелек не заменяет genesis/runtime identity.
