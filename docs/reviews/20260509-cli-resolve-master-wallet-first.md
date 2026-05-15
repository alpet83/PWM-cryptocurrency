# pwm-cli: приоритет `resolve_master_seed` и кошелёк — предварительное ревью (тикет `20260509-cli-resolve-master-wallet-first`)

Дата отчёта: 2026-05-15. Объект анализа: **текущее** состояние `pwm-cli` до смены политики; продуктовый Rust не менялся.

## Scope recap

- **Источник требования:** билет [`tasks/20260509-cli-resolve-master-wallet-first.json`](../../tasks/20260509-cli-resolve-master-wallet-first.json) и бриеф в нём: оценить риск текущего порядка источников master (`--master`/clap-env, затем `MASTER_SEED`, затем YAML при явном `--wallet-out` и существующем файле), предложить нормативную политику (wallet-first при существующем файле без перезаписи, исключение при `--overwrite-wallet`), учесть зашифрованный кошелёк без passphrase.
- **Связь с MVP-checklist:** в тикете `mvp_checklist: []`; привязка к чеклисту не заявлена.
- **Просмотренные места:** `crates/pwm-cli/src/cmd_addr.rs` (`resolve_master_seed`, `persist_wallet_account_output`, `run_addr_derive`, `run_addr_bruteforce`, `bruteforce_resume_index`), `crates/pwm-cli/src/wallet/account.rs` (`wallet_account_add_seed`), тесты в `crates/pwm-cli/src/tests/mod.rs` (в т.ч. `seed_resolve_*`, `bf_persist_*`). Проверка имён: `python scripts/check_entity_name_segments.py` по `cmd_addr.rs` и `wallet/account.rs` — нарушений нет.

## Requirements fit

Тикет описывает **желаемую** политику (реализация — зона `pwm-coding`). Для **текущего** кода:

- **Фактический приоритет** в `resolve_master_seed`: непустой trimmed `cli_master` (аргумент `--master` и значение, которое подставляет clap из env, если так настроено в определении CLI), затем непустой `MASTER_SEED`, затем при `wal_out_explicit` и существующем пути — чтение кошелька и `master_seed_hex` через `wallet_secrets`.
- **Соответствие будущей цели:** отсутствует; это ожидаемо для этапа ревью до правок.

## Style and module shape

- Структура модулей и комментарии в затронутых файлах выглядят согласованно с остальным `pwm-cli`; новых «раздутых» façade-файлов в рамках этого слайса нет.
- Имена сущностей в просмотренных файлах укладываются в лимит сегментов (см. запуск checker выше).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## Safety

Ответ на вопрос 1 билета (**насколько обоснованы страхи**):

**Частично обоснованы; сценарии различаются.**

1. **Существующий кошелёк, append (нет `--overwrite-wallet`, файл уже есть)**  
   После успешной деривации `persist_wallet_account_output` вызывает `wallet_account_add_seed`. Там есть жёсткая проверка: производный по переданному `seed` ключ для **первого (baseline)** аккаунта в файле должен совпасть с записанным `id_hex`. При неверном `$env:MASTER_SEED` или `--master` цепочка обычно **заканчивается отказом** с сообщением вроде «provided master seed does not match existing wallet accounts; refusing to append», а не тихим повреждением файла противоречивыми ключами под чужим сидом. Это существенно снижает боевой риск «записали аккаунт от другого мастера в чужой файл» для ветки append.

   **Остаточные риски даже при append:** ложный триггер в окружении (`MASTER_SEED`) приводит к **бесполезной** или **вводящей в заблуждение** работе (брутфорс/деривация на «чужом» дереве) до ошибки на persist; для оператора это выглядит как «команда сломалась», а не как явный «конфликт сида с кошельком».

2. **Перезапись кошелька (`--overwrite-wallet` в `addr-bruteforce`)**  
   `persist_wallet_account_output` в ветке перезаписи строит **новый** YAML и сохраняет его целиком. Baseline-проверка append **не применяется**. Риск **полной подмены** базового `master_seed_hex` и потери согласованности с ранее записанными derivation paths **реален** и соответствует описанному страху: это намеренная деструктивная операция при ошибочном сиде в CLI/env.

3. **`addr-derive`**  
   Вызов `persist_wallet_account_output` идёт с фиксированным `overwrite_wallet: false` — только create/append. Риск той же тяжести, что у `overwrite-wallet`, от этого флага **не возникает**; остаётся риск п.1 и UX/CPU.

Ответ на вопрос 2 (**рекомендуемая политика**):

- **Да**, логика «при явном `--wallet-out`, файле на диске и **без** `--overwrite-wallet` брать авторитетный master **только** из кошелька и **отвергать** команду, если пользователь также задал непустой сид через CLI/`MASTER_SEED` (и при необходимости `PWM_MASTER_SEED`, если это отдельный канал от `MASTER_SEED`), который **не эквивалентен** сиду кошелька — выглядит **правильной** операторской политикой: меньше скрытых конфликтов, ранний явный diagnostic, не полагаться на побочный эффект baseline-check на persist.
- **Исключение `--overwrite-wallet`:** разумно трактовать как явное намерение «источник истины — CLI/env; старый файл не защищаем». Документация и help должны это кричать (уже частично отражено смыслом флага).

Ответ на вопрос 3 (**побочные эффекты, зашифрованный кошелёк**):

- При **wallet-first** отсутствие passphrase для encrypted-режима приведёт к ошибке расшифровки **до** любого fallback на env — это ожидаемо и **предпочтительнее с точки зрения безопасности**, чем незаметное использование `MASTER_SEED` «мимо» кошелька. Стоит явно описать в help/сообщении об ошибке, что нужен `--wallet-passphrase` (или соответствующий env для кошелька, если принят в проекте), а не «добавьте MASTER_SEED».

Дополнительное наблюдение: текущий приоритет `MASTER_SEED` **выше** чтения кошелька, поэтому **один только** stray `MASTER_SEED` может полностью обойти wallet-fallback даже при явном `--wallet-out` — это усиливает аргумент за wallet-first при существующем файле без overwrite.

## Tests

- Есть модульные тесты приоритета (`seed_resolve_cli_wins`) и fallback кошелька (`seed_resolve_wallet_plain`, `seed_resolve_wallet_enc`, `seed_resolve_wallet_miss`), append/создание (`bf_persist_append_default`, `addr_der_out_create_miss`).
- **Пробел:** нет теста, что при существующем кошельке и несовпадающем CLI/env append **отклоняется** на уровне `resolve_master_seed` (сейчас это косвенно ловится на `wallet_account_add_seed`, но политика «конфликт на входе» потребует новых тестов в `pwm-testing`).

## Verdict

**Approve with nits** для постановки работ в `pwm-coding`: зафиксировать wallet-first + явный conflict error при существующем файле и заданном сиде извне; исключение `--overwrite-wallet` с усиленным предупреждением в UX/доках; обновить тесты и help. Текущий код **частично** защищает от порчи файла при append за счёт baseline-проверки, но **не** устраняет риск перезаписи и плохой операторский UX при конфликте env.

## Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260509-cli-resolve-master-wallet-first.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: low
```

---

```powershell
# git-handoff
# Уже закоммичено в main: 00294c5…714a648…6988f65 (отчёт, chores по тикету и typo/handoff).
Set-Location 'REPO_ROOT'
git pull
```
