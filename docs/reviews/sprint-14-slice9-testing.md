# Sprint 14 — Slice 9 Testing

Дата: 2026-04-28
Репозиторий: `P:/opt/docker/PWM-cryptocurrency`

## Scope

Проверены пункты Slice 9:
1. rename `detect_resume_der_index` и сохранение поведения;
2. `pwm-cli genesis-build` из wallet (включая encrypted path);
3. dual loader в `pwmd` для v2 hex + legacy форматов;
4. выравнивание docs по `GENESIS_BLOCK` (основной путь без `addr-derive`).

## Commands and results

### 1) Rename + behavior unchanged (`detect_resume_der_index`)

```powershell
rg -n "load_resume_index_domain|detect_resume_der_index" crates/pwm-cli/src
cargo test -p pwm-cli wallet::tests::detect_resume_der_index_uses_max_matching_index -- --exact
cargo test -p pwm-cli wallet::tests::detect_resume_der_index_prefers_matching_cluster -- --exact
```

Результат:
- В коде присутствует только `detect_resume_der_index` (старый символ не найден).
- Оба таргетных теста прошли.

Вердикт: **PASS**.

### 2) `pwm-cli genesis-build` from wallet (encrypted path)

Таргетные тесты:

```powershell
cargo test -p pwm-cli genesis_build_cli_parses_required_flags
cargo test -p pwm-cli genesis_build_generates_rows_from_wallet_accounts
```

Smoke (реальный CLI-поток с encrypted wallet):

```powershell
cargo run -p pwm-cli -- wallet init --country CY --wallet-out .tmp-test/slice9-wallet.yaml --wallet-passphrase "slice9-pass"
cargo run -p pwm-cli -- wallet account add --wallet .tmp-test/slice9-wallet.yaml --derivation-index 1 --wallet-passphrase "slice9-pass"
cargo run -p pwm-cli -- genesis-build --wallet .tmp-test/slice9-wallet.yaml --out .tmp-test/slice9-genesis.json --wallet-passphrase "slice9-pass"
```

Проверка артефакта:
- `.tmp-test/slice9-genesis.json` содержит:
  - `schema_version = 2`;
  - `gen_cfg.rows[*].acct_hex/pubkey_hex/der_idx/bal`;
  - `validator_seeds_hex`;
  - `genesis_rows 2` в stdout команды.

Вердикт: **PASS**.

### 3) `pwmd` dual loader: v2 hex + legacy

```powershell
cargo test -p pwmd genesis_json_v2_roundtrip_hex_fields
cargo test -p pwmd genesis_json_roundtrip_dev_seed
cargo test -p pwmd genesis_json_v2_rejects_invalid_hex
cargo test -p pwmd genesis_json_v2_rejects_seed_row_len_mismatch
```

Результат:
- v2 hex roundtrip — pass;
- legacy roundtrip (`dev_seed`) — pass;
- negative-case проверки v2 (`invalid hex`, `seed/row len mismatch`) — pass.

Вердикт: **PASS**.

### 4) Docs alignment (`GENESIS_BLOCK` main path)

```powershell
rg -n "addr-derive" docs/GENESIS_BLOCK.md
```

Результат:
- Основной workflow в документе построен вокруг `wallet init` + `genesis-build` + `pwmd --genesis-file`.
- `addr-derive` встречается только как предупреждение/guardrail в операторском разделе, не как шаг основного сценария.

Вердикт: **PASS**.

## Additional suite runs

```powershell
cargo test -p pwm-cli
cargo test -p pwmd
```

Итог:
- `pwm-cli`: **127 passed, 0 failed**.
- `pwmd`: **89 passed, 3 failed**:
  - `tx_policy::tests::export_guard_rejects_policy_invalid_recipient`
  - `tx_policy::tests::burn_mark_guard_rejects_policy_invalid_beneficiary`
  - `tx_policy::tests::burn_mark_guard_allows_same_shard_beneficiary`

Примечание: падения находятся вне прямого Slice 9 scope (genesis-build/dual-loader), но зафиксированы как текущий риск состояния ветки.

## Final verdict

По Slice 9 целевые проверки пройдены: **4/4 PASS**.

`pwm-cli` и таргетные `pwmd` тесты для genesis-пути зелёные; полный `pwmd` suite не полностью зелёный из-за 3 несвязанных `tx_policy` падений.
