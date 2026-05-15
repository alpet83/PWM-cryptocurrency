# V2-4 Slice 4 — полный аудит имён функций (`snake_case`, сегменты)

Дата: 2026-05-06. Агент: **pwm-review**. Политика: `docs/AGENT_PROMPT_coding.md`; проверка: `scripts/check_rust_fn_name_segments.py` (prod ≤ 4, test ≤ 5 сегментов по `_`).

## 1. Scope recap

Оркестратор передал список **67** нарушений по всему `crates/` (результат прогона скрипта). Задача слайса — **triаж** (MUST-FIX / DEFER / RECHECK), без правок прод-Rust-кода; исправления имён поручаются **pwm-coding**.

## 2. Методология и ограничения скрипта

- Контекст **test** выводится если: путь содержит `/tests/` или `/src/tests/`, либо объявление `fn` попадает внутрь блока **`#[cfg(test)] mod tests { ... }`** (баланс фигурных скобок), либо на строку перед `fn` висит только **`#[cfg(test)]`** (single-fn).
- **Важный пробел эвристики:** модули вида **`#[cfg(test)] mod hdr_hex_tests {`** или **`#[cfg(test)] mod trust_test_fake_genesis_tests {`** скрипт **не** считает тестовыми → на строках `fn` получается **ложный `prod`**. Для строк в таких файлах ниже указано **R** с фактическим контекстом.
- Контент функций сверян точечными чтениями исходников (узкие диапазоны строк), без обхода дерева CQDS (**skill `colloquium-cqds-mcp`** соблюдён; для уже известных `file:line` достаточно `Read`).
- Обозначения: **M** = MUST-FIX (первоочередная волна rename), **D** = DEFER (оставить имя или менять только с планом/доком), **R** = RECHECK (несоответствие скрипта контексту или граничный случай).

## 3. MUST-FIX (M) — рекомендуемая первая волна (~20 символов)

Условие: жёсткое превышение лимита при **правильном** контексте **или** однозначно лишний сегмент у публичного API с локальными call-site. Предлагаемые имена — ≤4 (prod) / ≤5 (test).

| # | Предлагаемое имя |
|---|------------------|
| 1 | `brute_force_flags_progress` *(было `brute_force_domain_flags_with_progress`)* |
| 2 | `digest_stable_same_payload` |
| 3 | `hdr_json_rt_legacy_arr` |
| 4 | `hdr_bincode_hash_stable` |
| 5 | `broke_trust_overrides_genesis` |
| 6 | `v1_rd_conflict_bridge_trust` |
| 7 | `v1_tx_underfunded_xfer_mempool` |
| 8 | `v1_tx_burn_purpose_bad` |
| 9 | `v1_tx_claim_daily_limit` |
| 10 | `v1_tx_import_fee_low` |
| 11 | `credit_min_import_fee_tests` |
| 12 | `seal_skip_ctx_block_h` |
| 13 | `peer_sink_isolated_from_main` |
| 14 | `export_rd_tip_advance_empty` |
| 15 | `mono_save_sync_disk_lag` |
| 16 | `runtime_persist_disk_lag` |
| 17 | `epoch_trust_respects_tail_cap` |
| 18 | `import_prov_tgt_facts_once` |
| 19 | `import_prov_tgt_facts_retry` |
| 20 | `mk_dev_cfg_json` |

Соответствие исходным строкам — в сводной таблице ниже (столбец «Proposal (M only)»).

## 4. Полная таблица triажа (67 записей)

| File:line | Symbol | Seg | Script kind | Cat | Proposal (M only) / примечание |
|-----------|--------|-----|---------------|-----|--------------------------------|
| pwm-cli/src/bruteforce.rs:112 | brute_force_domain_flags_with_progress | 6 | prod | M | `brute_force_flags_progress` |
| pwm-cli/src/cmd_addr.rs:145 | try_auto_init_after_bruteforce | 5 | prod | D | Семантика CLI; переименование в ту же волну опционально: `try_auto_init_post_bf` (+ `///`) |
| pwm-cli/src/cmd_roaming.rs:20 | run_tx_send_cross_domain | 5 | prod | D | Соответствует UX-термину cross-domain; док `///` + backlog |
| pwm-cli/src/cmd_roaming.rs:150 | import_provenance_from_target_facts | 5 | prod | D | Зеркало TUI/доков; лучше одна согласованная волна с `pwm-tui`: возможно `import_prov_tgt_facts` |
| pwm-cli/src/cmd_roaming.rs:360 | user_msg_roaming_intent_error | 5 | prod | D | Сообщения пользователю; низкий выигрыш от усечения без потери читаемости |
| pwm-cli/src/cmd_roaming.rs:491 | parse_export_id_hex_arg | 5 | prod | D | Второстепенно: `parse_export_id_hex` (4) при отдельном микро-пассе |
| pwm-cli/src/wallet/store.rs:189 | migrate_wallet_v2_to_v3 | 5 | prod | D | Миграция v2→v3; имя самодокументирует версии |
| pwm-cli/src/wallet/store.rs:233 | load_wallet_yaml_v3_raw | 5 | prod | D | Паритет с `save_*` / PWM-core |
| pwm-cli/src/wallet/store.rs:246 | save_wallet_yaml_v3_strict | 5 | prod | D | То же |
| pwm-cli/src/wallet/store.rs:350 | parse_der_idx_m0_path | 5 | prod | D | Дубликат логики с `pwm-core` — переименовывать только синхронно |
| pwm-cli/src/wallet/store.rs:424 | validate_v3_accounts_against_master | 5 | prod | D | Внутренний валидатор; длинное имя по смыслу |
| pwm-cli/src/wallet/store.rs:518 | account_id_from_truth_source | 5 | prod | D | Дубликат с `wallet_read`; DEFER до общего рефакторинга |
| pwm-cli/src/wallet_shell.rs:108 | wallet_regulatory_label_for_hit | 5 | prod | D | Регуляторный контекст; оставить с `///` |
| pwm-core/src/address_book.rs:117 | append_wallet_yaml_address_book | 5 | prod | D | Публичный утилитарный API; переимпорт/документирование дороже |
| pwm-core/src/block.rs:94 | hdr_json_uses_hex_strings | 5 | prod | R | Фактически **`#[cfg(test)] mod hdr_hex_tests`**. При test-бюджете **ровно 5** — **по политике допустимо**; скрипт дал prod — **ложная тревога** |
| pwm-core/src/block.rs:104 | hdr_json_roundtrip_and_legacy_byte_array | 7 | prod | R+M | Фактически test; **нужно** укоротить имя под ≤5 — см. строку **M №3** |
| pwm-core/src/block.rs:124 | hdr_bincode_hash_stable_across_codec | 6 | prod | R+M | Фактически test; см. **M №4** |
| pwm-core/src/bridge_commitment.rs:38 | digest_is_deterministic_for_same_payload | 6 | test | M | `digest_stable_same_payload` (**M №2**) |
| pwm-core/src/tx.rs:346 | burn_context_is_source_domain | 5 | prod | D | Короткий публичный предикат; имя уже «говорящее» |
| pwm-core/src/types.rs:57 | parse_acct_id_for_user | 5 | prod | D | Вторичная волна: возможно `parse_acct_id_user` (4) |
| pwm-core/src/types.rs:69 | parse_account_id_for_migration | 5 | prod | D | Широкий резолв через крейты |
| pwm-core/src/types.rs:83 | reject_ambiguous_legacy_pretty_domain | 5 | prod | D | private; ясность важнее 4 сегментов |
| pwm-core/src/types.rs:216 | domain_raw_from_account_id | 5 | prod | D | private helper |
| pwm-core/src/types.rs:224 | format_domain_pascal_hex_width | 5 | prod | D | Вторая волна: например `fmt_dom_pascal_hex_w` при согласии стиля |
| pwm-core/src/types.rs:235 | render_account_id_for_user | 5 | prod | D | Связано с человекочитаемым выводом; много связей |
| pwm-core/src/wallet_read.rs:159 | account_id_from_truth_source | 5 | prod | D | SYNC с pwm-cli store |
| pwm-core/src/wallet_read.rs:307 | parse_der_idx_m0_path | 5 | prod | D | SYNC pwm-cli/store |
| pwm-core/src/wallet_read.rs:339 | parse_wallet_read_v3_header | 5 | prod | D | Загрузчик формата |
| pwm-tui/src/config.rs:36 | cross_shard_target_rpc_base | 5 | prod | D | Название отражает env/контракт |
| pwm-tui/src/config.rs:53 | shard_hint_from_rpc_url | 5 | prod | D | Лёгкая волна 2: `shard_hint_from_rpc` |
| pwm-tui/src/roaming.rs:376 | import_provenance_from_target_facts_once | 6 | prod | M | `import_prov_tgt_facts_once` (**M №18**); синхрон с CLI см. backlog |
| pwm-tui/src/roaming.rs:435 | import_provenance_from_target_facts_retry | 6 | prod | M | `import_prov_tgt_facts_retry` (**M №19**) |
| pwm-tui/src/roaming.rs:499 | post_imp_tx_src_relay | 5 | prod | D | Компактнее: например `relay_imp_tx_at_src` — отдельный мини-пасс |
| pwm-tui/src/wallet.rs:45 | try_decrypt_wallet_secret_payload | 5 | prod | D | Кошелёк/крипто-путь |
| pwm-tui/src/wallet.rs:201 | wallet_try_unlock_with_passphrase | 5 | prod | D | То же |
| pwm-tui/src/wallet.rs:437 | wallet_encrypt_or_rekey_disk | 5 | prod | D | То же |
| pwmd/src/api/common.rs:390 | snapshot_save_under_inner_lock | 5 | prod | D | Внутренний hot-path helper |
| pwmd/src/api/common.rs:558 | persist_snapshot_or_http_err | 5 | prod | D | Связка с HTTP слоем |
| pwmd/src/bootstrap.rs:28 | app_from_dev_net_shard | 5 | prod | D | Семейство фабрик `app_from_*` |
| pwmd/src/bootstrap.rs:91 | app_from_genesis_in_shard | 5 | prod | D | То же |
| pwmd/src/bootstrap.rs:95 | app_from_genesis_shard_identity | 5 | prod | D | То же |
| pwmd/src/bootstrap.rs:116 | app_from_genesis_with_data | 5 | prod | D | То же |
| pwmd/src/bootstrap.rs:123 | app_from_genesis_data_shard | 5 | prod | D | То же |
| pwmd/src/identity.rs:141 | default_runtime_identity_for_shard | 5 | prod | D | Явная привязка к shard enum |
| pwmd/src/lifecycle.rs:619 | seal_skip_ctx_uses_block_height | 6 | test | M | `seal_skip_ctx_block_h` (**M №12**); модуль **`mod tests`** — скрипт верен |
| pwmd/src/logging.rs:271 | looks_like_id_or_hash | 5 | prod | D | Возможная волна 2: `looks_like_id_hash` |
| pwmd/src/logging.rs:1089 | peer_target_isolated_from_main_sink | 6 | test | M | `peer_sink_isolated_from_main` (**M №13**); **`mod tests`** |
| pwmd/src/main.rs:431 | deprecated_shard_arg_was_used | 5 | prod | D | Волна 2: `deprecated_shard_arg_used`; низкий приоритет |
| pwmd/src/roaming.rs:437 | export_readiness_allows_tip_advance_empty_blocks | 7 | test | M | `export_rd_tip_advance_empty` (**M №14**); **`mod tests`** |
| pwmd/src/snap_bench_hlp.rs:14 | bench_snap_path_from_env | 5 | prod | D | Bench-хелпер; не semver API поверхность |
| pwmd/src/snap_bench_hlp.rs:26 | bench_genesis_path_from_env | 5 | prod | D | То же |
| pwmd/src/snap_bench_hlp.rs:78 | mk_dev_cfg_and_json | 5 | prod | M | **5>4 prod** → `mk_dev_cfg_json` (**M №20**); «NOTE 4s» в списке оркестратора неверен для текущего имени |
| pwmd/src/snapshot/incremental.rs:120 | sync_epoch_disk_to_tip | 5 | prod | D | Отражает алгоритм синхронизации |
| pwmd/src/snapshot/incremental.rs:298 | load_tail_blocks_from_epochs | 5 | prod | D | То же |
| pwmd/src/snapshot/incremental.rs:454 | monolithic_save_syncs_when_disk_behind_memory | 7 | test | M | `mono_save_sync_disk_lag` (**M №15**); **`mod tests`** |
| pwmd/src/snapshot/incremental.rs:482 | runtime_persist_after_disk_lag_loads | 6 | test | M | `runtime_persist_disk_lag` (**M №16**) |
| pwmd/src/snapshot/incremental.rs:509 | epoch_trust_load_respects_tail_cap | 6 | test | M | `epoch_trust_respects_tail_cap` (**M №17**) |
| pwmd/src/tests/helpers.rs:15 | credit_min_import_fee_for_tests | 6 | test | M | `credit_min_import_fee_tests` (**M №11**) |
| pwmd/src/tests/http_export.rs:185 | v1_exp_rd_conflict_when_bridge_trust_latched | 8 | test | M | `v1_rd_conflict_bridge_trust` (**M №6**) |
| pwmd/src/tests/http_status.rs:609 | v1_tx_rejects_underfunded_transfer_mempool | 6 | test | M | `v1_tx_underfunded_xfer_mempool` (**M №7**) |
| pwmd/src/tests/http_status.rs:728 | v1_tx_parity_burn_purpose_invalid | 6 | test | M | `v1_tx_burn_purpose_bad` (**M №8**) |
| pwmd/src/tests/http_status.rs:758 | v1_tx_parity_claim_daily_limit | 6 | test | M | `v1_tx_claim_daily_limit` (**M №9**) |
| pwmd/src/tests/http_status.rs:805 | v1_tx_parity_import_fee_too_low | 7 | test | M | `v1_tx_import_fee_low` (**M №10**) |
| pwmd/src/transport/dial.rs:291 | broke_trust_test_overrides_hello_genesis_field | 7 | prod | R+M | Фактически **`#[cfg(test)] mod trust_test_fake_genesis_tests`**; см. **M №5** |
| pwmd/src/transport/transport_tick.rs:76 | update_seed_peer_after_attempt | 5 | prod | D | Transport state machine; DEFER |
| pwmd/src/transport/transport_tick.rs:92 | set_seed_peer_next_due | 5 | prod | D | DEFER |
| pwmd/src/tx_policy.rs:343 | shard_label_for_domain_hi | 5 | prod | D | Волна 2: `shard_label_dom_hi`; private |

## 5. Requirements fit / backlog

- **pwm-coding:** выполнить **волну M (20 переименований)** + обновить call sites / тестовые фильтры; прогнать `python scripts/check_rust_fn_name_segments.py` на затронутых путях до пустого `violations` **для изменённых файлов** или на всём `crates/` по решению владельца.
- **Скрипт / инфра:** в backlog — расширить детектор: любой **`#[cfg(test)] mod <ident> {`**, не только `mod tests`, чтобы убрать класс **R**-ложных prod (block.rs, dial.rs).
- **DEFER:** для символов **D** при принятии долга допустить краткий англоязычный `/// rationale: exceeds 4 prod segments; unchanged for …` по шаблону из политики (без блокирующего требования в этом аудите).

## 6. Style / Safety / Tests

- **Style:** массовые **D** символов с **ровно 5** сегментами в prod — формально вне текущего cap ≤4 из `AGENT_PROMPT_coding.md`; они зафиксированы как технический долг, не как немедленный defect одного MR.
- **Safety:** переименования из списка **M** затрагивают в основном тесты и внутренние хелперы; риск поведения минимальный при чистом rename.
- **Tests:** после волны M — прежний набор CI/крейтов должен проходить; отдельных новых сценариев не требуется.

## 7. Verdict

**APPROVED** для артефакта triажа: первоочередной список **M (~20)** и полная классификация **67** строк готовы для **pwm-coding**; задокументированы **R** (ограничение скрипта) и **D** (backlog).

## 8. Participation (orchestrator)

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/v2-4-naming-audit-20260506.md
token_usage: {"source":"estimate","input":null,"output":null,"total":15000,"confidence":"low"}
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/v2-4-naming-audit-20260506.md'
git add 'tasks/20260506-v2-sprint4-burn-clients.json'
git commit -m 'docs(v2-4-s4): workspace naming audit triage'
```

**Verdict:** APPROVED — triаж 67 нарушений готов; волна M (~20 rename) передана pwm-coding; RECHECK=R зафиксированы из‑за эвристики `mod tests` в скрипте.
