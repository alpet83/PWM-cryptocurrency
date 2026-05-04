# Sprint 14 Slice 11 — remediation2 review

## Verdict
`approve with nits`

## Result
- Баг generation-side закрыт: `genesis-build` теперь гарантирует, что каждый `validators.set[*].acct_hex` присутствует в `funding.rows`.
- При отсутствии строки добавляется детерминированная funding row с `bal=0`.
- Инвариантная ошибка старта `pwmd` больше не воспроизводится на новом genesis-файле.

## Residual nit
- Низкий приоритет: добавить отдельный тест на ветку, где validator account уже есть в funding (проверка отсутствия дубликата/перезаписи).
