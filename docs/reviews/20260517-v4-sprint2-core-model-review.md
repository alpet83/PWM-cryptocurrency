# Review: MVP V4-2 core policy model slice (`20260517-v4-sprint2-core-model`)

## Scope recap

- **Тикет:** `tasks/20260517-v4-sprint2-core-model.json` — минимальная consensus-модель V4: `TxBody::Policy`, `PolicyAction` / `PolicyKind`, расширение INIT V4, поля политики на `Account`, serde/signing, конвертация снимка, маппинг отклонений к RFC 14.
- **План:** `docs/plans/mvp_v4.md` — Sprint V4-2 (core data model and serialization), без полноценного evaluator и emergency routing.
- **RFC:** 6 (policy engine baseline), 7 (`PolicyTx`, INIT, правило JSON для `fee`), 14 (коды `E_POLICY_*`, `tx_kind`).

## Requirements fit

**Соответствует заявленному V4-2:**

- Отдельный перенос политики не через self-transfer: вариант `TxBody::Policy` с вложенным `PolicyAction`, таргет совпадает с подписантом на уровне `validate_tx_shape`.
- Компактные enum-ы `PolicyKind` / `PolicyAction` / `ActivationMode`, без VM, DSL и отдельного типа транзакции на каждую политику.
- Модель счёта: битовые маски `u16`, идентификаторы политик 0–4; переполнения битовой шкалы как класса риска нет при текущем наборе из пяти политик.
- INIT без `init_v4` сохраняется; валидация расширения только при наличии поля.
- Поведение apply для политик ограничено установкой битов и реверсибельностью `routing.emergency_redirect` через `PolicyIrreversible`; нет полной emergency-активации с cosign — укладывается в «не V4-4».

**Зазоры / частичное покрытие:**

- _(Исторически, до follow-up):_ в снимке терялись `cosigns`; **закрыто** — см. Final addendum и тест `tx_cosigns_rt_v2_wire`.
- `finalized` блокирует только ветку `TxBody::Policy`; остальные операции ещё не завязаны на финализацию — ожидаемо до V4-4 (**остаточный продуктовый нит**, не блокер V4-2; см. Final addendum).

## Style and module shape

- В затронутых путях **`python scripts/check_entity_name_segments.py`** на перечисленные файлы: нарушений политики имён (prod ≤4, test ≤5) **нет**.
- Модули с нетривиальной логикой имеют краткие английские `//!` там, где просмотрено (`tx.rs`, `state.rs`, `snapshot/io.rs`, `lifecycle.rs`, `api/common.rs`).
- Транспортный semver / `PeerWireMsg` в этом слайсе не менялись.

### Wire JSON / u128

- **Scope:** слайс затрагивает **публичный JSON-транспорт транзакций** (`SignedTx` / `TxBody` с `serde_json`) и **дисковый формат снимка v2** (строковые decimal для сумм и `PolicyTx.fee`), а не peer wire кадры `PeerWireMsg`.
- **`PolicyTx.fee` (u128):** в `TxBody::Policy` задано `#[serde(with = "crate::ser_json_u128")]` — согласовано с RFC 7 и существующим паттерном для `Transfer` / `Claim`; тест `policy_tx_json_fee_str` фиксирует decimal string.
- **Снимок v2:** для policy в `body_to_v2` / `body_from_v2` поле `fee` сериализуется как **decimal string** через `dec_of` / `dec_v2` — для JSON на диске безопасно для u128.
- **RFC 7:** нормативное предложение про decimal string для `PolicyTx.fee` присутствует; противоречий с кодом не видно.
- **Peer catch-up / framed JSON:** изменений в `crates/pwmd/src/transport/**` в составе заявленного слайса нет; отдельная проверка peer-payload не применялась.

## Safety

- **Паники:** в горячем пути применения транзакций явных новых `unwrap` в просмотренных фрагментах нет; `digest(st)` по-прежнему использует `expect` на bincode (наследие).
- **Обход политик как «evaluator»:** отдельного обхода consensus-гейтов не видно; ошибки политик на уровне схемы маппятся в RFC 14 в `tx_err_wire`.
- **Критично — снимок и replay (исторический блокер, исправлен):** первоначально **`Account.free_claim_utc_day`** терялось при конвертации снимка — см. **Addendum** и тест `acct_free_claim_day_rt`; актуальный код переносит поле.
- **`CosignPolicy.min_signers`:** в `validate_init_v4_ext` диапазон не ограничен — заниженный/завышенный байт дойдёт до состояния до отдельной бизнес-логики (низкий риск в V4-2, но заготовка для строгой схемы).

## Tests

- **Позитив:** в `pwm-core` есть тесты JSON/signing для `PolicyTx`, INIT V4, сценарий `policy_tx_state_lifecycle` в `state.rs`.
- ** pwm-testing (по тикету):** таргетированные тесты и `cargo check`, полный `cargo test` по пакетам не гонялся — приемлемо для отчёта субагента, но **не покрыло** бы регресс снимка по `free_claim_utc_day` без отдельного теста round-trip снимка после free-claim.
- **Рекомендация:** тест «free claim → seal/snapshot save/load → повторный free claim в тот же UTC day должен отвергаться» (или сравнение `digest(state)` до/после round-trip через v2).

## Findings (приоритет) — исторический снимок первого раунда

1. ~~**[Блокирующее]** Потеря `free_claim_utc_day`~~ — **исправлено** (`acct_free_claim_day_rt`).
2. ~~**[Среднее]** `tx_from_v2` и `cosigns`~~ — **исправлено** (`tx_cosigns_rt_v2_wire`).
3. ~~**[Низкое]** Info-map vs v3 bump~~ — **согласовано** (additive v2 в info-map).
4. **[Низкое, остаётся]** Частичное применение `finalized` только к `Policy` — до V4-4 / документирования для операторов.

## Verdict

> **Update:** актуальный итог после закрытия нитов (cosigns, info-map) — **`PASS`**, см. **Final addendum** в конце файла. Ранее: блокер снимка → `PASS_WITH_NITS` после `free_claim_utc_day`.

**REQUEST_CHANGES** _(исторический, до фикса `free_claim_utc_day`)_ — из-за потери `free_claim_utc_day` на пути снимка (нарушение детерминизма replay и заявленного snapshot gate). Остальная модель V4-2 и simplicity gate выглядят согласованно с ограничением слайса.

---

## Participation / token estimate

```json
{
  "agent": "pwm-review",
  "verdict": "REQUEST_CHANGES",
  "result_suggestions": {
    "review_verdict": "REQUEST_CHANGES",
    "task_json_enum_if_limited": "FAIL_or_blocked_until_snapshot_fix"
  },
  "artifacts": "docs/reviews/20260517-v4-sprint2-core-model-review.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 12000,
    "confidence": "low"
  }
}
```

**GLOSSARY.md:** обновление не требуется (подслайсовое ревью; термины в основном уже в RFC 7/14).

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260517-v4-sprint2-core-model-review.md'
```

---

## Addendum: re-review after `free_claim_utc_day` snapshot fix (2026-05-17)

**Scope:** закрытие блокера первого раунда — сохранение `Account.free_claim_utc_day` при конвертации `Account` ↔ `SnapshotAccountWire` и в пути v2 (`account_to_v2` / `account_from_v2`).

**Проверка кода (read-only):**

- `impl From<Account> for SnapshotAccountWire` задаёт `free_claim_utc_day: value.free_claim_utc_day`.
- `impl From<SnapshotAccountWire> for Account` задаёт `free_claim_utc_day: value.free_claim_utc_day`.
- `account_to_v2` / `account_from_v2` протаскивают то же поле.

**Тест:** `snapshot::types::tests::acct_free_claim_day_rt` покрывает цепочку `Account → wire → Account → v2 → wire` с ожидаемым днём; локально `cargo test -p pwmd acct_free_claim_day_rt` — **ok** (плюс заявленные pwm-testing прогоны `cargo check -p pwmd`, `cargo test -p pwm-core --lib`).

**Регрессии в затронутых путях:** не обнаружено; хвост `..Default::default()` / `..Account::default()` после явного `free_claim_utc_day` не перезаписывает поле (явные поля имеют приоритет).

**Ранее отмечённые нити вне блокера:** потеря `cosigns` в `tx_from_v2` и расхождение «snapshot v3» в info-карте vs `SNAPSHOT_VERSION = 2` — **закрыты** в follow-up (**Final addendum**).

### Wire JSON / u128 (addendum)

Без изменений относительно первого отчёта: правка касалась `Option<u64>` на аккаунте снимка, не u128 wire.

### Verdict (supersedes 2026-05-17 blocker only)

**PASS_WITH_NITS** — блокер **снят**; последующий follow-up закрыл оставшиеся ниты (**Final addendum**).

---

### Participation / token estimate (addendum)

```json
{
  "agent": "pwm-review",
  "verdict": "PASS_WITH_NITS",
  "artifacts": "docs/reviews/20260517-v4-sprint2-core-model-review.md#addendum-re-review",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 4200,
    "confidence": "medium"
  }
}
```

---

## Final addendum: cosigns snapshot + info-map (2026-05-17, post auto-close)

**Scope:** закрытие нит из `PASS_WITH_NITS` — сериализация `SignedTx.cosigns` в формате снимка v2 и согласование формулировок info-map с additive v2.

**Код (read-only):**

- `SignedTxV2` содержит `#[serde(default)] cosigns: Vec<CosignatureV2>`; `tx_to_v2` маппит через `cosign_to_v2`, `tx_from_v2` восстанавливает через `cosign_from_v2` (hex ключей и подписи).
- Отсутствие поля / пустой вектор даёт пустые `cosigns` после десериализации (совместимость со старым JSON).

**Документация артефактов:** `tasks/20260517-v4-sprint2-core-model-info.json` — символ `SNAPSHOT_VERSION` и digest описывают **additive** расширение текущего v2, bump только при будущем wire-breaking изменении (согласуется с кодом).

**Тесты:** `tx_cosigns_rt_v2_wire` — round-trip с одной cosign-подписью и ветка «legacy» с пустым `cosigns`; локально вместе с `acct_free_claim_day_rt` — **ok**. Заявленные pwm-testing прогоны принимаются.

**Остаточный нит (продуктовый, не V4-2 blocker):** `finalized` по-прежнему ограничивает в основном ветку `Policy` в этом слайсе — ожидаемо до V4-4; к закрытию нит auto-close не относится.

### Wire JSON / u128 (final)

Без изменений: cosigns на диске — hex-строки для pk/sig; не peer `u128` wire.

### Verdict (final для оркестратора)

**PASS** — ниты закрыты; блокер снимка и follow-up по cosigns/info-map подтверждены тестами и чтением кода.

---

### Participation / token estimate (final)

```json
{
  "agent": "pwm-review",
  "verdict": "PASS",
  "artifacts": "docs/reviews/20260517-v4-sprint2-core-model-review.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 3800,
    "confidence": "medium"
  }
}
```
