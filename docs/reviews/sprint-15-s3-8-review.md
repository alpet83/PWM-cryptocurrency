# S15-S3.8 Review: cross-shard import / DO balance discrepancy

## Verdict
`approve with nits`

## Ключевой вывод
По текущим признакам это не протокольный отказ `import`, а, с высокой вероятностью, проблема наблюдаемости/интерпретации:
- лог `tx commit delta kind=import bal:0->0` отражает баланс **подписанта import**, а не получателя `to`;
- в API поле `balance_pwm` для foreign-адресов может быть принудительно `0` (legacy), тогда смотреть нужно `local_state_balance` и `local_view_only`.

## Что проверять для подтверждения apply/reject
1. `POST /v1/tx` (import) -> `204` или явный `400` с причиной.
2. `GET /v1/flow/recent` -> есть ли `applied/imported` для export/import id.
3. `GET /v1/status` -> растёт ли `bridge_imported_set_size`.
4. На target RPC проверять именно адрес `to` и поле `local_state_balance`, а не только `balance_pwm`.

## Вероятные причины "баланс DO = 0"
1. Проверяется не тот адрес/не тот RPC.
2. Смотрится `balance_pwm` (legacy view), а не `local_state_balance`.
3. Лог import интерпретируется как delta получателя, хотя это delta отправителя import-tx.

## Remediation checklist
- После `tx-import` фиксировать `to` и проверять его `local_state_balance` на target.
- В runbook добавить обязательный шаг `flow/recent + bridge_imported_set_size`.
- В UI/операторских скриптах явно различать `balance_pwm` vs `local_state_balance`.
