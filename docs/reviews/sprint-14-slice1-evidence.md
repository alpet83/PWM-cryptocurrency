# Sprint 14 — Slice 1 evidence (wallet schema v3, read path)

**Конвейер:** pwm-coding → pwm-testing → pwm-review (замечания review закрыты правками в том же слайсе).

## Код

- `crates/pwm-cli/src/wallet.rs` — `WalletYamlV3*`, `parse_wallet_yaml_v3`, `detect_schema_version` (`unwrap_or(2)`), валидация path/index, plaintext id_hex vs master.
- `crates/pwm-core/src/wallet_read.rs` — v3 header parse, маппинг в `WalletReadHeader`, те же инварианты.
- `crates/pwm-core/src/types.rs` — актуализация тестов `format_domain_for_display` / pretty render под текущий `domain_index`.

## Тесты

- `cargo test -p pwm-cli` — зелёный (включая негативы v3 + новый `load_wallet_yaml_rejects_v3_derivation_path_index_mismatch`).
- `cargo test -p pwm-core` — зелёный.

## Ограничения (следующий слайс)

- `mode: encrypted` + `schema_version: 3` — явная ошибка «decrypt not implemented»; реализовать payload **A** и roundtrip.
- CLI `wallet account *`, TUI левая панель — Slice 2–3.
