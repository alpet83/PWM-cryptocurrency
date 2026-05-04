# Слайс R — финальное ревью (имена `fn` ≤ 5 сегментов)

**Область:** прод и внутренние не-тестовые `fn` в workspace после слайса **N**.

## История волн R.1–R.2

- **`pwm-core`:** `parse_acct_id_for_user` (ранее `parse_account_id_for_user_input`); в docstring указано прежнее имя.
- **`pwmd`:** `app_from_dev_net_shard`, `app_from_genesis_data_shard`, `app_from_genesis_shard_identity`; реэкспорт, `lifecycle`, тесты.

## Волна R.3 (внутренние «сверхдлинные» имена)

| Было | Стало | Файл |
|------|--------|------|
| `parse_derivation_index_from_m0_path` | `parse_der_idx_m0_path` | `pwm-core/src/wallet_read.rs`, `pwm-cli/src/wallet/store.rs` |
| `enqueue_seed_by_last_peer_class` | `enqueue_seed_peer_cls` | `pwmd/src/transport/transport_tick.rs` |
| `post_import_tx_via_source_relay` | `post_imp_tx_src_relay` | `pwm-tui/src/roaming.rs` |
| `looks_like_cross_domain_transfer_reject` | `is_xdom_xfer_reject_body` | `pwm-tui/src/tx_submit.rs` |

Поведение не менялось.

## Аудит «сверхдлинных» имён

Критерий: имя функции в `snake_case` с **не менее чем шестью** сегментами (шесть или больше групп, разделённых `_`).

Команда контроля (в корне репозитория, только Rust):

```text
rg '\bfn [a-z][a-z0-9]*(?:_[a-z0-9]+){5,}\s*\(' --glob '*.rs'
```

После R.3: **совпадений в дереве исходников нет** (не охватывает имена тестовых функций с другими паттернами; слайс **N** уже проходил по тестам).

Граница **ровно 5** сегментов (например `parse_account_id_for_migration`, `render_account_id_for_user`) **не нарушена** политикой «≤ 5».

## Риски

- **Breaking:** только для внешних вызывающих сторон по символам из R.1–R.2 (`pwm_core` / `pwmd` публичная поверхность). Символы R.3 были приватными внутри crate.

## Проверки

- `cargo fmt --all`
- `cargo test --workspace` — **PASS**

## Вердикт

**PASS (финально)** для цели слайса R: сверхдлинные прод-/внутренние имена `fn` в workspace устранены, контрольный grep даёт ноль попаданий по выбранному шаблону.
