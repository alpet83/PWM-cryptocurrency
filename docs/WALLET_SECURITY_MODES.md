# Wallet security modes: `plaintext_dev` vs `encrypted`

Короткая operational-страница для Phase 1: когда допустим `plaintext_dev`, какие риски, и как безопасно работать в режиме `encrypted`.

Связанные документы:
- [PHASE1_CHECKLIST.md](PHASE1_CHECKLIST.md)
- [tester-guide-cli-tui-scenarios.md](tester-guide-cli-tui-scenarios.md)
- [TUI_SPEC_v0.md](TUI_SPEC_v0.md)
- [pwm-cli.md](pwm-cli.md)

---

## 1) Когда допустим `plaintext_dev`

`plaintext_dev` допустим только для локальной разработки и коротких тестов, когда потеря seed не критична.

Разрешённые случаи:
- локальный devnet на вашей машине;
- временный тестовый wallet без реальных средств;
- отладка CLI/TUI флоу (`wallet init --plaintext-dev`, `wallet import-seed --plaintext-dev`, F4 Encrypt в TUI).

Запрещено использовать `plaintext_dev`:
- на shared/корпоративных машинах;
- в CI-раннерах, удалённых VM, постоянных окружениях;
- для wallet с реальной ценностью.

> `plaintext_dev` = секреты в явном виде на диске. Это осознанно небезопасный режим.

---

## 2) Основные риски `plaintext_dev`

- Любой доступ к файлу кошелька даёт доступ к signing key/seed.
- Копии файла могут утечь через backup-агенты, sync-клиенты, снимки диска, дампы.
- Passphrase-защита отсутствует, поэтому компрометация файла равна компрометации кошелька.
- Очистка файла после теста может быть неполной (история файловой системы, кэш, теневые копии).

Практическое правило: если не готовы к полной потере ключа, не используйте `plaintext_dev`.

---

## 3) Как работать в `encrypted` (production-like)

`encrypted` — режим по умолчанию для wallet-файла.

Рекомендуемый flow CLI:
1. Создание encrypted wallet:
   - `pwm wallet init --country <LABEL> --wallet-out <PATH>` + passphrase
   - или `pwm wallet import-seed --country <LABEL> --master <HEX> --wallet-out <PATH>` + passphrase
2. Операции подписи через wallet-first путь:
   - `pwm tx-send --wallet <PATH> ...`
   - `pwm tx-init --wallet <PATH> ...`
3. Не использовать `--master` в регулярной эксплуатации (это dev-override).

Рекомендуемый flow TUI:
- Запуск с `--wallet <PATH>`; для encrypted wallet можно:
  - передать passphrase на старте, или
  - разблокировать через F3 (Unlock) в рантайме.
- F3 повторно в unlocked-состоянии делает немедленный Lock.
- F4:
  - `plaintext_dev` -> `encrypted` (Encrypt),
  - `encrypted` -> смена passphrase (re-key) при наличии расшифрованного кэша в сессии.

---

## 4) Базовая operational hygiene

Seed / passphrase:
- Никогда не храните seed и passphrase в одном месте.
- Не вставляйте passphrase в командную историю/скрипты/логи.
- Для production-like используйте длинную уникальную passphrase.

Storage:
- Храните wallet-файл в приватной директории с минимальными правами доступа.
- Исключите wallet-файлы из облачной синхронизации и общих папок.
- Не пересылайте wallet-файлы через мессенджеры/почту.

Backups:
- Держите минимум две резервные копии в разных офлайн-локациях.
- Периодически проверяйте, что backup реально читается (test restore на отдельной машине/папке).
- После F4 re-key обновите процедуру восстановления и убедитесь, что старая passphrase больше не используется.

Инциденты:
- Если есть подозрение на утечку seed/signing key, считайте wallet скомпрометированным:
  - создайте новый wallet,
  - переведите средства на новый адрес,
  - прекратите использование старого файла.
