# Слайс R — чеклист

План: [sprint-15-slice-R-plan.md](sprint-15-slice-R-plan.md).

## R.1 `pwm-core`

- [x] `parse_acct_id_for_user` + док-комментарий о прежнем имени
- [x] `crates/pwm-core/src/lib.rs` — `pub use`
- [x] Потребители: `pwm-cli` (`address_book`, `cmd_genesis`, `cli_parse`), `pwm-tui` (`send_form`)
- [x] Юнит-тесты в `types.rs`

## R.2 `pwmd`

- [x] `app_from_dev_net_shard` (было `app_from_dev_net_in_shard`)
- [x] `app_from_genesis_data_shard` (было `app_from_genesis_with_data_in_shard`)
- [x] `app_from_genesis_shard_identity` (было `app_from_genesis_in_shard_with_identity`)
- [x] `lib.rs`, `lifecycle.rs`, тестовые модули под `crates/pwmd/src/tests/`

## R.3 Внутренние прод-`fn` (≥6 сегментов → ≤5)

- [x] `pwm-core` `wallet_read.rs` / `pwm-cli` `wallet/store.rs`: `parse_derivation_index_from_m0_path` → **`parse_der_idx_m0_path`**
- [x] `pwmd` `transport_tick.rs`: `enqueue_seed_by_last_peer_class` → **`enqueue_seed_peer_cls`**
- [x] `pwm-tui` `roaming.rs`: `post_import_tx_via_source_relay` → **`post_imp_tx_src_relay`**
- [x] `pwm-tui` `tx_submit.rs`: `looks_like_cross_domain_transfer_reject` → **`is_xdom_xfer_reject_body`**

## Проверки

- [x] `cargo fmt --all`
- [x] `cargo test --workspace`
- [x] Контрольный grep по дереву `*.rs`: `\bfn [a-z][a-z0-9]*(?:_[a-z0-9]+){5,}\s*\(` — **0 совпадений**

## Конвейер

После правок кода: **pwm-testing** → при необходимости доп. grep по длинным `pub fn` → **pwm-review** → запись в тикете `tasks/` и ссылка из [sprint-15-slice-O-checklist.md](sprint-15-slice-O-checklist.md).
