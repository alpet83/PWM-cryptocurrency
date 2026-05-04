# Sprint 14 - Slice 14 review (independent)

## Verdict
`request changes`

## Findings
- **High**: после rename canonical snapshot field (`genesis_rows` -> `genesis_accounts`) старый canonical snapshot не загружается автоматически; нужен либо controlled migration, либо явный runbook как intentional hard-break.
- **Low**: в `docs/pwm-cli.md` местами осталась старая формулировка “rows” рядом с новым `accounts`.

## Confirmed good
- Runtime/CLI/docs в основном переведены на `accounts`.
- F7-hint и обработчик F7 в TUI удалены.
