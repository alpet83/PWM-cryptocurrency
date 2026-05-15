# `pwm-core`: техническая документация

Крейт `pwm-core` содержит минимальное ядро MVP-цепочки PWM: типы аккаунтов и транзакций, применение транзакций к состоянию, сборку/подпись PoA-блока, базовый mempool и оффчейн-заглушку для batch-пакетов.

## Назначение и границы

**Назначение**
- единая бизнес-логика ledger/tx для `pwmd`, `pwm-cli`, `pwm-tui`;
- детерминированные правила состояния (`state`) и хэш-корней (`digest`, `txs_root`);
- минимальный devnet-консенсус PoA (ротация продюсера по индексу).

**Что входит в крейт**
- структуры `Account`, `SignedTx`, `BlockHdr`, `Block`, `Chain`, `Mpool`;
- проверка формы транзакции (`validate_tx_shape`) и подписи;
- применение tx к `State`, claim/burn-операции по единому `Account.marks` и награда продюсеру блока (`reward_producer`);
- фабрика genesis (`dev_net`, `GenCfg`) и HD-деривация (`m/0'/i`);
- бинарные/JSON-friendly сериализационные хелперы для фиксированных массивов.

**Что не входит**
- p2p/сетевой слой, RPC, HTTP, хранение на диске (это `pwmd`);
- полноценный mempool-планировщик (есть FIFO-очередь с cap);
- on-chain bridge для оффчейн batch (в `offchain` только крипто-примитивы/stub);
- BFT/финализация/сложный выбор валидатора (только простой PoA dev-режим).

## Карта модулей

- `block` — `BlockHdr`/`Block`, `sign_preimage`, подпись/верификация заголовка, `txs_root`, `hdr_hash`.
- `chain` — `Chain::boot`, `Chain::seal`, связь блоков через `prev_hash`, ротация PoA-продюсера, тип ошибки `SealAbort`.
- `state` — `State`, `apply_tx`, `digest`, claim/burn-логика по `Account.marks` и награда продюсеру.
- `tx` — `TxBody`, `SignedTx`, `signing_message`, `tx_hash`, `verify_sig`, `validate_tx_shape`, `TxError`.
- `genesis` — `GRow`, `GenCfg`, `state0`, `prod_acct`, dev-конфиг `dev_net()`.
- `hd` — `account_id_from_parts`, `domain_of_account_id`, `brute_cluster_address`.
- `mempool` — `Mpool` FIFO (`push`/`take`/`prepend_block`) с ограничением ёмкости.
- `crypto` — `blake3_32`, Ed25519 `sign`/`verify`, `hash_header_signing_payload`.
- `ser_bin` — serde-хелперы фиксированных массивов (`sig64`) для JSON/bincode.
- `offchain` — `merkle_root`, `batch_preimage`, `sign_batch` (оффчейн stub).
- `types` — `AccountId`, `Account`, `Account::genesis_funded`.

## Базовый поток данных

## 1) Применение транзакции (`State::apply_tx`)

1. Вход: `SignedTx`.
2. `validate_tx_shape`:
   - подпись Ed25519 валидна для `signing_message`;
   - `domain_code` совпадает с доменом, вычисленным из `computed_account_id()`.
3. Вычисляется `account_id` отправителя (`pk + derivation_index` -> `blake3`).
4. Особый случай `TxBody::Init`:
   - если записи ещё нет, создаётся stub-аккаунт (нулевые балансы, нужные `pk/index`).
5. Базовые проверки состояния:
   - аккаунт существует;
   - `signing_pubkey`/`derivation_index` совпадают с tx;
   - `nonce` tx равен `Account.nonce`.
6. Ветка `TxBody`:
   - `Init`: флаг инициализации, запись `index/flags`, `nonce += 1`;
   - `Transfer`: проверка `initialized`, `amount + fee`, списание sender, пополнение `to`, `fee_pool += fee`;
   - `Stake`/`Unstake`: перенос между `balance_pwm` и `staked`, `nonce += 1`;
   - `BurnMark`: уменьшение `marks`, `nonce += 1`.
   - В состоянии нет отдельного `marks_quota`: единственный консенсусный источник марок — `Account.marks`.
7. На успехе изменённый `State` фиксируется; на ошибке возвращается `TxError`.

## 2) Запечатывание блока (`Chain::seal`)

1. Вход: пакет `Vec<SignedTx>` (обычно из `Mpool::take`).
2. Определяется:
   - `height = tip + 1`;
   - `prev_hash` из tip или `prev_gen()` для первого блока;
   - `prod_idx` = `(height - 1) % validators`.
3. Клонируется `State` и последовательно применяется каждый tx:
   - любая ошибка -> `Err((msg, txs))` (`SealAbort`), чтобы caller мог вернуть tx в пул (`prepend_block`).
4. После успешного применения `reward_producer(block_reward)` начисляет награду продюсеру. Per-block `accrue_marks` в seal-пути удалён; марочный UX v2 опирается на genesis/claim-контур и единый `Account.marks`.
5. Считаются `state_root` и `tx_root`, собирается `BlockHdr`.
6. Заголовок подписывается валидатором `val_sks[prod_idx]`; подпись проверяется по pubkey из genesis-строки.
7. На успехе: `self.st = st`, блок добавляется в `self.blocks`.

## Ключевые инварианты и допущения

- `Chain::boot`: число `val_sks` равно числу `GenCfg.rows`.
- `AccountId` детерминирован от (`signer_pk`, `derivation_index`), домен = первые 2 байта (`u16` big-endian).
- Tx проходит shape-check до state-логики: неверная подпись/домен отбрасываются сразу.
- `nonce` строгий, монотонный (`==` ожидаемому значению на момент применения).
- `Transfer` возможен только между существующими и `initialized` аккаунтами.
- `Init` разрешён один раз; для новых аккаунтов сначала создаётся stub.
- `Chain::seal` атомарен: частично применённое состояние не коммитится.
- `txs_root`/`merkle_root` имеют фиксированные empty-константы (`PWMv0/EMPTYTX`, `PWMv0/OFFEMPTY`).
- Крипто-домены префиксованы (`PWMv0/TX`, `PWMv0/BLOCKHDR`, `PWMv0-OFFBATCH`) для domain separation.

## Карта тестов (текущее покрытие)

**В `pwm-core`**
- `hd::tests`
  - `brute_finds_i0_for_derived_domain`: проверка перебора индекса для домена.
- `state::tests`
  - `apply_tx_init_then_transfer_happy_path`;
  - `apply_tx_rejects_bad_nonce`;
  - `apply_tx_rejects_insufficient_balance_on_transfer`;
  - `validate_tx_shape_rejects_domain_mismatch`.
- `chain::tests`
  - `seal_empty_block`;
  - `seal_returns_txs_on_apply_error` (возврат tx при abort).
- `mempool::tests`
  - `prepend_block_restores_fifo_order`;
  - `seal_fail_then_prepend_keeps_len`.

**Смежное smoke-покрытие**
- В `crates/pwmd` есть интеграционные/oneshot-сценарии, которые косвенно проходят путь `POST /v1/tx` -> mempool -> `Chain::seal`.

## Точки расширения

- `TxBody`: добавление новых типов tx с веткой в `State::apply_tx`.
- `validate_tx_shape`: усиление pre-check (например, лимиты полей/политики).
- `GenCfg`: больше валидаторов, внешние genesis-конфиги, изменяемые `block_reward` и параметры genesis/claim-политики.
- `Mpool`: приоритеты, дедупликация, TTL, anti-spam.
- `Chain`: иная политика выбора продюсера, pre/post-block hooks, дополнительные проверки заголовка.
- `offchain`: переход от stub к on-chain верификации batch/bridge.

## Известные ограничения MVP

- Нет персистентного state/chain в самом `pwm-core` (только in-memory структуры).
- Нет параллельного исполнения tx и конфликт-менеджмента.
- Нет комиссии/цены газа как отдельной экономической модели (только `fee_pool`).
- Нет reorg/fork-choice и сетевого консенсуса; PoA ротация простая и детерминированная.
- Нет встроенной anti-replay между сетями, кроме доменной части `AccountId` и префиксов сообщений.
- `offchain` не интегрирован в on-chain состояние (только крипто-утилиты для demo).
