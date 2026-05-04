# Logging Style Review — Sprint 14 Slice 18

## Verdict
`approve with nits`

## Style contract for pwmd
- Формат: `[HH:MM:SS.mmm] #TAG: event | k1=v1 k2=v2 ...`
- Теги: `TRACE|DEBUG|INFO|WARN|ERROR`, uppercase, с префиксом `#`.
- Маршрутизация: `WARN/ERROR -> stderr`, остальные -> stdout.

## Requested palette
- Все числовые значения: **bright purple**.
- `#ERROR`: **bright red**.
- `#WARN`: **dark red**.

## Numeric highlight rules
- Подсвечивать числа в message и `k=v` значениях (amount/height/nonce/latency/fees/counts).
- Не подсвечивать timestamp и числа внутри hash/id (hex/base58/base64).

## Non-TTY fallback
- В non-TTY/NO_COLOR отключать ANSI и оставлять plain-текст.
- Файловые логи по умолчанию no-color безопасны для парсинга.

## Implementation checklist for coding
- Добавить единый palette mapping для severity + numeric highlighter.
- Применять color layer только поверх готовой plain строки.
- Закрепить TTY/NO_COLOR policy.
- Добавить тесты на palette, numeric highlighting, non-TTY behavior.
