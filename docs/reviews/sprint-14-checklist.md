# Sprint 14 checklist (multi-address wallet)

Конвейер на **каждый код-слайс**: `pwm-coding` → `pwm-testing` → `pwm-review` → решение оркестратора.

## Slice 0 — Spec / audit (done в docs)

- [x] Аудит полей: [sprint-14-wallet-schema-audit.md](sprint-14-wallet-schema-audit.md)
- [x] RFC draft: [../rfc/10-wallet-file-format-v3.md](../rfc/10-wallet-file-format-v3.md)
- [x] План roadmap: `docs/plans/mvp_v1_testnet_multi-sprint.md` — секция Sprint 14
- [x] Changelog: `docs/CHANGELOG.md`

## Slice 1 — pwm-core + pwm-cli (структуры, load, миграция)

- [x] `pwm-coding`: парсинг v3 plaintext_dev, маппинг v3→`WalletReadHeader`, defer encrypted v3; проверка `derivation_path`↔`derivation_index`; `detect_schema_version` без поля → `2`
- [x] `pwm-testing`: `cargo test -p pwm-cli` (90), `cargo test -p pwm-core` (67); правка устаревших `types` тестов под актуальный `domain_index`
- [x] `pwm-review`: minor → закрыто доп.проверками path/index и дефолтом schema

## Slice 2 — CLI операторские команды

- [x] `pwm-coding`: `wallet account list|add|use`
- [x] `pwm-testing`: интеграционные тесты парсинга CLI
- [x] `pwm-review`: UX сообщений и совместимость с v2 (после remediation: `approve with minor`, см. `sprint-14-slice2-review.md`)

## Slice 3 — pwm-tui левая панель

- [x] `pwm-coding`: список всех `accounts`, highlight active
- [x] `pwm-testing`: smoke с фикстурой v3 (или mocked header)
- [x] `pwm-review`: нет утечки секретов в UI логах (`approve with minor`, см. `sprint-14-slice3-review.md`)

## Slice 4 — closeout (v3 active-account mismatch)

- [x] `pwm-coding`: добавлен негативный TUI-тест загрузки v3-кошелька с невалидным `active_account_id_hex`; проверяется clear error и отсутствие panic
- [x] `pwm-testing`: независимый прогон `cargo test -p pwm-tui` и точечный negative case (см. `sprint-14-slice4-testing.md`)
- [x] `pwm-review`: финальный closeout review `approve with minor` (см. `sprint-14-slice4-review.md`)

## Slice 5 — automatic wallet migration

- [x] `pwm-coding`: remediation Slice 5 BLOCK — `load_wallet_yaml` снова read-only; миграция schema v2 -> v3 выполняется только по явному `--upgrade-wallet` (CLI/TUI), с сохранением без раскрытия секретов
- [x] `pwm-testing`: remediation validation (details: `sprint-14-slice5-remediation-testing.md`)
- [x] `pwm-review`: remediation финально `approve with minor` (см. `sprint-14-slice5-remediation-review.md`); block из `sprint-14-slice5-review.md` закрыт

## Slice 6 — v3 create-path + addr-bruteforce resume

- [x] `pwm-coding`: новые create-path (`wallet init`, `wallet import-seed`, `addr-bruteforce`) сразу пишут schema v3; user-visible pretty поле унифицировано как `id_pretty`; `addr-bruteforce` возобновляет поиск по metadata существующего wallet (`start = max_derivation_index + 1`) без повторного сканирования с нуля
- [x] `pwm-testing`: независимая валидация `cargo test -p pwm-cli` + точечные сценарии resume с существующим wallet (см. `sprint-14-slice6-testing.md`)
- [x] `pwm-review`: remediation выполнен — strict overwrite для create-path и upgrade persist cleanup; merge-preserve оставлен только для update-path (`wallet account add/use`) (см. `sprint-14-slice6-remediation-coding.md`)

## Demo-ready (конец спринта)

- Один файл v3, два `accounts`, `tx-send` с каждого после `use`; TUI показывает оба в левой панели.

- [x] Sprint closeout snapshot done: `docs/reviews/sprint-14-closeout.md`.
