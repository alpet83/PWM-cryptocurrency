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

Дополнительные policy-проверки для получателя:
- позитив: `--to` в strict pretty и canonical bech32DX обе формы принимаются;
- негатив: unknown/reserve/witness recipient отклоняются с явной ошибкой.
- `--master` + `--domain` использовать только как dev override (не основной путь smoke).

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
