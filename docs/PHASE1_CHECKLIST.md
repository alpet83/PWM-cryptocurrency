# Phase 1 Checklist (MVP-concept)

Документ для рабочего трекинга Phase 1.  
Формат: отмечаем `[x]` по мере выполнения и кратко заполняем блоки `Факт`.

---

## 0) Рамки фазы

- [x] Базовая модель сети остаётся плоской/одноранговой (без роуминга и P2P-расширений).
- [x] Адресный формат: `bech32DX` (Domain eXtension), с узнаваемым префиксом `pwm1`.
- [x] Wallet режим: dual-mode (encrypted default + plaintext dev).
- [x] API-стратегия: compat-first (в `pwmd` пока hex в path/JSON, CLI/TUI конвертируют локально).
- [x] Доменная стратегия: performance-first 16-бит (split high/low byte) для primary user scenario.

**Факт:**
- Дата старта:
- Ответственный:
- Комментарий:

---

## Sprint 1A — Спецификация адресов и witness-модель

### 1. Адресный формат bech32DX

- [x] Зафиксировать структуру адреса: `version + domain + flags + subaccount-tail + checksum`.
- [x] Зафиксировать правила валидации (в т.ч. регистр/чексумма/ошибки ввода).
- [x] Уточнить, какие элементы наследуются от bech32, а какие являются расширением bech32DX.

### 2. Доменное поле и диапазоны RAW-значений

- [x] Описать диапазон для стран.
- [x] Описать диапазон для sector-класса.
- [x] Описать резервный диапазон.
- [x] Описать расширенный диапазон witness-адресов.
- [x] Добавить критерий выбора 16/20 бит по UX-метрике (время brute-force).

### 3. Witness addresses

- [x] Добавить в spec/whitepaper новую сущность witness-адресов.
- [x] Зафиксировать ограничения: без хранения/отправки средств.
- [x] Зафиксировать допустимость подписей witness только для мультисиг-сценариев.
- [x] Описать lifecycle: активные адреса из мощной среды, witness из слабой/аппаратной среды.

**Факт Sprint 1A:**
- Спецификации/доки: `docs/ADDRESS_SPEC_PHASE1_bech32dx.md` (готово для Sprint 1A).
- Принята 16-битная split-модель: `domain_hi/domain_raw` policy classes = 195 country (regulatory, indexed) + 11 sector (indexed) + reserve range (`0xE000..=0xEFFF`) + witness range (`0xF000..=0xFFFF`); `domain_lo` как future-region для country и selector для sector.
- Открытые вопросы: нет блокирующих для Sprint 1A; детальная policy-валидация `flags`/witness переносится в Sprint 1B.

---

## Sprint 1B — Реализация core + CLI (адреса и wallet)

### 1. `pwm-core` (codec + parser)

- [x] Реализовать `account_id_to_bech32dx()`.
- [x] Обновить `parse_account_id()` (bech32DX как primary + legacy fallback на переходный период).
- [x] Добавить валидации domain/flags/witness-ограничений.
- [x] Добавить unit-тесты codec и edge-кейсов.

### 2. `pwm-cli` (адресный UX)

- [x] Сделать bech32DX основным форматом отображения в `addr-derive`.
- [x] Сделать bech32DX основным форматом ввода в `tx-send` / `tx-burn-mark`.
- [x] Зафиксировать strict pretty как primary UX-формат: `pwm1-<label_or_$hex!>-f<flags8hex>-t<tail52hex>` (без embedded canonical, полный tail).
- [x] Сохранить canonical bech32 как отдельную поддерживаемую форму ввода/вывода.
- [x] В user-flow отклонять получателей tx с unknown/reserve/witness доменами.
- [x] Обновить help/error тексты под новую терминологию active/witness адресов.

### 2b. `pwm-cli` (bruteforce + benchmark hook)

- [x] Реализовать однопоточный brute-force адреса по `domain + flags mask` (линейный скан derivation path).
- [x] Добавить CLI-команду/режим для фонового benchmark запуска brute-force (минимальный отчёт: попытки/сек, время до первого совпадения).
- [x] Сохранять найденный derivation path и связанный ключевой материал в wallet (для дальнейшего использования без повторного скана).
- [x] Явно задокументировать ограничение Phase 1: single-thread only.
- [x] Перевести `addr-bruteforce` на label-only ввод домена (country labels для user-profile), numeric raw значения отклонять.
- [x] Перейти на `--expected-flags` как primary (alias `--expected-result` сохранён).
- [x] Зафиксировать policy user-profile: low-10 flags + high-byte country matching.

### 3. Wallet v1 (yaml + dual-mode)

- [x] Добавить wallet schema (YAML, base64/hex поля).
- [x] Реализовать encrypted режим по умолчанию.
- [x] Реализовать plaintext режим для dev с явной маркировкой `INSECURE_DEV_ONLY`.
- [x] Добавить минимум команд: `wallet import-seed`, `wallet show`.
- [x] Добавить `wallet init --country <LABEL> --wallet-out <PATH>` с автопоиском первого совпадения и сохранением country/derivation path.
- [x] Переключить `tx-*` на `--wallet` как основной путь (`--master` как dev-override).
- [x] Финальное ревью кода и тестового кошелька на соответствие спецификации.

**Факт Sprint 1B:**
- Коммиты: in progress (ещё не выделен отдельный commit-срез Sprint 1B).
- Что стабильно работает: кодек bech32DX в `pwm-core` (юнит-тесты parse/pretty/bech32dx/domain в `crates/pwm-core/src/types.rs`); CLI `addr-bruteforce` (single-thread linear scan) с benchmark-метриками; сохранение результата в wallet YAML (`plaintext_dev`), тесты `pwm-cli`/`pwm-core` зелёные.
- Что стабильно работает (обновлено): strict pretty как primary UX (без embedded canonical, полный tail); canonical bech32 как отдельная форма; policy reject для unknown/reserve/witness recipient в `tx-send`/`tx-burn-mark`; `wallet init --country` с auto-bruteforce и сохранением derivation path.
- Что оставлено в fallback: отдельная донастройка UX в override-пути `--master + --domain` (label-домены в части команд пока требуют numeric fallback).
- Принятое исключение ревью: backward-compat для legacy поля `expected_result_u32` не реализуется (по решению продукта, низкая актуальность старой схемы).
- Pending policy validation: baseline для regular non-custodial/non-state/non-commercial wallet — использовать только 10 младших бит `flags`; целевое practical brute-force пространство ~18 бит (достаточно для linear scan, далее scale через multithread).

---

## Sprint 1C — TUI send-flow + валидация

### 1. Send-форма в TUI

- [x] Заменить F6-заглушку на рабочую форму отправки.
- [x] Поля формы: from/to/amount/fee/confirm.
- [x] Локальные валидации формы перед submit.
- [x] Submit в `POST /v1/tx` с отображением статуса/ошибки.
- [x] Отображать адреса в bech32DX как основной пользовательский формат.

### 2. Тесты и smoke

- [x] `cargo test --workspace` зелёный после внедрения.
- [x] Автотесты по адресам/wallet/witness ограничениям.
- [x] CLI smoke (wallet + send).
- [x] Ручной TUI smoke (happy-path + 2–3 негативных кейса) — **F6 send-flow**; см. `tasks/20260421-sprint1c-tui-send-and-validation.json`, гайды `docs/tester-guide-cli-tui-scenarios.md`.

### 3. Документация и релизная фиксация

- [x] Обновить `WHITE_SPEC` (bech32DX + witness + domain ranges).
- [x] Обновить `TUI_SPEC` и user-facing summary фазы 1.
- [x] Добавить/обновить блок в `MVP-checklist` для статуса Phase 1.

**Факт Sprint 1C:**
- Результат тестов: `cargo test --workspace` зелёный; CLI smoke (`wallet + send`) подтверждён, включая отправку на pretty recipient; автотесты по адресам/wallet/witness — закрыты в чеклисте §2.
- Реализация TUI: F6 send-форма внедрена (поля from/to/amount/fee/confirm, локальные валидации, submit в `POST /v1/tx`, показ статуса/ошибки); отображение счетов и полей формы через `account_id_to_human` (strict pretty как primary UX Phase 1, согласовано с `WHITE_SPEC` / `ADDRESS_SPEC`).
- Ручной F6 smoke подтверждён визуально оператором: happy-path + негативные сценарии send-flow пройдены, включая актуальные проверки unlock/encrypt и UX-детали status/footer.

---

## Доп. расширения (по приоритету после core Scope)

- [x] URI-формат для адресов (`pwm:<address>?amount=`).
- [x] Адресная книга (labels/contacts) в wallet — **минимум Phase 1**: секция `address_book` в YAML, CLI `wallet book-*`, TUI панель + модалка append после send.
- [x] История операций в TUI.
- [x] Backup/recovery сценарии wallet.
- [x] Короткая security-страница: dev-only vs production-like.

**Факт по расширениям:**
- Какие вошли в Phase 1: адресная книга (минимальная схема YAML + CLI/TUI), URI (`pwm:<address>?amount=`) в CLI `tx-send`/`tx-burn-mark` (plain + URI, query allow-list `amount`, явная ошибка при конфликте с `--amount`, запрет `amount` для `--beneficiary`).
- Какие перенесены: нет блокирующих переносов по разделу доп. расширений Phase 1.
- История операций в TUI добавлена в MVP: модалка `H` (empty/pending/ok/error), локальный timeline из send-flow, фикс статуса после закрытия формы до RPC-ответа.
- Security-страница добавлена: `docs/WALLET_SECURITY_MODES.md` (`plaintext_dev` vs `encrypted`, operational hygiene).
- Backup/recovery закрыты в минимальном MVP: CLI `wallet backup` / `wallet recover` с валидацией encrypted payload и предсказуемыми ошибками для wrong passphrase/corrupted payload; recovery playbook: `docs/WALLET_BACKUP_RECOVERY_PLAYBOOK.md`.

