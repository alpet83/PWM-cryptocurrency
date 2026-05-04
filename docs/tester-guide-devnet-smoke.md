# Tester guide: devnet запуск и smoke-проверка

Этот гайд для тестера: как быстро поднять локальный devnet и проверить базовые пользовательские сценарии без погружения во внутреннюю реализацию.

## 1) Что нужно заранее

- Установлены `Rust` и `cargo` (команда проверки: `cargo --version`).
- Репозиторий PWM открыт в терминале.
- Подготовлены минимум 3 окна терминала:
  - A: нода `pwmd`,
  - B: CLI-команды `pwm`,
  - C: `pwm-tui`.

## 2) Быстрый старт (Windows + универсально)

По умолчанию RPC ноды: `http://127.0.0.1:3030`.

### Терминал A: запустить ноду

```powershell
cargo run -p pwmd --bin pwmd
```

Ожидание: процесс запущен без падения, в логе появляются строки про старт сервера/запечатывание блоков.

### Терминал B: задать RPC и проверить CLI

PowerShell (Windows):

```powershell
$env:PWM_RPC="http://127.0.0.1:3030"
cargo run -p pwm-cli --bin pwm -- --help
```

Опционально для CLI можно задать RPC timeout через `PWM_CLI_RPC_TIMEOUT_MS` (default `10000` ms, max `120000` ms; некорректные значения игнорируются).

Если используется `cmd.exe`, аналог переменной:

```cmd
set PWM_RPC=http://127.0.0.1:3030
```

### Терминал C: запустить TUI

PowerShell (с той же RPC):

```powershell
$env:PWM_RPC="http://127.0.0.1:3030"
cargo run -p pwm-tui --bin pwm-tui
```

Для TUI timeout настраивается отдельной переменной `PWM_TUI_RPC_TIMEOUT_MS` (другой env, чем у CLI; default `3000` ms, max `120000` ms).

Ожидание: TUI открывается, видны таблицы/статус, выход по `q` или `F10`.

## 3) Минимальные smoke-сценарии

Ниже сценарии, которые достаточно пройти для базовой проверки окружения.

### A. Проверка health/head ноды

Команда:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/head"
```

Ожидание:
- есть ответ JSON;
- присутствуют поля высоты/хэша головы;
- команда завершается без ошибки подключения.

### B. Генерация ключа и derivation аккаунта

1) Сгенерировать master seed:

```powershell
cargo run -p pwm-cli --bin pwm -- key-gen
```

2) По полученному hex seed получить адрес/аккаунт:

```powershell
cargo run -p pwm-cli --bin pwm -- addr-derive --master <MASTER_HEX> --domain CY
```

Ожидание:
- `key-gen` возвращает seed в hex;
- `addr-derive` возвращает `account_id_human` в strict pretty (`pwm1-...-f...-t...`) и `account_id_bech32dx` отдельной строкой.

### C. `wallet init` user-flow (country + auto bruteforce)

```powershell
cargo run -p pwm-cli --bin pwm -- wallet init --country CY --wallet-out .\tmp\wallet-cy.yaml
```

Ожидание:
- команда завершена без ошибки;
- в выводе есть `country_label CY`, `domain_match_mode high_byte_only`, `derivation_path m/0/...`;
- файл wallet создан.

Негативная проверка:

```powershell
cargo run -p pwm-cli --bin pwm -- wallet init --country MSFT --wallet-out .\tmp\wallet-msft.yaml
```

Ожидание: reject с причиной (в этой фазе принимаются только country/regulatory labels).

### D. Путь init + простой transfer

1) Подготовьте wallet (основной путь подписи для tx):

```powershell
cargo run -p pwm-cli --bin pwm -- wallet init --country CY --wallet-out .\tmp\wallet-cy.yaml
```

2) Инициализация аккаунта от wallet:

```powershell
cargo run -p pwm-cli --bin pwm -- tx-init --wallet .\tmp\wallet-cy.yaml --index 0 --flags 0
```

3) Простой перевод от wallet:

```powershell
cargo run -p pwm-cli --bin pwm -- tx-send --wallet .\tmp\wallet-cy.yaml --to <PRETTY_OR_CANONICAL_RECIPIENT> --amount 10
```

4) Повторная проверка head (должен продолжать обновляться):

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/head"
```

Ожидание:
- команды `tx-init` и `tx-send` завершаются без фатальной ошибки;
- нода доступна и продолжает отвечать;
- в логе ноды есть следы обработки отправленных транзакций.
- при проблеме nonce fetch (HTTP не-2xx/битый JSON/нет `nonce`) команда падает явной ошибкой вместо «тихого» `nonce=0`.

Дополнительные policy-проверки для получателя:
- позитив: `--to` в strict pretty и canonical bech32DX обе формы принимаются;
- негатив: unknown/reserve/witness recipient отклоняются с явной ошибкой.
- `--master` + `--domain` использовать только как dev override (не основной путь smoke).

Negative expectation (cross-domain):
- попытка cross-domain `tx-send` не заменяет roaming-flow и должна уходить в reject/операторский маршрут `tx-export -> tx-import`.
- после roaming intent create проверяйте `GET /v1/roaming-intents/:id`: поля `relay_mode=manual_handoff_required` и `relay_hint` обязаны явно сообщать, что auto-relay пока не реализован.
- для быстрой диагностики runtime-приема используйте `GET /v1/flow/recent` и убедитесь, что есть свежие `accepted:*`/`sealed:*` записи по вашему действию.

Архитектурное напоминание (Sprint 15, closeout S3.12): упрощённый путь **«одного окна»** (частые опросы RPC / наблюдение чужого шарда через trusted peer там, где это включено) годится для **MVP/devnet** и ручных прогонов. Для массового продакшена этот паттерн **не масштабируется** и может **перегружать сеть**; целевое развитие — отдельные read-сервисы (explorer) и подписка клиента на обновления по адресам (см. `docs/reviews/sprint-15-s3-12-9-closeout.md`).

### E. Проверка TUI и обновления данных

1) Открыть `pwm-tui` (см. раздел запуска).
2) Убедиться, что статус/данные не "зависли":
   - значения обновляются во времени (например, высота/состояние);
   - навигация стрелками работает;
   - выход по `q` или `F10` корректен.

Ожидание:
- интерфейс запускается стабильно;
- периодическое обновление данных присутствует;
- приложение завершается штатно.

## 4) Что фиксировать в отчете тестера

Для каждого шага записывайте короткую строку в формате:

- команда (как запускали);
- результат (что вернулось/что увидели);
- статус: `PASS` / `FAIL`;
- короткий фрагмент лога (1-3 строки) или текст ошибки.

Рекомендуемый шаблон:

```text
[Сценарий] A. Head check
Command: Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/head"
Result: JSON returned, height increased on repeated call
Status: PASS
Log excerpt: "..." 
```

Если шаг упал, добавьте:
- время запуска;
- где выполнялось (PowerShell/cmd);
- что помогло/не помогло при повторе.

Отдельно для persistence ошибок:
- если `POST /v1/tx` или `POST /v1/roaming-intents` вернули `500` с текстом про snapshot save, фиксируйте это как явный persistence-fail (это ожидаемое strict поведение, не silent успех);
- приложите `GET /v1/status` (поля `phase`, `snapshot_error`, `snapshot_file`) и 1-3 строки из лога `pwmd snapshot file path=...`.

## См. также

- [tester-guide-cli-tui-scenarios.md](./tester-guide-cli-tui-scenarios.md) — два шарда (`pwmd` A/B), переключение `PWM_RPC`, сценарии CLI/TUI и burn path.
- [rfc/9-crossdomain-roaming.md](./rfc/9-crossdomain-roaming.md) — текущий контракт roaming MVP (Sprint 13).
