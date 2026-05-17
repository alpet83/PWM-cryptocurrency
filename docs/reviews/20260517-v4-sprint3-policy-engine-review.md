# Review: V4-3 pure policy evaluator (`20260517-v4-sprint3-policy-engine`)

**Reviewer role:** `pwm-review` (read-only product Rust).  
**Ticket:** `tasks/20260517-v4-sprint3-policy-engine.json`  
**Scope (coding artifacts):** `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/tx.rs`, `crates/pwmd/src/api/common.rs`  
**Specs:** `docs/plans/mvp_v4.md` (Sprint V4-3), `docs/rfc/6-policy-engine.md`, `docs/rfc/14-claim-burn-api-error-contract.md`

---

## 1. Scope recap

Тикет обещает чистый read-only слой `evaluate_policy`, выравнивание preflight/apply, базовые виды политик (`sender_filter`, same-domain routing, default behavior, `cosign_required`), стабильные `E_POLICY_*` отказы и **не** реализовывать emergency redirect/finalization поведение V4-4.

Реализация сосредоточена в `State::evaluate_policy` + ранний вызов из `State::apply_tx_with_ctx`; HTTP preflight для локальных (не export/import) транзакций использует `precheck_apply_with_ctx` → тот же `apply_tx_with_ctx` на клоне состояния (`handlers_tx.rs`).

---

## 2. Findings (по серьёзности)

**Re-review:** описание ниже относится к первичному проходу. Спорные места уровня High/Medium по `sender_filter`, `default_behavior` и generic cosign **закрыты нормативно** RFC 6 §10.1 и блоком **V4-3 semantic boundary** в `mvp_v4.md`; актуальный вердикт — **PASS** (§7).

### High — семантика `sender_filter` vs ожидание whitelist / метаданные

В `evaluate_policy` для входящего `Transfer` при активном `SenderFilter` у **инициализированного** получателя условие: отказ, если `sender_id != to`. На практике это блокирует любой входящий перевод от третьей стороны; «разрешённый» случай `sender == to` пересекается с запретом self-transfer на уровне тела транзакции. То есть политика работает как «запрет входящих от других аккаунтов», а не как фильтр по списку разрешённых отправителей (`pwm-info` уже предупреждал про отсутствие on-chain списка).

**Риск:** имя политики и RFC-подобное описание «sender filter» могут вводить оператора в заблуждение; до появления явных данных whitelist в модели аккаунта это скорее «incoming closed», чем фильтр.

### Medium — `default_behavior`: только «deny incoming transfer», нет режима allow/deny в типе

`PolicyKind::DefaultBehavior` не параметризован (в отличие от будущего явного allow/deny в спеках/рисках pwm-info). В коде активная политика всегда приводит к `PolicyDenied` на входящий `Transfer` к инициализированному получателю.

**Риск:** расхождение с формулировками плана «default reject/allow behavior» и RFC 6 (перечисление primitive без нормативной привязки к одному варианту на enum).

### Medium — `cosign_required`: проверяется подпись на canonical message, но не роль и не политика ролей

`has_valid_cosign` принимает любую запись в `cosigns`, для которой `verify(signer_pk, signing_message, signature)` успешен; поле `CosignRole` не участвует в решении. Негатив «подменённое сообщение» покрыт тестом `policy_cosign_bad_sig_deny`.

**Риск:** для сценариев «только Organization / только Rescue» обход возможен любым посторонним ключом, который подпишет тот же `signing_message()` — если продукт ожидал строгого матчинга роли к политике (см. pwm-info R3).

### Low — заглушка `PolicyDecision::Redirect` и код ответа API

`evaluate_policy` нигде не возвращает `Redirect`; в apply ветка `Redirect(_) => Err(TxError::PolicyDenied)` — корректная защита от scope creep, но при будущем включении redirect клиенты временно увидят `E_POLICY_DENIED`, а не специализированный код (ожидаемо до V4-4).

### Low — порядок «до мутации» для `Init` на новом аккаунте

Для первичного `Init` при отсутствии строки аккаунта в состоянии код вставляет stub-запись в `accounts` **до** вызова `evaluate_policy`. Чистота «нет записи в стейт до политики» для этого краевого случая нарушена; на текущую логику evaluator (нет веток, зависящих от наличия stub) это, вероятно, не влияет.

### Low — политики получателя не применяются к неинициализированному получателю

Условие `.filter(|acc| acc.initialized)` для блока recipient-policies означает, что при переводе на неинициализированный аккаунт фильтры получателя не срабатывают на этапе policy; дальнейший отказ идёт через `require_recipient` / инициализацию. Осознанное упрощение, но стоит держать в голове для будущих расширений.

---

## 3. Requirements fit

| Требование | Оценка |
|------------|--------|
| Чистый evaluator без мутаций внутри fn | Да: `evaluate_policy(&self, …)` только читает карту аккаунтов. |
| Вызов до мутации балансов/nonce в apply | Да: после проверки nonce/подписи, до `match &tx.body`; см. нит про stub для Init. |
| Preflight ≡ apply для policy | Да: общий путь через `apply_tx_with_ctx`; есть тест `policy_precheck_apply_same_err`. |
| Нет логики emergency redirect/finalization в apply | Да: `RoutingEmergencyRedirect` не обрабатывается в evaluator; redirect не возвращается. |
| Структурированные отказы RFC 14 | Маппинг `TxError` → `E_POLICY_*` в `tx_err_wire` полный для перечисленных policy-вариантов. |

После RFC §10.1 и semantic boundary перечисленные ранее «пробелы» относительно whitelist, allow-режима и role-binding переклассифицированы как **намеренные ограничения V4-3**, а не расхождение спека с кодом.

---

## 4. Style and module shape

- Файл `state.rs` уже имеет модульный баннер `//!`; новые хелперы (`policy_is_active`, `has_valid_cosign`, …) локальны и короткие.
- `python scripts/check_entity_name_segments.py` на заявленных путях: **violations пусто**.

### Wire JSON / u128

Slice опирается на существующие контракты: в `tx.rs` суммы/fee в телах транзакций используют `serde(with = "crate::ser_json_u128")` там, где нужно для JSON. Изменения в `common.rs` касаются только маппинга ошибок, не новых peer-wire полей. **Wire JSON / u128:** not applicable as a new peer-wire hazard in this slice (normative wire integers remain covered by existing helpers; RFC 14 не вводит новых числовых полей без кодирования в этом диффе).

---

## 5. Safety

- Крипто: cosign проверяется через общий `verify` на том же `signing_message()`, что и основная подпись — подделка подписи под тем же сообщением не проходит без ключа.
- Паники: в новых policy-путях не добавлены новые `unwrap` в горячих местах evaluator.
- Доверенная граница: policy ошибки проходят через стабильные коды — хорошо для наблюдаемости.

---

## 6. Tests

По репозиторию фильтр имён вроде `policy_*` даёт **11** тестов (включая `policy_tx_json_fee_str`, `policy_signing_changes_by_action` в `tx.rs` и `policy_v2_gates_with_season` в `chain.rs`), что согласуется с заметкой тикета.

Покрыто хорошо: cross-domain routing deny без мутаций, finalized sender, default_behavior incoming deny, cosign missing / bad sig, совпадение ошибки precheck и apply, точечная проверка `evaluate_policy` для sender filter.

Не покрыто автоматически (не блокер V4-3, но для backlog): матрица «preflight vs apply» для **каждого** кода `E_POLICY_*` через HTTP-форму JSON; отдельные тесты на роль cosign; позитивный сценарий для «allow» default behavior, когда он появится в модели.

**Независимый прогон:** в рамках этого ревью `cargo test` не выполнялся; принято по tracing в тикете (PASS после доработки тестов).

---

## 7. Verdict

**PASS** (актуально после re-review и уточнения документов 2026-05-17).

Первоначальный **PASS_WITH_NITS** опирался на пробел нормативного текста про минимальную семантику. Сейчас это закрыто:

- `docs/rfc/6-policy-engine.md` **§10.1 MVP V4-3 minimal semantics** фиксирует placeholder для `sender_filter`, default-deny для `default_behavior` на входящий `TRANSFER`, generic cosign только по canonical intent без registry binding, и границу V4-4 для emergency/rescue.
- `docs/plans/mvp_v4.md` параграф **V4-3 semantic boundary** дублирует те же ограничения в дорожной карте.

Соответственно nit’ы уровня «спека не совпадает с кодом» по этим трём пунктам сняты. Остаются только прежние **low** наблюдения (stub до policy на `Init`, политики только для `initialized` получателя, временный `Redirect` → `PolicyDenied`), которые специально не регулируются новым текстом и не блокируют закрытие V4-3.

Микро-замечание по докам: в Sprint V4-3 **Scope** в `mvp_v4.md` всё ещё фигурирует формулировка «default reject/allow behavior»; её имеет смысл выровнять с boundary («default-deny only»), чтобы читатель плана не споткнулся.

---

## 8. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/20260517-v4-sprint3-policy-engine-review.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 3800,
    "confidence": "low"
  }
}
```

(Оценка относится к циклу re-review; первичный проход ревью ~9500 токенов.)

---

## 9. Рекомендации тикету / докам

- Тикет: проставить `artifacts.review_md`, статус slice по процессу оркестратора.
- Косметика: одна строка Scope в `mvp_v4.md` про default behavior — заменить на согласованную с boundary.
- RFC §10.1: формулировка «`routing.emergency_redirect` may influence the returned decision shape» опережает текущий код (evaluator пока не использует этот kind); допустимо как forward-looking; при желании добавить «в текущей реализации не влияет» для полного совпадения с кодом.

---

**Glossary:** не финальное ревью спринта V4 целиком — `docs/GLOSSARY.md` не менялся.
