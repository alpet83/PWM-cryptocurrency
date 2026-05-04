## Sprint 15 S2 Review

## Verdict
`approve with nits`

## Remediation Result
1. `balance_pwm` для foreign переведён в safe clamp `"0"`; ambiguity закрыта.
2. Добавлен контрактный тест `/v1/accounts` для local/foreign split-семантики.
3. Migration/compat policy зафиксирован в `docs/pwmd.md` (раздел API балансовой семантики).

## Nits
- Добавить короткую ссылку на split-семантику в клиентские гайды (`docs/pwm-cli.md` / `docs/pwm-tui.md`) в следующем слайсе.
