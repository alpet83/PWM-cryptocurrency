# Same-shard sync v1 runbook (Slice 5)

Практический runbook для инцидентов same-shard sync v1 и cluster storm-guard из RFC 15.

**Имена счётчиков:** ниже в списках — поля **`TransportSnapshot` / `TransportCounters`** в Rust (`crates/pwmd/src/transport/metrics.rs`). В **JSON** ответов (`/v1/status`, dev peers) часть ключей **старее и длиннее** из‑за `serde(rename)` и обратной совместимости. Если значение в ответе не находится по имени из этого runbook, см. атрибут `rename` у соответствующего поля в `metrics.rs`.

## 1) Что проверить первым (быстрый checklist)

- Убедиться, что peer реально в `full_v1` (в логах есть `peer sync mode negotiated ... mode=full_v1`).
- Проверить в `/v1/dev/peers` блок `transport`: растут ли `sync_tip_seen_total`, `sync_hdr_req_total`, `sync_blk_req_total`.
- Если есть лаг, проверить динамику `sync_cup_start_total` / `sync_cup_chunk_total` / `sync_cup_done_total` / `sync_cup_fail_total`.
- При всплеске gossip проверить `sync_tx_seen_total`, `sync_tx_accept_total`, `sync_tx_drop_total` и `sync_tx_drop_reason` (JSON: `sync_tx_drop_reason_total`).
- Для anti-amplification проверить `mempool_push_suppressed` (JSON: `mempool_cluster_push_suppressed_total`) и `mempool_egress_relay_total` (нет ли suppress без egress).

## 2) Профили синхронизации (`legacy_observe` vs `full_v1`)

В текущем v1 профиль выбирается из handshake:

- `full_v1`: peer объявляет `services` с `sync` и валидный `sync_profile` (`sync_wire_version=1`, лимиты > 0).
- `legacy_observe`: peer не удовлетворяет требованиям `sync_profile`; соединение может жить, но sync-v1 кадры игнорируются.

Операционный индикатор:

- Лог `peer sync mode negotiated ... mode=full_v1|legacy_observe`.
- Счётчик drop по причине `profile_mismatch` в `sync_v1_drop_reason` (JSON: `sync_v1_msg_drop_reason_total`).

## 2.1) Single-sealer failover toggles (S1)

Runtime profile и роль оператора:

- `--deployment-profile single-sealer` (default) — строгий MVP режим: same-validator `active/active` отклоняется.
- `--deployment-profile multi-sealer-experimental` — явный non-default режим для экспериментов; на старте пишется warning.
- `--seal-role active|standby` — явная локальная роль для hello/status сигналов и policy.
- `--debug-disable-seal-loop` остаётся совместимым fallback: если `--seal-role` не задан, роль выводится как `standby`.

Поля identity-сигналов в hello/status:

- `validator_identity_hash`
- `node_instance_id`
- `seal_role`
- `deployment_profile`

Базовый reason-код strict policy:

- `same_validator_active_conflict` — отклонение/закрытие при same-validator `active/active` в `single_sealer`.

## 2.2) Lease/fencing failover (S2)

`single_sealer` теперь использует lease gate: локальный seal-loop разрешён только при валидной аренде.

Operator knobs:

- `--seal-lease-ttl-ms` / `PWM_SEAL_LEASE_TTL_MS` (default `10000`)
- `--seal-takeover-timeout-ms` / `PWM_SEAL_TAKEOVER_TIMEOUT_MS` (default `8000`)
- `--seal-takeover-max-tip-lag` / `PWM_SEAL_TAKEOVER_MAX_TIP_LAG` (default `1`)
- `--seal-lease-backend file|process-local` / `PWM_SEAL_LEASE_BACKEND` (default `file`; `process-local` только для явного fallback в test/dev)
- `--seal-lease-dir <DIR>` / `PWM_SEAL_LEASE_DIR` (default `<state_root>/leases`; для `file` backend обязательно должен быть доступен для записи)

Ожидаемые переходы (2-node baseline):

- `active_sealing -> fenced_standby`: потеря/перехват аренды, локальный sealing подавляется.
- `standby_syncing -> suspect_active_lost`: аренда активного истекла, standby ждёт takeover timeout.
- `suspect_active_lost -> active_sealing`: timeout прошёл, tip свежий, takeover с новым `term/fence` успешен.
- `old active return -> standby_syncing`: старый active без аренды не seal'ит до повторного acquire.

Проверка в `/v1/status`:

- gate/state: `seal_gate_allowed`, `lease_state`, `lease_last_reason`
- backend observability: `lease_backend_mode`, `lease_backend_path`, `lease_last_backend_error`
- lease signal: `lease_owner_id`, `lease_term`, `lease_expires_at_ms`, `lease_last_tip`, `lease_fence`
- counters: `lease_acquire_ok`, `lease_renew_ok`, `lease_loss_total`, `lease_reject_total`, `lease_takeover_ok`

Операторские заметки:

- В `single_sealer` default `file` backend fail-closed: backend error => `seal_suppressed_by_fence` и `lease_last_backend_error` в `/v1/status`.
- `process-local` не защищает от split-brain между независимыми процессами с одинаковым validator key; использовать только при явном осознанном отключении внешнего lease coordination.

## 3) Ключевые метрики и reason-коды

Синхронизация цепочки:

- tip: `sync_tip_seen_total`
- headers req/resp: `sync_hdr_req_total`, `sync_hdr_resp_total`
- blocks req/resp: `sync_blk_req_total`, `sync_blk_resp_total`
- apply: `sync_apply_ok_total`, `sync_apply_fail_total`
- conflicts: `sync_fork_conflict_total`

Catch-up:

- start/progress/done/fail/drop: `sync_cup_start_total`, `sync_cup_chunk_total`, `sync_cup_done_total`, `sync_cup_fail_total`, `sync_cup_drop_total`
- причины fail: `sync_cup_fail_reason` (JSON: `sync_cup_fail_reason_total`) (`req_write`, `chunk_*`, `done_mismatch`, `nack`, ...)

Mempool gossip и storm-guard hooks:

- seen/accepted/dropped: `sync_tx_seen_total`, `sync_tx_accept_total`, `sync_tx_drop_total`
- drop причины: `sync_tx_drop_reason_total` (`duplicate`, `invalid`, `rate_limit`, `profile_mismatch`, `shard_mismatch`, `unsupported_msg`)
- ingress path: `mempool_ingress_kind_total` (`p2p`)
- suppression events: `mempool_push_suppressed` (JSON: `mempool_cluster_push_suppressed_total`) (например `recent_peer_dedup`)
- egress routing: `mempool_egress_relay_total` (например `same_shard_peer`)

Bad/corrupt frames:

- общий drop: `sync_v1_drop_total` (JSON: `sync_v1_msg_drop_total`)
- причина drop: `sync_v1_drop_reason` (JSON: `sync_v1_msg_drop_reason_total`) (`shard_mismatch`, `profile_mismatch`, `decode_failed`, `invalid_frame_len`)

## 4) Troubleshooting

### A. Sync “залип” (tip есть, apply не идёт)

- Если `sync_tip_seen_total` растет, но `sync_hdr_req_total`/`sync_blk_req_total` стоят:
  - проверить, не накопился ли `profile_mismatch`/`shard_mismatch`.
  - проверить reconnect path (рост `peer_close_by_reason`, `session_retrying_total`).
- Если `sync_blk_resp_total` растет, а `sync_apply_fail_total` растет:
  - смотреть `peer sync apply failed ... reason=...`.
  - проверить, не растет ли `sync_fork_conflict_total`.

### B. Catch-up loops

- `sync_cup_start_total` растет, `sync_cup_done_total` почти не растет:
  - проверить `sync_cup_fail_reason` (JSON: `sync_cup_fail_reason_total`) (часто `chunk_*`, `done_mismatch`, `nack`).
  - проверить логи `peer sync catchup fail ...` и `peer sync catchup aborted by nack ...`.
- Если после fail нет возврата в live:
  - убедиться, что после `on_nack` снова растет `sync_hdr_req_total` (fallback в live flow).

### C. Gossip storm / blackhole risk

- Рост `mempool_push_suppressed` (JSON: `mempool_cluster_push_suppressed_total`) без роста `mempool_egress_relay_total` — риск blackhole на egress.
- Для same-shard peer проверять логи:
  - `peer storm guard suppress ...`
  - `peer storm guard egress route ...`
- При необходимости временно снизить входной поток/частоту heartbeat и повторить наблюдение.

### D. Controlled divergence dump (debug-only)

- Для incident-диагностики persistent divergence можно временно включить:
  - `--debug-dump-on-divergence`
  - `--debug-dump-cap 16` (или ниже в ограниченной среде)
  - `--debug-dump-trigger-streak 2` (не снижать до 1 в обычных прогонах)
  - при необходимости отдельный каталог: `--debug-dump-dir <DIR>`
- Файлы пишутся как `.../blocks/b{height}.json` (по умолчанию рядом с `data_file`), содержат локальный block snapshot и метаданные (`source=node divergence probe`).
- Без включения флага dump-файлы не создаются; при достижении cap запись останавливается и логируется `reason=cap_reached`.
- После завершения диагностики вернуть флаг в OFF, чтобы не копить служебные файлы на длительном uptime.

## 5) Минимальный incident flow

1. Снять `/v1/dev/peers` snapshot (до изменений).
2. Зафиксировать 2-3 минуты логи `pwmd::peer` с sync/catch-up событиями.
3. Отделить проблему handshake (`legacy_observe`/mismatch) от chain apply (`sync_apply_fail_total`) и от gossip suppression/egress.
4. Применить корректировку (peer profile, сеть, reconnect), снять второй snapshot и сравнить дельты.

## 6) Wave A (автоматический двухнодовый прогон с детерминированной остановкой)

Локальный smoke/операторский запуск:

- Из корня репозитория: `python scripts/wave_a_same_shard_stop.py`
- Скрипт поднимает 2 same-shard ноды (`--transport-real`) с разными `state_root` и `--debug-stop-height`.
- По умолчанию `stop_height = max(--stop-height, 2 * SNAP_CHK_BLK_IV)`, то есть минимум 2 checkpoint-окна.

Ожидаемый `PASS` (stdout JSON):

- `canonical_h` одинаковый на обеих нодах и не ниже целевого `stop_height`.
- Совпадают базовые инварианты epoch-manifest (`schema_v`, `epoch_span`, число epoch rows); для последнего epoch-файла `last_epoch_hash_equal=true`.
- `tip_hash_equal=true`; mismatch tip hash трактуется как реальная дивергенция цепочки (не косметика).
- Для ключевых аккаунтов sender/receiver совпадают `balance_pwm`, `nonce`, `initialized`.

Сигналы `FAIL`:

- превышен `allowed-prestop-lag` до остановки;
- хотя бы одна нода не завершилась сама (timeout/ненулевой код);
- `tip_hash_equal=false` или `last_epoch_hash_equal=false` (реальная дивергенция chain identity);
- расхождение `canonical_h`/snapshot checkpoint/epoch-manifest инвариантов или несовпадение account effects между нодами.
