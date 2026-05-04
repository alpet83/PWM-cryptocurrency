# Genesis Validator Key Roles Review — 2026-04-28

Источник проверки: `crates/pwm-core/src/genesis.rs`, `crates/pwmd/src/snapshot.rs`, `crates/pwmd/src/bootstrap.rs`, `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/tx.rs`, `crates/pwmd/src/tx_policy.rs`, `crates/pwmd/src/identity.rs`, `crates/pwmd/src/transport.rs`.

## Executive summary

- Исторически в legacy flow использовался derivation `[0,0]` (эквивалент `m/0'/0'`), но в текущем контракте genesis используется фиксированный путь `m/1000000'/1'` для **validator signing key**.
- Это **не делает validator account id “рандомным”**: он детерминирован от `validator seed + derivation path + row.der_idx` и дополнительно проверяется на согласованность при загрузке genesis.
- В текущем дизайне regular tx/premine spendability контролируются записью аккаунта в state (`signing_pubkey + derivation_index + nonce`), а не “ролью валидатора” как таковой.
- Ограничения `domain_hi / shard` применяются на локальном tx API пути и завязаны на runtime identity узла, а не на key material валидатора.
- Практически имеет смысл рассмотреть опцию конфигурируемого validator derivation index (с валидацией), но минимумом уже сейчас должны быть явные docs guardrails по разделению ролей.

## Current behavior with precise code references

- Историческая заметка: legacy `dev_net()` использовал `[0,0]` (`m/0'/0'`) для вектора по умолчанию.
- В активном v4 genesis flow (`pwmd --genesis-file`) validator key contract фиксирован на `m/1000000'/1'`, а загрузка проверяет консистентность derived `pubkey/acct` с validator row.
- Account id вычисляется из `pubkey + der_idx` через `account_id_from_parts`; это делается и в genesis, и в runtime проверках (`crates/pwm-core/src/genesis.rs`, `crates/pwmd/src/snapshot.rs`).
- В `State::apply_tx()` право тратить баланс определяется совпадением `tx.signer_pk` и `tx.derivation_index` с записью аккаунта, плюс `nonce` (`crates/pwm-core/src/state.rs`). То есть контроль средств — это ключ/индекс аккаунта, а не “статус валидатора”.
- Local tx guards проверяют соответствие sender domain_hi текущему runtime identity узла (`enforce_local_tx_guards`) и shard policy (`crates/pwmd/src/tx_policy.rs`; вызов из `crates/pwmd/src/api.rs`).
- Runtime identity (`cluster_domain_hi`, `cluster_id`, `node_id`) задаёт сетевую/операционную идентичность узла и используется в handshake/peer-classification, отдельно от genesis validator key (`crates/pwmd/src/identity.rs`, `crates/pwmd/src/transport.rs`).

## Risk analysis (operational + security)

### Operational

- **Жёсткий `m/1000000'/1'` для validator seeds** снижает гибкость операций: если в вашей операционной модели валидаторский ключ должен лежать на другом пути, это нельзя выразить нативно в genesis bundle.
- Это может провоцировать ручные обходы (перегенерация seed/подмена ролей), что повышает риск ошибок при развёртывании кластеров/доменов.
- Возможна путаница ролей у операторов: “validator key”, “premine owner”, “node runtime identity” сейчас логически разделены, но без явной документации это легко спутать.

### Security

- Фиксация на `m/1000000'/1'` сама по себе **не даёт явного криптографического ослабления** (если seed уникален и защищён), но увеличивает вероятность human/ops misconfiguration.
- Текущие runtime tx-ограничения (domain/shard) не завязаны на validator key; это хорошо с точки зрения разделения полномочий, но требует документирования.
- Проверка согласованности `seed -> pubkey -> acct` в `load_genesis_bundle()` снижает риск тихой рассинхронизации genesis данных.

## Recommendation options

### A) Keep fixed validator path + docs guardrails

- Оставить текущую фиксированную модель `m/1000000'/1'` для validator seeds.
- Добавить в docs явные правила:
  - validator key derivation фиксирован на `m/1000000'/1'` в genesis flow;
  - validator key роль — block production/signing, а не отдельный режим контроля user tx;
  - spendability premine определяется аккаунтной записью (`signing_pubkey + derivation_index + nonce`) и policy API/runtime identity.
- Плюс: минимум изменений, низкий риск регрессии.
- Минус: операционная негибкость.

### B) Configurable validator derivation index + validation

- В genesis bundle добавить явный validator derivation index/path (или использовать `row.der_idx` как индекс derivation для seed-derive).
- При загрузке валидировать:
  - длины и соответствия rows/seeds/indices;
  - детерминированное совпадение `derived pk` и `derived account id` с `gen_cfg.rows[i]`.
- Плюс: лучше для operational reliability, меньше ручных обходов.
- Минус: больше сложности и тестового покрытия.

## Clear answers to user concerns

1. **Да, в текущем typical genesis flow validator key жёстко привязан к `m/1000000'/1'`**.  
   **Но validator account id не “случайный”** — он детерминирован и проверяется на консистентность с `gen_cfg.rows[i].acct`.

2. **Да, это можно и обычно стоит ослабить** до настраиваемого validator derivation index/path с жёсткой валидацией.

3. **Если не ослаблять**, docs должны явно сказать, что:
   - validator key не является “мастер-переключателем” regular tx flow;
   - ограничения расхода идут через account/tx checks и runtime domain/shard guards;
   - domain/cluster блоки касаются tx-пути и runtime identity, а не “факта валидаторского ключа”.

4. **Роли в typical genesis bundle:**
   - **Validator seed/key**: источник signing key для block producer (сейчас derive через `m/1000000'/1'`).
   - **Validator account row (`gen_cfg.rows`)**: on-chain запись (`acct`, `pubkey`, `der_idx`, `bal`), формирует state0 и владельца premine.
   - **Premine balance owner**: конкретный account id в `rows[*].acct`; тратит по обычным правилам подписи/nonce.
   - **Node runtime identity (`domain_hi/cluster/node`)**: сетевой/операционный контур узла (handshake, local tx guards, routing policy), отдельно от genesis validator key.

## Verdict

**approve with nits** — критической уязвимости в текущей логике не выявлено, но нужны явные docs guardrails; опция конфигурируемого validator derivation index обоснована для повышения operational reliability.

## Option 1 accepted

Принят вариант **A) Keep fixed index 0 + docs guardrails** как текущий рабочий шаг.

Документация обновлена в:

- `docs/GENESIS_BLOCK.md` (раздел `Validator key roles (operator guide)` + pre-launch checklist);
- `docs/pwmd.md` (краткая cross-reference note);
- `docs/pwm-cli.md` (краткая cross-reference note).
