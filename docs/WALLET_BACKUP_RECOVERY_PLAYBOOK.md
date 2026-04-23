# Wallet Backup/Recovery Playbook (Phase 1)

Этот playbook закрывает базовый operational flow для backup/recovery wallet в Phase 1.

## 1) Где хранить backup

- Храните основной wallet и backup в разных каталогах/носителях.
- Не кладите backup в общие папки, мессенджеры и cloud sync без шифрования.
- Для encrypted wallet passphrase храните отдельно от файла backup.

## 2) Создать backup (CLI)

Для encrypted wallet:

```bash
pwm wallet backup --wallet .\tmp\wallet-cy.yaml --out .\tmp\wallet-cy.backup.yaml --wallet-passphrase "<PASSPHRASE>"
```

Для `plaintext_dev`:

```bash
pwm wallet backup --wallet .\tmp\wallet-cy-dev.yaml --out .\tmp\wallet-cy-dev.backup.yaml
```

Что проверяется перед копированием:
- wallet-файл читается как валидный YAML;
- wallet identity/metadata консистентны (`account_id_*`, `domain_u16`);
- для encrypted wallet payload реально расшифровывается указанным passphrase.

Если passphrase неверный или payload поврежден, команда завершается предсказуемой ошибкой и backup не создается.

## 3) Восстановить из backup (CLI)

Для encrypted wallet:

```bash
pwm wallet recover --backup .\tmp\wallet-cy.backup.yaml --out .\tmp\wallet-cy.restored.yaml --wallet-passphrase "<PASSPHRASE>"
```

Для `plaintext_dev`:

```bash
pwm wallet recover --backup .\tmp\wallet-cy-dev.backup.yaml --out .\tmp\wallet-cy-dev.restored.yaml
```

Восстановление также валидирует payload до записи `--out`.

## 4) Проверка после восстановления

1. Проверить metadata:

```bash
pwm wallet show --wallet .\tmp\wallet-cy.restored.yaml
```

2. Для encrypted wallet проверить decrypt-path:

```bash
pwm wallet show --wallet .\tmp\wallet-cy.restored.yaml --unsafe-show-secrets --wallet-passphrase "<PASSPHRASE>"
```

3. Проверить, что `account_id_human`, `derivation_path`, `domain_u16` совпадают с ожиданиями.
4. Выполнить smoke-действие с восстановленным wallet (например `tx-init` в тестовой среде).

## 5) Минимальный checklist восстановления

- [ ] Backup-файл хранится отдельно от рабочего wallet.
- [ ] Для encrypted wallet passphrase проверен командой `wallet show --unsafe-show-secrets`.
- [ ] Восстановленный wallet читается без ошибок (`wallet show`).
- [ ] Контрольный smoke на восстановленном wallet выполнен в test/dev окружении.
