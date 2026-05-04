# Слайс R — продакшен-идентификаторы ≤ 5 сегментов (`snake_case`)

Цель: закрыть хвост переименований после слайса **N** (там были преимущественно **`#[cfg(test)]`**). Здесь — **`pub` / `pub(crate)` и прод-пути**, без изменения семантики API кроме имён.

Правило сегментов: число частей между `_` в имени функции ≤ 5 (как в `docs/AGENT_PROMPT_coding.md`).

## Волны

| Волна | Область | Действие |
|-------|---------|----------|
| **R.1** | `pwm-core` | `parse_account_id_for_user_input` → **`parse_acct_id_for_user`** (в док-комментарии указано прежнее имя). Обновить `pub use`, CLI/TUI импорты. |
| **R.2** | `pwmd` bootstrap | Три имени с 6–8 сегментами → короче при сохранении префикса `app_from_*`: **`app_from_dev_net_shard`**, **`app_from_genesis_data_shard`**, **`app_from_genesis_shard_identity`** (`pub(crate)` для последнего). Реэкспорт в `lib.rs`, `lifecycle`, тесты `pwmd`. |
| **R.3** | `pwm-core` / `pwm-cli` / `pwmd` transport / `pwm-tui` | Приватные прод-`fn` с **≥6** сегментами: `parse_der_idx_m0_path`, `enqueue_seed_peer_cls`, `post_imp_tx_src_relay`, `is_xdom_xfer_reject_body` (см. ревью). |

## Приёмка

- `cargo fmt --all`
- `cargo test --workspace`
- При необходимости внешним потребителям — замена имён по таблице выше (breaking для ранее экспортированных символов).
