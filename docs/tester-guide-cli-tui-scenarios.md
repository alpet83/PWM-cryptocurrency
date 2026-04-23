# Tester guide: CLI/TUI сценарии и ожидаемые результаты

Документ для ручной проверки пользовательских сценариев на локальном devnet с актуальной Phase 1B policy.

Перед запуском сценариев по wallet-режимам: [WALLET_SECURITY_MODES.md](WALLET_SECURITY_MODES.md).

## 0) Терминал и долгоживущие процессы

- **Windows / PowerShell:** для цепочек вроде `cargo run ... | ...` и кавычек в параметрах типичны сюрпризы; консоль по умолчанию не всегда дружелюбна к UTF-8. Для ручных сценариев предпочтительнее **Git Bash** (или WSL), либо внешний оркестратор процессов на хосте (например `cq_process_ctl` в Colloquium), чтобы корректно завершать `pwmd`/`pwm-tui` и не зависать на интерактиве.
- **Автотесты / RPC:** у `pwm-tui` есть таймаут HTTP к ноде — `PWM_TUI_RPC_TIMEOUT_MS` (мс, по умолчанию 3000, максимум 120000). Задайте меньшее значение, если сценарий должен быстро упасть при недоступном `PWM_RPC`.
- **Encrypted wallet / unlock:** таймер авто-блокировки signing key после разблокировки — `PWM_TUI_WALLET_UNLOCK_SECS` или `--wallet-unlock-secs` (секунды, по умолчанию 300, макс. 604800). Passphrase из F3/F4 и из переменных окружения не должен попадать в логи приложения.
- **F4 encrypt / re-key:** см. `TUI_SPEC_v0.md` §4 (кэш расшифрованного payload до auto-lock; запись wallet через temp+rename). После смены passphrase обновите `PWM_TUI_WALLET_PASSPHRASE` в окружении для **следующего** запуска процесса.
- **Linux-сборки:** при появлении Linux-окружения (например контейнер `mcp-sandbox` с toolchain) имеет смысл гонять те же `cargo test`/`cargo run` там, чтобы отделить проблемы консоли Windows от логики приложения.

## 1) Подготовка

- Терминал A:

```powershell
cargo run -p pwmd --bin pwmd
```

- Терминал B:

```powershell
$env:PWM_RPC="http://127.0.0.1:3030"
```

- Проверка health:

```powershell
Invoke-RestMethod -Uri "$env:PWM_RPC/v1/head"
```

## 2) Позитивные CLI-сценарии

### CLI-P01: генерация seed

```powershell
cargo run -p pwm-cli --bin pwm -- key-gen
```

Ожидание: seed в hex, без ошибок.

### CLI-P02: `wallet init` с country + auto bruteforce

```powershell
cargo run -p pwm-cli --bin pwm -- wallet init --country CY --wallet-out .\tmp\wallet-cy.yaml
```

Ожидание:
- команда завершается штатно;
- в выводе есть `country_label CY`, `domain_match_mode high_byte_only`, `derivation_path m/0/...`;
- `domain_lo = 00` считается валидным исходом и не блокируется в генерации (как и любой другой `00..FF`);
- wallet-файл создан.

### CLI-P03: `addr-bruteforce` по country label

```powershell
$MASTER="<MASTER_HEX>"
cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master $MASTER --domain CY --flags-mask 0x03FF --expected-flags 0 --wallet-out .\tmp\bf-cy.yaml
```

Ожидание:
- команда принимает label-only домен;
- есть benchmark поля (`benchmark_attempts`, `benchmark_attempts_per_sec`);
- выводит `account_id_human` (pretty) и `account_id_bech32dx` (canonical отдельно).

### CLI-P04: `tx-send` с pretty recipient

1) Получите pretty-адрес получателя (из `account_id_human`).
2) Если в YAML кошелька уже есть **непустой** `address_book`, сначала зарегистрируйте получателя: `pwm wallet book-add --wallet ... --address <PRETTY_RECIPIENT>` (см. CLI-P04b). При **пустой** книге ограничение не действует.
3) Отправьте (основной путь: wallet-first):

```powershell
cargo run -p pwm-cli --bin pwm -- tx-send --wallet .\tmp\wallet-cy.yaml --to <PRETTY_RECIPIENT> --amount 10
```

Ожидание: транзакция уходит без ошибки формата адреса.

### CLI-P04b: адресная книга (`address_book`) и allow-list для `tx-send`

В файле кошелька поле **`address_book`**: активное хранилище canonical-only (`bech32dx`). CLI может принимать canonical, strict pretty (`pwm1-LABEL/XX-f...-t...`) и policy-допустимый legacy hex на вход, но при `book-add` в файл записывается canonical.
Ambiguous legacy pretty без `/LO` (например `pwm1-CY-f...`) в runtime input-paths (`tx-send --to`, `tx-burn-mark --beneficiary`, `wallet book-remove --address`, TUI `F6/to`) отклоняется с явной рекомендацией перейти на strict pretty `LABEL/LO` или canonical bech32dx.
Legacy pretty-записи в уже существующем wallet при загрузке игнорируются (не источник истины) с предупреждением в статусе; загрузка не падает.
Пока список **пуст** — `tx-send --wallet` разрешает любого policy-валидного получателя. Как только в списке есть **хотя бы одна** canonical запись, `to` должен совпадать с одной из них (после парсинга в `AccountId`).

```powershell
cargo run -p pwm-cli --bin pwm -- wallet book-add --wallet .\tmp\wallet-cy.yaml --address <PRETTY_OR_CANONICAL_RECIPIENT>
cargo run -p pwm-cli --bin pwm -- wallet book-list --wallet .\tmp\wallet-cy.yaml
cargo run -p pwm-cli --bin pwm -- wallet book-remove --wallet .\tmp\wallet-cy.yaml --address <SAME_AS_ADD>
```

Ожидание: дубликаты в книгу не добавляются; удаление неизвестного адреса — ошибка; `wallet show` показывает секцию `address_book`.

**Исключение:** `tx-send` с **`--master`** (dev override) **не** проверяет `address_book` — ограничитель привязан к пути «подпись из файла кошелька».

Dev override (только для отладки):

```powershell
cargo run -p pwm-cli --bin pwm -- tx-send --wallet .\tmp\wallet-cy.yaml --master $MASTER --domain CY --to <PRETTY_RECIPIENT> --amount 10
```

### CLI-P05: `tx-send` с canonical recipient

```powershell
cargo run -p pwm-cli --bin pwm -- tx-send --wallet .\tmp\wallet-cy.yaml --to <CANONICAL_BECH32DX_RECIPIENT> --amount 10
```

Ожидание: canonical форма принимается как отдельный валидный формат.

## 3) Негативные CLI-проверки

### CLI-N01: запрет numeric domain в `addr-bruteforce`

```powershell
cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master $MASTER --domain 17241 --flags-mask 0x03FF --expected-flags 0 --wallet-out .\tmp\bad.yaml
```

Ожидание: явная ошибка `numeric domain input is not allowed`.

### CLI-N02: запрет non-country label в `wallet init`

```powershell
cargo run -p pwm-cli --bin pwm -- wallet init --country MSFT --wallet-out .\tmp\wallet-msft.yaml
```

Ожидание: явный reject (corporate/other rejected in this phase).

### CLI-N03: запрет flags вне low-10 policy

```powershell
cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master $MASTER --domain CY --flags-mask 0x0400 --expected-flags 0 --wallet-out .\tmp\bad-flags.yaml
```

Ожидание: ошибка про low 10 bits в user profile.

### CLI-N04: `tx-send` reject unknown/reserve/witness recipient

Проверить 3 варианта получателя:
- pretty с unknown domain (`$....!`);
- адрес из reserve domain;
- адрес из witness domain.

Ожидание: во всех случаях явный отказ с причиной policy (unknown / reserve / witness-only).

### CLI-N07: `tx-init` с regulatory `/00` не блокируется low-byte policy

```powershell
cargo run -p pwm-cli --bin pwm -- tx-init --master $MASTER --domain 0x2c00 --index 0 --flags 0
```

Ожидание: отдельного policy reject по low-byte `00` нет (допустимы и `/00`, и `/01..FF`); возможны только другие стандартные отказы (например `domain mismatch` для неконсистентной подписи/домена).

### CLI-N05: malformed pretty

```powershell
cargo run -p pwm-cli --bin pwm -- tx-send --wallet .\tmp\wallet-cy.yaml --to pwm1-CY-f00000000-t123 --amount 1
```

Ожидание: человекочитаемая ошибка с accepted formats (`pretty/canonical/legacy`).

### CLI-N06: reject ambiguous legacy pretty без `/LO`

```powershell
cargo run -p pwm-cli --bin pwm -- tx-send --wallet .\tmp\wallet-cy.yaml --to pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000 --amount 1
cargo run -p pwm-cli --bin pwm -- wallet book-remove --wallet .\tmp\wallet-cy.yaml --address pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000
```

Ожидание: явный runtime reject с текстом про `missing '/LO'` и подсказкой использовать strict pretty `pwm1-LABEL/XX-f...-t...` или canonical bech32dx.

## 4) Проверка strict pretty policy

Для адреса из `addr-derive` / `wallet init` проверьте:
- формат строго `pwm1-...-f...-t...`;
- нет embedded canonical фрагмента (`|pwm1...`);
- tail полный (`t` + 52 hex).

## 5) TUI smoke

```powershell
$env:PWM_RPC="http://127.0.0.1:3030"
cargo run -p pwm-tui --bin pwm-tui -- --wallet .\tmp\wallet-cy.yaml --wallet-passphrase "<PASSPHRASE>"
```

Ожидание:
- TUI запускается без падения;
- owner-панель берёт адрес из truth-source wallet (`master_seed_hex + derivation_path` или `signing_key_hex + derivation_index`), а `account_id_*` нормализуются как кеш;
- legacy mismatch между `account_id_human` и `account_id_hex` не блокирует загрузку, если truth-source валиден;
- при успешной миграции owner/from используют strict pretty (`pwm1-CY/LO-...`), и файл auto-upgrade меняется только при реальном отличии;
- если в кошельке задан **`address_book`**, правая панель — **только** canonical записи active-book (pretty legacy entries игнорируются) с подстановкой балансов из RPC при наличии счёта; иначе «получатели» как раньше из **`GET /v1/accounts`** без owner;
- независимо от источника получателей (active-book или `/v1/accounts`) адреса regulatory с `domain_lo == 00` отображаются и могут быть выбраны как обычные получатели;
- строка `New Recipient` всегда остаётся первой и доступной;
- в F6 форме editable-поля (`to` для `New Recipient`, `amount`, `fee`, `confirm`) имеют тёмно-серый фон только на value-части строки (label без фоновой подсветки), fixed-поля не помечаются как editable;
- `amount/fee` принимают decimal (`12`, `12.34`, `0.001`) со scale `1 PWM = 1_000_000 base units`; значения с >6 знаками после точки отклоняются (без округления);
- активное editable поле показывает видимый caret `|` при переключениях;
- внутри active editable-поля работают `Left/Right`, `Home/End`, вставка в середину строки, `Backspace/Delete` в позиции курсора;
- при отправке, если `GET /v1/account/:id` вернул `404`/любой non-success или bad JSON, submit не блокируется из-за nonce: используется fallback `nonce=0` (ошибка только при offline/timeout RPC);
- UI-loop остаётся отзывчивым при недоступном RPC: при offline/timeout можно продолжать навигацию (`Tab`, стрелки, переключение панелей) без "фризов" на сетевых вызовах;
- debug JSON не запрашивается каждый кадр: при удержании фокуса на строке обновляется с дросселированием/кэшем, переключение строк не вызывает заметной input-latency;
- submit в `F6` неблокирующий: сразу виден промежуточный статус (`submitting tx...`), а итог успех/ошибка появляется асинхронно без долгой блокировки ввода;
- по `H` открывается модалка `Operations History` с локальным списком последних отправок (`pending/ok/error`, latest-first), закрытие `H`/`Esc`/`Enter`;
- если отправок ещё не было, в истории отображается пустое состояние `No operations yet`;
- при ошибке submit запись в истории получает `error` и сохраняет текст ошибки;
- в нижней строке при offline/timeout виден явный красный индикатор `RPC offline` или `RPC timeout` **в начале** строки (рядом с `head:`/`accounts:` при ошибках опроса), чтобы на узком терминале не уезжал вправо; длинный `tip=` в блоке `height=… tip=…` укорачивается с `...`;
- в `F6/to` ambiguous legacy pretty без `/LO` (`pwm1-CY-f...`) отклоняется до submit с явной подсказкой про strict pretty `LABEL/LO` или canonical bech32dx;
- обе панели в compact-режиме показывают только колонки `Address` и `PWM` (`Address` визуально шире);
- в правой панели стрелка `Down` позволяет дойти до самого нижнего получателя (без off-by-one), `Up`/`Down` не выходят за границы;
- навигация (`Tab`, стрелки, `F4`, `F6`) работает без регрессий;
- в footer присутствует подсказка `H history`;
- выход по `q`/`F10` штатный.

### TUI-S01: F3 unlock без passphrase на CLI

1) Запустите TUI с encrypted wallet **без** `--wallet-passphrase` / `PWM_TUI_WALLET_PASSPHRASE`.
2) Убедитесь, что в статусе есть указание на `wallet: LOCKED`, а в подсказке F3 показан `unlock`.
3) Нажмите **F3**, введите passphrase, **Enter** — отправка через **F6** должна снова быть доступна, а F3-подсказка меняется на `lock`.
4) Нажмите **F3** повторно в состоянии unlocked: должен выполниться мгновенный lock (без модалки), затем `wallet: LOCKED`, F3-подсказка снова `unlock`.
5) **Esc** в модалке F3 закрывает диалог без разблокировки.

Ожидание: без unlock **F6** показывает сообщение о необходимости F3; после unlock — форма отправки открывается; после явного lock через F3 — снова требуется unlock.

### TUI-S02: авто-блокировка по таймеру

1) Запустите с `--wallet-unlock-secs 3` (или `PWM_TUI_WALLET_UNLOCK_SECS=3`).
2) Разблокируйте кошелёк (F3 или passphrase при старте).
3) Подождите >3 с и проверьте статус (`wallet: LOCKED` / отсутствие счётчика unlock) и что **F6** снова требует F3.

Ожидание: ключ сброшен из памяти, UI явно показывает блокировку. Для encrypted wallet при lock (таймер или ручной F3) очищается и кэш расшифровки для re-key.

### TUI-S03: plaintext wallet и F3

1) Запустите TUI с `plaintext_dev` wallet.
2) Нажмите **F3**.

Ожидание: информационное сообщение, что unlock не нужен.

### TUI-S04: F4 encrypt (`plaintext_dev`) и re-key (`encrypted`)

1) **Plaintext → encrypted:** запустите TUI с dev plaintext wallet (`mode: plaintext_dev`). Нажмите **F4**, введите passphrase и подтверждение, **Enter**. Убедитесь, что на диске `mode: encrypted`, plaintext поля секретов исчезли, `account_id_human` / `address_book` при необходимости нормализованы (как при загрузке).
2) **Re-key:** запустите с encrypted wallet; для сценария re-key нужен **кэш расшифровки**: либо старт с `--wallet-passphrase` / `PWM_TUI_WALLET_PASSPHRASE`, либо **F3** unlock. Нажмите **F4**, задайте новый passphrase+confirm, **Enter**. Проверьте, что старый passphrase больше не открывает файл, новый — открывает.
3) **Esc** в модалке F4 отменяет без записи.
4) Заблокированный encrypted wallet **без** кэша (не было F3 и не было passphrase при старте): **F4** должен показать подсказку про F3 / env.

Ожидание: несовпадение passphrase/confirm — ошибка в модалке, файл не портится; успешная запись атомарна (при сбое rename исходный файл сохраняется).

Альтернатива без `--wallet`: положите готовый файл **`default.yml` в текущую рабочую директорию** перед запуском `pwm-tui` — он будет использован как wallet (как если бы передали `--wallet default.yml`).

Fallback-путь для dev:

```powershell
$env:PWM_TUI_MASTER_SEED="<MASTER_HEX>"
cargo run -p pwm-tui --bin pwm-tui
```

Ожидание: **визуально** на экране TUI видна жёлтая зона предупреждения про seed fallback (wallet не задан). Перехват **stdout** для таких проверок **не считается** надёжным (alternate screen / raw mode); при отчёте можно вставить фрагмент, скопированный из окна консоли вручную.

## 6) Единый шаблон отчёта

```text
Date/Env: <YYYY-MM-DD>, PowerShell, PWM_RPC=<url>
Scenario: <CLI-N04 witness reject>
Steps: <кратко>
Expected: <кратко>
Actual: <кратко>
Status: PASS|FAIL
Evidence: <1-3 строки вывода/ошибки>
```
