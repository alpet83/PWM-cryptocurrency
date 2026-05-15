# Review: FAIL «balance verify (target)» (delta vs expected) и межшардовый import fee

**Дата:** 2026-05-07  
**Тикет:** `tasks/20260507-target-balance-verify-fee-gap.json`  
**Агент:** `pwm-review`

## 1. Scope recap

Проверена жалоба по TUI/diagnostics кросс-шардового flow:  
`balance verify (target): FAIL — delta=990000 raw, expected 1000000` при `pre=1000000000 post=1000990000`.  
Гипотеза: ожидаемая дельта не учитывает минимальную комиссию импорта.  
В scope — трассировка от строки шага 5 в TUI до правил начисления на целевом шарде (`pwm-core`), без правок прод-кода в этом коммите.

## 2. Requirements fit

| Ожидание пользователя | Факт по коду |
|----------------------|--------------|
| Шаг 5 подтверждает, что баланс получателя на target вырос на сумму, согласованную с протоколом | Сравнение выполняется с **полным `amount` экспорта**, тогда как при стандартном пути TUI импортёр = получатель и протокол даёт **нетто `amount − import_fee`** |

Итог: диагностика **не соответствует** фактической семантике state для типичного roaming-пути; жалоба воспроизводима при `import_fee = 10_000` и `amount = 1_000_000`.

## 3. Style and module shape

Изменений в рамках этого ревью в исходниках не вносилось. Для будущего фикса в `pwm-tui`: имена и модульность затронуты минимально (локальная формула/аргументы и текст подсказки). Запуск `scripts/check_rust_fn_name_segments.py` на будущий diff — по желанию оркестратора (slice не менялся в этом коммите).

## 4. Safety

Проблема **не** указывает на порчу леджера или двойное списание: в `pwm-core` импорт атомарно списывает `import_fee` с аккаунта подписанта и начисляет `amount` получателю; при `signer == recipient` это одна и та же запись — нетто совпадает с наблюдаемым `post − pre = 990_000`.

Риск только **операторский**: ложный FAIL в TUI может заставить искать несуществующую ошибку учёта.

## 5. Tests

Явных unit/integration-тестов на `format_balance_verify_step5` и ожидаемую дельту не найдено (поиск по репозиторию даёт только `roaming.rs`). Для фикса желательно добавить тест(ы) на ожидаемый net credit при `MIN_IMPORT_FEE_UNITS` и при произвольном `import_fee ≥ MIN` (если API позволит задавать fee явно).

## 6. Root cause (доказательная цепочка)

**Где формируется шаг 5**

- `crates/pwm-tui/src/roaming.rs`: после `status == "imported"` вызывается  
  `format_balance_verify_step5(&target_rpc, pre_recv_bal, post_bal, amount, fee)`, где `amount` — сумма экспорта, `fee` — **комиссия экспорта на source**, не импорта.
- `format_balance_verify_step5` считает `delta = post − pre` и требует `delta == expected_credit`; в качестве `expected_credit` передаётся **только `amount`**.

**Откуда 10_000**

- `crates/pwm-core/src/tx.rs`: константа `MIN_IMPORT_FEE_UNITS = 10_000`; при `SignedTx::sign_body` для `TxBody::Import` выставляется `import_fee = Some(MIN_IMPORT_FEE_UNITS)`.

**Фактическая формула на целевом шарде**

- `crates/pwm-core/src/state.rs`, ветка `TxBody::Import`: у аккаунта подписанта (`id == computed_account_id()`) сначала `balance_pwm -= import_fee`, затем при `to == id` выполняется `balance_pwm += amount`.  
  Для пути TUI `submit_import_after_relay` подписывает импорт от имени **`to`**, значит **`to == id`**, и наблюдаемая дельта баланса получателя равна **`amount − import_fee`** (при отсутствии других транзакций между снимками).

Сопоставление с симптомом: `1_000_000 − 10_000 = 990_000` — совпадает с «FAIL … delta=990000 … expected 1000000».

**Текст `fee_note` в TUI**

Сейчас он описывает списание на **источнике** (`amount + export fee`), а не списание **import_fee на target** у получателя-подписанта; это усиливает путаницу при ложном FAIL.

## 7. Классификация

**Ложная тревога UX / ошибка ожиданий в верификаторе TUI**, а не ошибка учёта цепочки импорта в `pwm-core` для описанного сценария.

## 8. Verdict (review)

**Approve implementation of a narrow TUI-side fix** — изменить ожидаемую дельту и пояснение так, чтобы они отражали правило state (нетто кредит получателя-подписанта).

Приоритетный **fix-list для `pwm-coding`**:

1. **`crates/pwm-tui/src/roaming.rs`** — в месте вызова `format_balance_verify_step5` передавать ожидаемый нетто-кредит, например  
   `expected_credit = amount.saturating_sub(import_fee_expected)`, где  
   `import_fee_expected` берётся из фактически подписанной импорт-транзакции (после `SignedTx::sign_body` для `Import` это поле `import_fee`, по умолчанию `MIN_IMPORT_FEE_UNITS`) или явно из `pwm_core::tx::MIN_IMPORT_FEE_UNITS`, если TUI не меняет fee.
2. **Тот же файл, `format_balance_verify_step5`** — скорректировать текст `fee_note`: разделить (а) комиссию экспорта на source и (б) комиссию импорта на target; явно указать, что при импорте от имени получателя шаг 5 сравнивает **нетто** `amount − import_fee`.
3. **Тесты** — добавить покрытие на совпадение дельты с `amount - MIN_IMPORT_FEE_UNITS` для типичного self-import пути (или чистую функцию расчёта ожидаемого кредита, если её выделят).

## 9. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  review_md: docs/reviews/20260507-target-balance-verify-fee-gap.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 4200
  confidence: low
```

Примечание: `result: PASS` означает успешное завершение ревью с выводом; продуктовый вердикт по слайсу — см. §6–8 (нужен узкий фикс TUI).

---

**Однострочный вердикт для оркестратора:** `NEED-FIX` (ожидание шага 5 в TUI не учитывает `import_fee` при signer==recipient; расхождение ровно `MIN_IMPORT_FEE_UNITS`).
