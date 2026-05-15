# Sprint V2-4 Slice 3 — smoke после fix(pwmd) slice20 e2e (коммит `4a3a678`)

## 1. Scope recap

Тикет `tasks/20260506-v2-v4-slice3-smoke-review.json`: смоук pwmd↔CLI (эквивалент `cargo test` для slice20) после слайса E-3; независимое ревью согласованности с планом V2-4 Slice 3 (`docs/plans/mvp_v2.md`: §4 pwm-cli, §5 pwm-tui).

Заявленное изменение в **`crates/pwmd/src/slice20_e2e_tests.rs`** (коммит **`4a3a678`**, сообщение: import fee floor / ликвидность фикстуры): в потоке `slice20_dual_flow_ok` введён отдельный **`wallet-genesis.yaml`**: копия `wallet-cy` плюс **`wallet account add`** для того же `do_signer_di`, что и у кошелька **`wallet-do.yaml`** для `tx-import`. `genesis-build` переведён с `wallet-cy` на эту genesis-only копию. Кошельки для рантайм-транзакций (`wallet_cy`, `wallet_do`, получатели) не смешиваются с тем, что лежит в genesis-only файле, кроме согласованности сид/индексов. Дополнительно переименован тест `cross_shard_two_pwmd_bridge_federation_ok` → `cross_shard_bridge_ok` (вне основного сценария импорта).

## 2. Requirements fit

**Соответствие цели:** после введения пола комиссии импорта (`MIN_IMPORT_FEE_UNITS` в `pwm-core`, см. `crates/pwm-core/src/tx.rs`: константа и проверки минимума) подписант импорта на DO должен иметь **достаточный on-chain баланс** до `tx-import`. Ранее genesis строился только из `wallet-cy` (отправитель CY), поэтому аккаунт DO-подписанта не получал премайн — типичный сбой «insufficient balance» на HTTP-слое согласуется с handoff pwm-testing. Добавление того же `do_signer` в кошелёк, из которого строится genesis, и сохранение **отдельного** `wallet_do` только для CLI-импорта — логичная и минимальная правка фикстуры без изменения прод-логики.

**Пробелы / частичное покрытие:** в JSON тикета последняя запись pwm-testing — **FAIL** до фикса; повторный формальный прогон pwm-testing после `4a3a678` в `delegations` не отражён (остаётся организационным риском, не логическим дефектом патча). Продуктовый UX текстов reject (RFC Slice D, tester-guide) в этом диффе **не затрагивался** — вне узкого scope данного коммита.

## 3. Style and module shape

- В начале файла уже есть англоязычный модульный **`//!`** — ок.
- Комментарии к правке genesis-only кошелька и ссылка на **`MIN_IMPORT_FEE_UNITS`** понятно фиксируют мотивацию (роуминговая экономика).
- **`python scripts/check_rust_fn_name_segments.py crates/pwmd/src/slice20_e2e_tests.rs`**: `violations` — **пустой массив** (политика test_max=5 соблюдена).
- Переименование `cross_shard_bridge_ok` укорачивает идентификатор и убирает избыточность — в духе лимитов на сегменты имён в тестах.

## 4. Safety

Изменения — **только интеграционный тест**: временные каталоги, локальные порты, детерминированный dev seed из комментария Sprint 14. Новых доверенных границ (RPC к внешнему миру, произвольные пути извне) не добавлено. Паники/`expect` в тесте — ожидаемый стиль для e2e. Риски уровня «секрет в репозитории» не ухудшены относительно уже существующего жёстко заданного `master_seed_hex` в том же файле.

## 5. Tests

- **Покрыто:** полный dual-flow (перевод CY → export/finalize → handoff → import на DO → проверка баланса и логов) снова может завершиться успешно при наличии ликвидности подписанта импорта под пол комиссии.
- **Проверено в этой сессии ревью:** `cargo test -p pwmd --lib slice20_dual_flow_ok` — **1 passed** (~11 s на данной машине).
- **Пробелы:** `cross_shard_bridge_ok` в этом прогоне не запускался; при регрессиях транспорта/федерации имеет смысл периодически гонять оба теста модуля.

## 6. Verdict

**Утвердить с небольшими замечаниями (approve with nits).** Фикстура согласована с экономикой импорта (пол `MIN_IMPORT_FEE_UNITS` = 10_000 units в `tx.rs`). Разделение **genesis-only** кошелька и **рантайм** кошельков снижает путаницу; ключевой инвариант — один и тот же `do_signer_di` в `wallet_gen` (премайн) и `wallet_do` (подпись `tx-import`): при будущих правках теста это нужно сохранять явно.

**Остаточные риски:** (1) дрейф тикета: зафиксировать повторный smoke pwm-testing после мержа; (2) если изменится распределение премайна по аккаунтам в `genesis-build` или вырастет эффективный минимум комиссии импорта, фикстуру снова придётся подстроить; (3) жёстко прошитый seed остаётся только для dev/e2e, не для реальных сетей.

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/v2-v4-slice3-smoke-review-20260506.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 9000,
    "confidence": "low"
  }
}
```

**Однострочный вердикт для оркестратора:** Verdict: **approve with nits** — `slice20_dual_flow_ok` зелёный после `4a3a678` (проверено `cargo test -p pwmd --lib slice20_dual_flow_ok`); зафиксировать повтор pwm-testing в тикете и следить за инвариантом signer/genesis.
