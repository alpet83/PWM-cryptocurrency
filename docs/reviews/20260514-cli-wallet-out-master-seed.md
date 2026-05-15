# Ревью: pwm-cli — приоритет источника master seed для `addr-derive` / `addr-bruteforce`

**Тикет:** `tasks/20260509-cli-wallet-out-master-seed-fallback.json`  
**Дата:** 2026-05-14  
**Ревьюер:** pwm-review (независимое)

## 1. Scope recap

Слайс закрывает операторский сценарий: для устаревшего `addr-derive` и основного `addr-bruteforce` нужен предсказуемый порядок получения 32-байтного master seed:

1. Явный `--master` (строка hex).
2. Переменная окружения `PWM_MASTER_SEED` (подключена к полю `master` через `clap` `env = "PWM_MASTER_SEED"` в `cli_cmd.rs`).
3. Если **на CLI явно передан** `--wallet-out`, файл по разрешённому пути **существует**, а `master` после парсинга пуст — загрузка кошелька (`load_wallet_yaml_upgrade`), извлечение секретов с учётом `wallet_secrets` и глобального `--wallet-passphrase` / `PWM_WALLET_PASSPHRASE`, затем разбор `master_seed_hex`.

В stateless-режиме `addr-derive` (без явного `--wallet-out`) по-прежнему требуется `--master` или `PWM_MASTER_SEED`; fallback из файла намеренно **не** включается для неявного пути (в т.ч. дефолт `~/.pwm-crypto/default-wallet.yaml`), что согласовано с `issues-report.md`.

## 2. Requirements fit

**Соответствует заявленным правилам.**

- `resolve_master_seed` в `cmd_addr.rs` сначала обрабатывает `cli_master`; при отсутствии источника и `wal_out_explicit == false` возвращает явную ошибку с текстом про `--master` или `PWM_MASTER_SEED`.
- При `wal_out_explicit == true` и отсутствии файла — отдельная ошибка про необходимость существующего `--wallet-out`.
- При существующем файле — загрузка и `wallet_secrets` с passphrase-aware ошибками.

`cli_dispatch.rs` передаёт `wal_out_explicit = wallet_out.is_some()` до `resolve_wallet_out_path`, что корректно отделяет «явно указанный выход» от дефолтного пути.

**Незначимые пробелы (не блокеры):** нет отдельного unit-теста «`PWM_MASTER_SEED` перекрывает существующий wallet» — поведение фактически делегировано `clap` (env заполняет поле до вызова `resolve_master_seed`). Явный тест был бы полезен только как регрессия на будущие ручные правки парсинга.

## 3. Style and module shape

- Имена и структура согласованы с остальным `pwm-cli`; новые символы не расползают в «комбайны» в `lib.rs`/`main.rs`.
- **`scripts/check_entity_name_segments.py`** по путям `cli_cmd.rs`, `cli_dispatch.rs`, `cmd_addr.rs`, `tests/mod.rs`: **нарушений нет** (`violations: []`).
- Модульные `//!` у затронутых файлов на месте или не требуют расширения для объёма изменений.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- **Сид в окружении:** `PWM_MASTER_SEED` попадает в среду процесса; на общих машинах возможен leak через `/proc`, дампы, логирование — это ожидаемый класс риска для dev/ops, не регрессия слайса.
- **Passphrase:** для зашифрованного кошелька без passphrase `wallet_secrets` даёт контролируемую ошибку; тест `seed_resolve_wallet_enc` это фиксирует.
- **Explicit `--wallet-out`:** оператор может указать чужой/устаревший файл — будет использован `master_seed_hex` оттуда; это осознанный trust boundary. Важно не путать с stateless-режимом, где путь в stdout может разрешаться к дефолту, но **seed оттуда не читается** — см. UX nit ниже.

Паник в новой логике резолва seed не добавлено; ошибки — через `Result` / `exit_user_error`.

## 5. Tests

Покрыто в `crates/pwm-cli/src/tests/mod.rs`:

- Приоритет явного `--master` над файлом (`seed_resolve_cli_wins`).
- Fallback из plaintext wallet (`seed_resolve_wallet_plain`).
- Encrypted wallet: ошибка без passphrase и успех с passphrase (`seed_resolve_wallet_enc`).
- Явный `--wallet-out` при отсутствии файла и stateless без seed (`seed_resolve_wallet_miss`).
- Парсинг `addr-bruteforce` без `--master` (`bf_cli_master_opt`).

**Пробел:** нет интеграционного smoke «полный CLI вызов с env-only и wallet-only», но unit-уровень для `resolve_master_seed` достаточен для слайса.

## 6. Verdict

**PASS_WITH_NITS** — логика приоритетов и граница «только explicit `--wallet-out`» реализованы верно, тесты адекватны; остаются только операторская документация и косметические усиления тестового покрытия.

**Nits (не блокируют merge):**

1. В `issues-report.md` уже отмечено: явно прописать в operator cheatsheet, что fallback seed из кошелька работает **только** при явном `--wallet-out`; дефолтный путь к файлу не включает этот режим. Дополнительно: в stateless `addr-derive` в stdout может фигурировать разрешённый `wallet_path` к дефолту — не читать как «оттуда взят seed».
2. По желанию: тест «env seed vs wallet file» для защиты от регрессии порядка.
3. По желанию: одна фраза в help у `--master` для `addr-derive` / `addr-bruteforce` про третий источник (filesystem), чтобы снизить число тикетов от операторов.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260514-cli-wallet-out-master-seed.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 15000
  confidence: low
```

**Glossary:** не финальное ревью спринта — **`docs/GLOSSARY.md` не менялся** (отдельная запись в отчёте не требуется по правилам п.4 промпта для подслайса).

---

**Вердикт одной строкой для оркестратора:** `PASS_WITH_NITS` — реализация соответствует спеке; nits: operator docs, опциональный тест env-vs-wallet, опциональное уточнение help.
