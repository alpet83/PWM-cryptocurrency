# Sprint 14 Validator Seed Security Review (2026-04-28)

## Verdict
`request changes`

## Короткий ответ на ваш вопрос
- Да, в текущем потоке `genesis-build` фактически копирует `MASTER_SEED` в `validator_seeds_hex`.
- Это **не** хорошая практика для production/защищённого контура.
- Для dev/test это может быть терпимо как упрощение, но для реальной эксплуатации — высокий риск утечки секрета.

## Основные риски
1. `validator_seeds_hex` в genesis-файле содержит сырой seed; при утечке файла утекает и секрет валидатора.
2. Сейчас seed валидатора связан с wallet master seed, т.е. нет изоляции ролей.
3. Genesis-файл обычно широко распространяется, поэтому это плохой контейнер для приватного материала.

## Рекомендации

### Срочно (операционно)
- Не публиковать/не коммитить genesis с реальными `validator_seeds_hex`.
- Использовать отдельный seed для валидатора (не wallet master seed).

### Следующий спринт (архитектурно)
- Перейти к схеме: **public genesis без секретов** + локальная подача validator secret (env/keystore/HSM).
- Оставить в genesis только публичные данные валидатора (`pubkey`/`acct` и параметры строк).
- Ввести domain-separated derivation для validator role, если оставляете HD-подход.

## Подтверждение в коде
- Источник риска: `pwm-cli genesis-build` формирует `validator_seeds_hex` из wallet seed.
- Потребление: `pwmd` декодирует эти seed и строит signing keys при старте.
