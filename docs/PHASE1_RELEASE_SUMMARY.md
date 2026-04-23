# Phase 1 Release Summary

Короткая сводка по закрытию Phase 1 (MVP-concept) для release notes/README.

## Что вошло в релиз

- **Адреса и policy:** bech32DX как основной UX-формат, witness-ограничения, domain-range policy.
- **CLI и wallet-first flow:** `--wallet` как основной путь подписи, strict pretty + canonical формы адресов, send/burn policy-валидации.
- **TUI send-flow:** рабочая F6-форма (валидации, submit в `POST /v1/tx`, статус/ошибки), wallet controls `F3 Unlock/Lock`, `F4 Encrypt/Re-key`.
- **Производительность/UX TUI:** приоритетный footer со статусом RPC, сокращение длинных полей (`tip`), history modal (`H`) со статусами `pending/ok/error`.
- **Security/docs:** `docs/WALLET_SECURITY_MODES.md` (`plaintext_dev` vs `encrypted`) и `docs/WALLET_BACKUP_RECOVERY_PLAYBOOK.md`.
- **Backup/recovery MVP:** `pwm wallet backup` и `pwm wallet recover` с проверкой encrypted/plaintext payload и предсказуемыми ошибками.
- **URI support (минимум):** `pwm:<address>?amount=` в CLI send-flow (с проверкой конфликтов amount и allow-list query params).

## Ключевые коммиты финального добивания

- `49cb281` — CLI: поддержка `pwm:` URI для `tx-send`.
- `9995f55` — docs: security page для wallet режимов.
- `7b3e0fd` — TUI: operations history modal.
- `2867a67` — TUI: фикс race в history (pending-after-close).
- `3bd77c0` — CLI/core/docs: wallet backup/recovery MVP.
- `e80ee7b` — docs/tasks: финальная синхронизация чеклиста и тикетов.

## Проверка качества

- Автотесты целевых крейтов зелёные:
  - `pwm-core`
  - `pwm-cli`
  - `pwm-tui`
- Критичные сценарии также подтверждены вручную:
  - TUI `F6` send-flow (happy + негативные кейсы),
  - TUI `F3`/`F4` wallet security flow,
  - визуальный контроль footer/`tip`-truncation.

## Статус фазы

- `docs/PHASE1_CHECKLIST.md`: все пункты текущего среза Phase 1 отмечены как выполненные.
