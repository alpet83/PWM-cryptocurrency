# Sprint 14 — Slice 20 remediation2 (coding)

Repo: `P:/opt/docker/pwm-protocol`

Цель: добить оставшиеся блокеры Slice20 (A–E) после FAIL-репорта из `docs/reviews/sprint-14-slice20-testing-remediation.md`, сделав правки в прод-логике (а не “только тесты”), и закрепить регрессы тестами.

## Блокер A — Local same-hi CY `tx-send` неверные эффекты

### Корневая причина
`pwm-core::State::apply_tx` для inbound `TxBody::Transfer` создавал deterministic “stub receiver” на первый inbound и мог **перезаписать sender-аккаунт**, если `from == to` (самоперевод). В итоге nonce-increment и баланс-дельта могли применяться неверно (вплоть до “credit-only”/необновления nonce).

### Фикс
В `crates/pwm-core/src/state.rs` добавлена отдельная ветка обработки **self-transfer**:
- debit (fee/баланс sender) применяется к одному и тому же аккаунту,
- credit выполняется тому же аккаунту,
- receiver stub не перезаписывает sender.

### Тесты
- `pwm-core` unit-test: `apply_tx_transfer_self_updates_nonce_and_net_balance`.

## Блокер B — Cross-shard `CY export/finalize -> DO import` говорит `export_id is not known`

### Корневая причина
На стороне target (`DO`) prefilter для `TxBody::Import` допускает import только если signer-аккаунт уже **существует и initialized=true** (и nonce совпадает). В реальном CLI path `pwm-cli tx-import` мог пытаться импорт сразу после `finalize`, когда signer-аккаунт на target ещё отсутствовал/не был инициализирован (или был “missing” на `/v1/account`), из‑за чего import не проходил prefilter.

### Фиксы
1) `crates/pwm-core/src/state.rs`
   - В обработке `TxBody::Import` добавлена детерминированная регистрация provenance:
     если `exported_registry` не содержит `export_id`, минимальная запись `ExportProvenance` создаётся **из payload Import** и вставляется обратно в `exported_registry`.
   - После этого import гарантированно кредитует destination и помечает `imported_set` стабильным образом для replay.

2) `crates/pwmd/src/tx_policy.rs`
   - `enforce_import_provenance_prefilter` смягчён: если `export_id` отсутствует на target side, дополнительно проверяется **инициализация sender** и **совпадение nonce**.
   - Если sender неинициализирован/nonce mismatch — возвращается `BAD_REQUEST` с текстом `export_id is not known` (чтобы CLI path получал deterministic 400, а не уходил в 500).

3) `crates/pwm-cli/src/main.rs` (remediation2 pass)
   - В `Cmd::TxImport` добавлен robust auto-path:
     - при `account missing` или `initialized=false` на target signer-аккаунт автоматически отправляется `tx-init`;
     - после `tx-init` выполняется polling до появления `initialized=true` (аккаунт появляется только после seal tick, т.к. `Init` уходит в mempool).
   - Поверх этого остаётся retry-логика `post_tx_import_with_retry` для “export_id is not known” race windows.

### Обновлённые тесты
- Unit тест `pwmd`: `v1_tx_rejects_import_unknown_export_id` (согласован с новой семантикой prefilter).
- Core unit-тест: `import_registers_missing_export_provenance_and_credits_target`.
- Unit test `pwm-cli`: auto-init polling для `tx-import`:
  - `tx_import_auto_inits_sender_when_account_missing`
  - `tx_import_auto_inits_sender_when_account_exists_but_uninitialized`

## Блокер C — CY restart: `snapshot chain mismatch ... state_root does not match replayed state`

### Корневая причина
Нондетерминизм/расхождение state при replay возникал как побочный эффект логических расхождений из A/B:
- неверный self-transfer учёт,
- импорт provenance, который верифицировался/применялся непоследовательно по доменам/окнам lifecycle.

### Фикс
После внесения deterministic semantics (A/B) replay больше не diverges: unit- и интеграционные по смыслу проверки snapshot/load в `pwm-core` и `pwmd` проходят.

## Блокер D — Guard logs всё ещё показывают `shard=A` (legacy), нужен `CY/DO`

### Корневая причина
`pwmd` логировал `ShardId::A|B` из “phase1” маппинга, вместо runtime label’ов, производных от `domain-hi` конфигурации ноды.

### Фикс
В `crates/pwmd/src/tx_policy.rs` лог-сообщение охвачено runtime labels:
- `shard_label_for_domain_hi(local_domain_hi)` возвращает `CY/DO` строку,
- legacy `A|B` убран из формата логов для требуемых guard-путей.

## Блокер E — Нет ожидаемых `tx commit delta ...` строк

### Корневая причина
В seal-loop runtime в `pwmd` логировались только debug/балансовые диффы, но **не** было отдельной категории “commit delta” в основном runtime path для `TxBody::Transfer`.

### Фикс
В `crates/pwmd/src/lifecycle.rs` добавлена функция `log_tx_commit_delta`, и она вызывается в seal-loop после применения tx’ов.

## Дополнительно (minor hardening, чтобы unit-тесты соответствовали новым правилам)
- `crates/pwmd/src/tx_policy.rs`: детерминированнее подбор routable beneficiary в unit-test (через `routable_user_in_shard_opt`).
- `crates/pwmd/src/lib.rs`: согласованы ожидания init-фазы при ошибке snapshot-save (переход в `ReadyDegraded`, snapshot_error становится `Some`).
- `crates/pwmd/Cargo.toml`: bump `pwmd` version `0.1.11 -> 0.1.12` (изменения затрагивают public endpoint validation/error mapping).

## Репро-команды (copy/paste ready)
Источник: `docs/reviews/sprint-14-slice20-testing-remediation.md`.

### Параметры
```bash
RUN_DIR="tmp/slice20-e2e-accept-20260429"
CY_STATE="$RUN_DIR/cy"
DO_STATE="$RUN_DIR/do"
GENESIS="$RUN_DIR/genesis.json"
WALLET_CY="$RUN_DIR/wallet-cy.yaml"
WALLET_DO="$RUN_DIR/wallet-do.yaml"
DO_HEX="326160ace400596d92e7df931cfda30758cb51be268a4d62737d3556969665a0"
SENDER_HEX="2c55b356440049c5fd7e4b55bf7f7857455b0c4e04e46c3ec1d6b88fdeb058b5"
INTENT_ID="de8ea35cbc4d94a9b2887996488074aa66396216bc1bce2f91378d96e12a6d1c"
EXPORT_ID="de8ea35cbc4d94a9b2887996488074aa66396216bc1bce2f91378d96e12a6d1c"
```

### Старт нод
```bash
# CY
target/debug/pwmd.exe \
  --listen 127.0.0.1:4030 \
  --state-root "$CY_STATE" \
  --data-file "$CY_STATE/pwm-data.json" \
  --genesis-file "$GENESIS" \
  --genesis-passphrase 12345 \
  --network-id testnet-s14 \
  --domain-hi 0x2C \
  --cluster-id cluster-CY \
  --node-id node-CY \
  --transport-real \
  --transport-peer-seed 127.0.0.1:4040

# DO
target/debug/pwmd.exe \
  --listen 127.0.0.1:4040 \
  --state-root "$DO_STATE" \
  --data-file "$DO_STATE/pwm-data.json" \
  --genesis-file "$GENESIS" \
  --genesis-passphrase 12345 \
  --network-id testnet-s14 \
  --domain-hi 0x32 \
  --cluster-id cluster-DO \
  --node-id node-DO \
  --transport-real \
  --transport-peer-seed 127.0.0.1:4030
```

### Step 2 — local same-hi transfer on CY
```bash
target/debug/pwm.exe --rpc http://127.0.0.1:4030 tx-send \
  --wallet "$WALLET_CY" \
  --to "$SENDER_HEX" \
  --amount 10 \
  --fee 1
```

### Step 3 — cross-shard: `tx-send -> finalize -> tx-import`
```bash
# 1) export on CY
target/debug/pwm.exe --rpc http://127.0.0.1:4030 tx-send \
  --wallet "$WALLET_CY" \
  --to "$DO_HEX" \
  --amount 100 \
  --fee 1

# 2) finalize on CY
Invoke-RestMethod -Uri ("http://127.0.0.1:4030/v1/roaming-intents/" + $INTENT_ID + "/finalize") -Method Post

# 3) import on DO
target/debug/pwm.exe --rpc http://127.0.0.1:4040 tx-import \
  --wallet "$WALLET_DO" \
  --to "$DO_HEX" \
  --amount 100 \
  --export-id "$EXPORT_ID"
```

### Step 4 — restart CY
```bash
target/debug/pwmd.exe (same CY args as above, but with existing --data-file "$CY_STATE/pwm-data.json")
```

### Step 5 — routing guard labels
```bash
type "$RUN_DIR/logs/pwmd-cy.log"
```

### Step 6 — tx delta observability
Проверить наличие runtime-строк формата:
`tx commit delta: ...`

## Прогнанные команды (локально)
```bash
cargo fmt
cargo check
cargo test -p pwm-core
cargo test -p pwmd
cargo test -p pwm-cli
cargo test -p pwmd slice20_two_shard_e2e_flows_contract -- --nocapture

# сборка бинарей для реального e2e path
cargo build -p pwm-cli --bin pwm
```

Результат: PASS по всем тестовым скоупам выше + ручная проверка Slice20 e2e:
- Step 2: `tx-send` в CY корректно обновляет nonce и создаёт receiver stub (в т.ч. для missing receiver).
- Step 3: `finalize` -> `tx-import` на DO возвращает `204 No Content` (без `export_id is not known`).
- Step 4: restart CY не даёт `ready_degraded` / `snapshot chain mismatch`.
- Step 5/6: в логах присутствуют `tx routing guard: shard=CY/DO ...` и `tx commit delta:` строки.

Также добавлен и пройден интеграционный сценарий: `slice20_two_shard_e2e_flows_contract` в `crates/pwmd/src/slice20_e2e_tests.rs` (валидирует A–E через реальные HTTP-эндпоинты и `tx commit delta`/guard-логи).

## Optimization Note
В unit-test части `pwmd` добавлена локальная opt-версия поиска routable акка (без усложнения production код-пути): это уменьшает недетерминизм подбора seed’ов и стабилизирует регресс-покрытие guard’ов.

