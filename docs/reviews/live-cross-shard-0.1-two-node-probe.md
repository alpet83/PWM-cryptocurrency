# Live cross-shard 0.1 probe (2 nodes)

## Scope
Проверка live-нод `3030/3031`, прогон 0.1 coin сценария и фиксация этапов:
`preflight -> export -> finalize+handoff -> register -> import`.

## What was observed
- `3030` и `3031` стабильно seal'ят блоки.
- На `3030` зафиксирован `EXPORT` (routing guard + commit delta).
- После этого intent истек (`expired roaming intents count=1`).
- На `3031` нет признаков приема/import/handoff-register (только sealing).

## Stage checklist
- Preflight: **NOT CONFIRMED** (в live-логах следов нет).
- Export: **PASS** (подтвержден в source логе).
- Finalize + handoff: **NOT CONFIRMED**.
- Register (target provenance): **NOT CONFIRMED**.
- Import: **NOT CONFIRMED**.

## Why target could be silent
1. Не был выполнен полный ручной relay-path после export (finalize/handoff/register/import).
2. Истек roaming intent до завершения цикла.
3. Runtime mismatch с заданными параметрами пробы (`genesis-custom.json + 12345` вместо `tmp/genesis.yaml + 1234`), поэтому это не строгий повтор требуемого стенда.

## Verdict
**PARTIAL**

## Remediation checklist
- [ ] Перезапустить обе ноды строго с `tmp/genesis.yaml` и password `1234`.
- [ ] На source выполнить `export-readiness` для точного EXPORT payload.
- [ ] Сразу отправить EXPORT (0.1 coin) тем же payload.
- [ ] Выполнить finalize/export handoff на source.
- [ ] На target выполнить register provenance (`/v1/export-provenance` / `tx-handoff-register`).
- [ ] На target выполнить `tx-import`.
- [ ] Проверить целевые маркеры в логах обеих нод и сохранить таймштампы этапов.
- [ ] Если снова target silent — сразу проверять отсутствие шагов handoff/register/import до диагностики сети.
