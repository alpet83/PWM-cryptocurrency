# Changelog

Notable behavior and documentation changes. Section timestamps are **UTC**, derived from the authoring commit (`git show -s --format=%cI <hash>`). Parenthetical `abbrev` is the tip commit for that batch.

---

## 2026-05-16T22:00Z — old task backlog cleanup (`tasks/20260516-old-task-backlog-triage.json`)

### Changed

- **Tasks:** closed stale `in_progress` / `blocked` S14/S15/V2-era tickets as completed, superseded, or deferred after owner approval; remaining live backlog is explicit `open` rather than misleading active work.
- **Docs:** added detailed owner-decision report for old task tails and revised the domain-index SQLite recommendation to defer any domain registry data-source choice until lease/auction work, considering ClickHouse.

---

## 2026-05-16T20:00Z — domain cluster allocation policy note (`tasks/20260516-domain-cluster-allocation-policy.json`)

### Added

- **Docs:** clarified that corporate/sector domain clusters are not permanently constrained to a one-industry-one-shard model; IT may receive a future reserved multi-cluster allocation, with an initial planning direction of up to 16 base clusters and up to 255 rentable `domain_lo` values per cluster after service reservations.
- **Governance:** kept current `docs/DOMAINS.md` / `domain_index.rs` as runtime source of truth and deferred concrete codes, labels, auction policy, and migration path to a future ADR/RFC before production domain auctions. Review: `docs/reviews/20260516-domain-cluster-allocation-policy-review.md`.
- **V4 gap:** documented future `domain_lo = 0` root/generic company registration via extended corporate `INIT`, separate from rented `domain_lo > 0` namespace leases, and linked it to the V4 policy/emergency-routing backlog. Review: `docs/reviews/20260516-domain-lo-zero-init-policy-gap-review.md`.

---

## 2026-05-16T19:30Z — pwmd runtime log control operator RPC (`tasks/20260516-pwmd-runtime-log-control-rpc.json`)

### Added

- **`pwmd`:** authorized operator/debug endpoints `GET|POST|DELETE /v1/operator/log/override` for temporary runtime log verbosity/focus overrides with loopback or `PWM_ADMIN_TOKEN` bearer gate, TTL auto-restore, whitelisted focus filters, and `pwmd::operator` audit events.
- **Docs/tests:** RFC 17, operator API docs, and focused `op_log_` tests cover invalid focus/TTL, remote denial, token allow/wrong bearer, bounded TTL restore, and lightweight TCP smoke. Review: `docs/reviews/runtime-log-control-rpc-review-20260516.md`.

---

## 2026-05-16T18:30Z — MVP v3 public devnet foundation closeout (`tasks/20260516-v3-sprint4-public-devnet-closeout.json`)

### Added

- **V3 closeout:** integrated public-devnet smoke on fresh deterministic demo genesis passed with 21B PWM premine verification, CY 3-node startup, `/v1/status`, `/v1/head`, `/v1/accounts`, and `/v1/account/:id`; final review: `docs/reviews/sprint-v3-4-public-devnet-closeout-review-20260516.md`.
- **Docs:** V3 glossary/checklist/roadmap traceability updated; `POST /v1/tx` smoke remains an explicit follow-up beyond the V3 foundation gate.

---

## 2026-05-16T17:00Z — MVP v3 Sprint 3 demo genesis/public devnet package (`tasks/20260516-v3-sprint3-demo-genesis-devnet.json`)

### Added

- **Scripts:** demo genesis build/verify/start path for public devnet, including a 21B PWM premine target (`21_000_000_000_000_000` raw), fail-fast verifier, and CY launcher overrides for generated genesis path/passphrase.
- **Docs:** public devnet quickstart runbook with premine math, API smoke, demo-only security posture, and review traceability. Review gate: `docs/reviews/sprint-v3-3-demo-genesis-devnet-review-20260516.md`.

---

## 2026-05-16T15:30Z — MVP v3 Sprint 2 snapshot schema/replay foundation (`tasks/20260516-v3-sprint2-snapshot-replay.json`)

### Added

- **`pwmd`:** centralized Epoch Snapshot manifest schema contract for `pwm-epochs-manifest.json`, focused v1 acceptance / unsupported-version rejection tests, and lightweight replay determinism gate `cargo test -p pwmd --lib v3_replay_det_gate_ok`.
- **Docs:** storage/runbook updates for manifest `schema_v`, replay gate commands, and the boundary between current Epoch Snapshot and future Bootstrap Snapshot. Review gate: `docs/reviews/sprint-v3-2-snapshot-replay-review-20260516.md`.

---

## 2026-05-16T09:30Z — MVP v3 Sprint 1 spec/ADR/API foundation (`tasks/20260516-v3-sprint1-spec-adr-api.json`)

### Added

- **Docs:** MVP v3 foundation plan, `/v1/*` API freeze skeleton, ADR index, and V3 ADR drafts for IPv4 Claiming, Offchain Scaling, and Cleanup-chain / Bootstrap Snapshot / external anchoring. Review gate: `docs/reviews/sprint-v3-1-spec-adr-api-review-20260516.md`.

---

## 2026-05-15T10:45Z — MVP v2 public docs readiness (`tasks/20260515-mvp-v2-public-docs-readiness.json`)

### Changed

- **Docs:** public-facing MVP v2 package aligned before mirror sync: README/README-ru no longer claim per-block marks accrual, `docs/pwm-core.md` reflects the current seal path, MVP plans use relative links, and `docs/CONCEPT_ROADMAP.md` is intentionally excluded from the public package.

---

## 2026-05-15T06:32Z — pwm-cli: wallet-first `resolve_master_seed` (тикет `20260509-cli-resolve-master-wallet-first`)

### Changed

- **`pwm-cli`:** при явном `--wallet-out` и **существующем** файле `addr-derive` / `addr-bruteforce` берут master из кошелька; `--master` / `PWM_MASTER_SEED` / `MASTER_SEED` допустимы только если байтово совпадают с кошельком, иначе явная ошибка конфликта. С **`--overwrite-wallet`** внешний seed обязателен, чтение старого master из файла не выполняется. См. `docs/reviews/20260509-cli-resolve-master-wallet-first.md`.

---

## 2026-05-22T18:00Z — pwm-cli: `--master` без значения и env `MASTER_SEED`

### Changed

- **`pwm-cli`:** для `addr-derive` / `addr-bruteforce` флаг `--master` может быть **без аргумента** (пустое значение → fallback); после `PWM_MASTER_SEED` (clap `env`) поддерживается **`MASTER_SEED`**; сообщение об ошибке в stateless обновлено.

---

## 2026-05-22T16:00Z — план MVP v2: Sprint V2-9 помечен completed

### Changed

- **`docs/plans/mvp_v2.md`:** в YAML `v2-sprint-9-validator-clone-attestation` → `status: completed`; в § Sprint V2-9 добавлен блок **«Статус закрытия спринта»** со ссылками на ревью, partition-lite тикет и CONCEPT_* .
- **`docs/reviews/20260509-v2-9-rfc16-sprint-checklist.md`:** в шапке отсылка к фиксации закрытия в `mvp_v2.md`.

---

## 2026-05-22T14:00Z — V2-9 optional partition-lite test + scan_pwmd_log_counters

### Added

- **`pwmd`:** harness `cluster_partition_attest_stuck` (2-of-3, второй attester обрывает сессию до attest; ожидание `quorum_timeout` `got=1 need=2` в WARN); см. `tasks/20260522-v2-9-optional-partition-lite-fault.json`.
- **`docs/reviews/20260510-v2-9-slice-b-wave-notes.md`:** команда `cargo test -p pwmd cluster_partition`.
- **`scripts/scan_pwmd_log_counters.ps1`:** режим `-LogDir`, `-PerFile`, счётчики `sealed height`, lease acquired/renewed.

### Note

- На Windows полный `cargo test -p pwmd cluster_partition` может конфликтовать с запущенным `pwmd.exe` (блокировка артефакта); для узкого прогона: `cargo test -p pwmd --lib cluster_partition`.

---

## 2026-05-22T12:00Z — CONCEPT_* + сканер логов pwmd

### Changed

- **`docs/CONCEPT_ROADMAP.md`**, **`docs/CONCEPT_PROGRESS.md`:** синхронизация с фактическим закрытием V2-9 (RFC 16) и статусом **legacy V2-8 Slice 6** (`blocked`, не предусловие V3); уточнён backlog опциональных fault-тестов.
- **`scripts/scan_pwmd_log_counters.ps1`:** быстрый подсчёт характерных подстрок в текстовых логах `pwmd` (cluster/sync/seal) для CY lab / длинных прогонов.

---

## 2026-05-14T12:00Z — `tasks/20260509-cli-wallet-out-master-seed-fallback.json` (pwm-cli)

### Changed

- **`pwm-cli` (`addr-derive`, `addr-bruteforce`):** получение master seed в порядке **явный `--master`** → **`PWM_MASTER_SEED`** → при **явном** `--wallet-out` и существующем файле — **`master_seed_hex`** из wallet YAML (шифрование учитывает `--wallet-passphrase` / `PWM_WALLET_PASSPHRASE`). Для stateless `addr-derive` без явного `--wallet-out` по-прежнему нужны `--master` или env.
- **`pwmd`:** только **rustfmt** в затронутых тестах/transport для зелёного **`cargo fmt --check`** workspace.
- **`issues-report.md`:** операторская ловушка про explicit `--wallet-out` и fallback seed.

---

## 2026-05-20 — документация: имена полей `TransportSnapshot` vs JSON

### Changed

- **`docs/blockchain-sync.md`**, **`docs/pwmd.md`**, **`docs/runbook-same-shard-sync-v1.md`**, **`docs/GLOSSARY.md`**, **`docs/rfc/15-same-shard-sync-v1.md`**, **`docs/reviews/20260517-attester-cup-epoch-clamp-review.md`**, **`CHANGELOG.md` (архивная строка):** в тексте — идентификаторы Rust после рефакторинга имён; где ответ HTTP держит прежний ключ — явная отсылка к **`serde(rename)`** в **`metrics.rs`**.

---

## 2026-05-20 — `tasks/20260520-slice-entity-name-segments.json`

### Added / changed

- **`scripts/check_entity_name_segments.py`:** линтер длины имён для **`fn`**, полей **`struct`/`enum`/`union`**, **`const`/`static`**, **`mod`**, snake_case **`type`**, **`macro_rules!`**; JSON с полем **`entity`**; те же лимиты prod/test, что и у предшественника.
- **`scripts/check_rust_fn_name_segments.py`:** shim на новый скрипт + **stderr** deprecation.
- **`docs/AGENT_PROMPT_coding.md`**, **`docs/AGENT_PROMPT_testing.md`**, **`docs/AGENT_PROMPT_review.md`**, **`docs/AGENT_PROMPTS.md`:** норматив и команды обновлены под **`check_entity_name_segments.py`**.
- **Код (`pwm-coding`):** устранены все текущие нарушения сегментной политики в **`crates/*`**; где нужна стабильность wire/JSON — **`#[serde(rename = "...")]`** на коротких полях Rust.

---

## 2026-05-09 (pwmd — live short-tail: приоритет hdr/blk, CUP только при глубоком lag)

### Changed

- **`pwmd` (`sync_live`):** старт epoch CUP из **`on_tip`** только при **`cup_on`** (повтор mid-catch-up) или **`lag ≥ SYNC_CUP_LAG_MIN` (256)**; убрана ветка «`live_stall` + lag ≥ 32». Короткий хвост: **`ask_hdr`** цепляется при неполном батче заголовков, если ещё есть высоты до peer tip; после успешного live-apply при пустых очередях blk и **`tip_lag < 32`** — дополнительный **`ask_hdr`** (**`live_tail_pull_hdr`**). **`sync_prog_tail_quiet`:** тихий хвост по **`tip_lag < 32`**, без условия **`rem ≤ 1`** (обновлены тесты троттлинга прогресса).
- **`docs/blockchain-sync.md`:** описание политики CUP, live short-tail и Standby-тишины согласованы с кодом.

---

## 2026-05-18T12:00Z — `tasks/20260518-slice-peers-yaml-bootstrap.json`

- **`pwmd`:** флаг **`--peers-list <PATH>`**, при отсутствии — чтение **`{state_root}/peers.yaml`** только если файл есть; формат YAML `peers: ["host:port"]`; объединение с **`--transport-peer-seed`** с дедупликацией; из сидов удаляется **`--transport-peer-listen`** текущей ноды; после успешного завершения **`run_with`** эффективный список записывается обратно в использованный файл. Модуль **`crates/pwmd/src/peer_list.rs`**.

---

## 2026-05-09 (CY lab — multi-loopback `:3030` / `:3130`)

### Changed

- **`cy-cluster-common.ps1`:** по умолчанию адреса **`127.0.0.1`–`127.0.0.3`**, HTTP **3030** и peer **3130** на каждом узле (вместо одного IP с портами 3030–3032 и peer 33430–33432).
- **`scripts/cy_cluster_mvp_v2_tail_smoke.ps1`**, **`docs/runbooks/cy-lab-multi-ip-same-ports.md`:** согласованы с новой схемой RPC.

---

## 2026-05-09 (scripts — MVP v2 tail CY smoke; runbook multi-IP)

### Added

- **`scripts/cy_cluster_mvp_v2_tail_smoke.ps1`:** preflight, 2–3 узла CY lab, сверка `/v1/head`, опциональный relay `tx-burn-mark` через RPC аттестера (`127.0.0.2:3030`), `-Attach`.
- **`docs/runbooks/cy-lab-multi-ip-same-ports.md`:** KM-Test / `169.254.0.0/16` vs `127.0.0.0/8`, одинаковые порты на разных IP.
- **Тикет:** `tasks/20260509-mvp-v2-tail-automated-smoke.json`; ссылка в `docs/plans/mvp_v2.md` (§Декомпозиция).

---

## 2026-05-09 (pwmd — Standby `Sync progress` только вне короткого хвоста)

### Fixed / changed

- **`pwmd` (`sync_live`):** для **`SealRole::Standby`** строка **`Sync progress`** не печатается **только** при **`sync_prog_tail_quiet`** (нет **CUP**, `lag < 32`, `rem ≤ 1`). При докачке / большом хвосте прогресс снова виден в консоли.

---

## 2026-05-09 (standby — no Sync progress; checkpoint log every 100 blocks)

### Added / changed

- **`pwmd`:** для **`SealRole::Standby`** отключён вывод **`Sync progress`** (`maybe_log_sync_prog`); периодический сброс на диск для standby — каждые **100** блоков (`STANDBY_SYNC_FLUSH_BLK_IV`), лог **`standby sync checkpoint`** с **`flush_iv=100`**.
- **`docs/blockchain-sync.md`:** раздел про ведомые узлы и смок.
- **`scripts/cy_cluster_two_node_smoke.ps1`:** **`SMOKE_PASS`** по **snapshot ready** + **proposer listening**; **`RequireQuietTail`** совместим с **`maxPct==0`** у Standby.
- **Тикет:** `tasks/20260509-standby-minimal-sync-log.json`.

---

## 2026-05-09 (docs — blockchain-sync; pwmd — live-tail progress quiet; smoke strict tail)

### Added / changed

- **`docs/blockchain-sync.md`:** нормативное описание режимов same-shard sync, отличие **live short-tail** от патологии, критерии приёмки и ссылки на RFC 15 / RCA.
- **`pwmd` (`sync_live`):** при здоровом **live short-tail** увеличен интервал консольного **`Sync progress`** (`SYNC_PROG_LIVE_TAIL_MS`), путь **`quiet_goal_bump`**; снижены дубли **`ask_hdr`** для того же `from_h` при уже in-flight запросе.
- **`scripts/cy_cluster_two_node_smoke.ps1`:** флаг **`-RequireQuietTail`** (порог по хвосту лога + `maxPct`); исправлен разбор строк Windows PowerShell 5.x (нет **`>=`/`%` в двойных кавычках**).
- **Тикет:** `tasks/20260514-blockchain-sync-quiet-acceptance.json`.

---

## 2026-05-13 (pwmd — short tail vs CUP, cluster-before-sync)

### Fixed / changed

- **`pwmd` transport:** Отложенный старт **epoch catch-up (CUP)** при отставании **менее 32 блоков** от peer tip: при `live_stall` используется **live headers/blocks**; глубокая докачка по-прежнему при **lag ≥ 256**; **retry mid-CUP** (`cup_on`) не блокируется.
- **`pwmd`:** В циклах чтения peer-сессий (**steady seed**, **inbound**, **initial exchange**) обработка **`ClusterPropose` / `ClusterAttest` перед** массивом sync-v1 кадров; исходящий порядок после batch: **cluster propose → sync tip** (раньше наоборот), чтобы снизить задержку кворума при насыщенном sync-трафике.
- **`sync_live`:** Троттлинг **`Sync progress`** — убран сброс **`sync_log_done`** при отложенном логе с **`rem > 0`**, чтобы не печатать **100% на каждый блок** при быстрой догонке.
- **`SyncPeerState::sync_pct100_goal`:** не повторять строку **`Sync progress 100%`** для того же **peer tip goal** (дубли с разных вызовов `maybe_log_sync_prog` / сброс `sync_log_done`).
- **`sync_live` / `on_tip`:** если **дельта высот** `head_h - local_h` **строго меньше** **`SYNC_CUP_SHORT_TAIL_MAX` (32)** и при этом активен **epoch CUP**, режим catch-up **снимается** (`cup_clear`, сброс `cup_try` / `cup_next_ms`), счётчик **`TransportSnapshot::sync_cup_demote_tail`** (JSON-ключ снапшота `sync_cup_demote_short_tail_total`), лог **`peer sync cup_demoted_short_tail`**. Интеграционные тесты CUP позже переведены на **глубокий lag (256+)** для политики CUP; см. актуальный `sync_live` и `docs/blockchain-sync.md`.

---

## 2026-05-13 (pwm-core — JSON peer wire u128; CY two-node smoke)

### Fixed

- **`pwm-core`:** JSON peer wire (`serde_json`) — `u128` в `TxBody`, `SignedTx.import_fee`, `ExportProvenance.amount` через модуль **`ser_json_u128`** (encode: decimal string, decode: decimal string или `u64`), чтобы catch-up/live не рвали сессию с **`wire_decode_failed: u128 is not supported`** на фреймах с полным **`Block`** / **`Vec<SignedTx>`**. Тикет: `tasks/20260517-attester-sync-stall-at-4pct.json`.

### Added

- **`scripts/cy_cluster_two_node_smoke.ps1`** — запуск **`cy-cluster-proposer.ps1`** и **`cy-cluster-attester.ps1`** с перенаправлением stdout/stderr, окно **`SmokeSeconds`** (по умолчанию 120), затем **`taskkill` `pwmd.exe`**, сводка **Sync progress**, хвост **`logs/**/pwmd-peer-cy-attester*.log`**. Host-прогон: MCP **`cq_process_ctl`** (`host: true`, `spawn` → `wait` с таймаутом > окна смока).

---

## 2026-05-16 (docs: RFC15 disk-backed sync note; CY lab follower script)

### Added / changed

- **RFC 0015** (`docs/rfc/15-same-shard-sync-v1.md`): информативная **§14** — раздача истории ниже RAM-хвоста в JsonFile epoch mode (`TAIL_BLOCK_CAP`, epoch JSONL, optional `block_heights`, ссылки на slice/reviews и CY lab).
- **Ops:** `cy-cluster-follower.ps1` — **`--seal-lease-backend process-local`** в паритет с quorum-нодами, комментарий про 3-node mesh; `cy-cluster-common.ps1` — строка про симметричный peer mesh (proposer / attester / follower).

---

## 2026-05-12 (pwm-tui — active panel focus; pwmd — seal lease renew log cadence) <!-- pending tip -->

### Fixed

- **`pwm-tui`:** активная панель Owner / Receivers: рамка и заголовок используют **`LightYellow` + `BOLD`** вместо `Color::Yellow`, чтобы в Windows Terminal (ANSI slot 3 / «тёмный жёлтый» на box-drawing) курс фокуса оставался читаемым; композиция `Block` → `Table(inner)` без изменений. Задача: `tasks/20260512-tui-wt-border-debug.json`, диагностика: `docs/debug/20260512-tui-border-root-cause.md`.
- **`pwmd`:** `INFO seal_lease_renewed` — как и `sealed height=`, только на **высоте 1 и каждые 10 блоков** (`tip_h % 10 == 0`), чтобы не засорять лог при каждом блоке в single-sealer.

### Added / changed

- **`pwmd`:** редкие операторские **`INFO`** на консоль (target **`pwmd::sync`**, не `pwmd::peer`) — **`История синхронизирована на NN%, осталось M блоков`** с троттлингом (~7 s / шаг ≥1 % плюс однократное «догнал» и возобновление после отставания); вызовы в live sync и catch-up (`on_tip`, успешный apply батча, `on_cup_chunk` / `on_cup_done`). Тикет и ревью: `tasks/20260513-slice-sync-console-progress.json`, `docs/reviews/20260513-sync-console-progress-slice.md`.
- **`pwmd`:** sync persistence/progress fix (`tasks/20260514-slice-sync-disk-progress-standby-persist.json`): `apply_blk_batch` теперь сохраняет snapshot при пересечении autosnap-границ внутри батча (не только на финальном `tip_h`), `SealRole::Standby` добавляет ранние flush checkpoints (height `1` и каждые `10` блоков) с `pwmd::sync` INFO, а консольный sync-progress показывает цель + `mem_tip`/`disk_tip` и не сообщает ложные `100%` для genesis-only (`peer_tip=0` без meaningful goal).
- **`pwmd`:** sync serve below RAM tail (`tasks/20260515-slice-sync-serve-below-ram-tail.json`): `on_hdr_req` / `on_blk_req` / `on_cup_req` now fallback to epoch JSONL when requested heights are below in-memory tail, `SyncBlocksReq` carries optional `block_heights` for low-cost disk lookup with hash verification, and a legacy hash-only epoch scan path keeps mixed-version peers compatible. / отдача sync ниже RAM-хвоста теперь идёт с диска epoch JSONL (с верификацией hash и backward-compatible fallback для legacy peers), чтобы узлы не застревали на `mem=0` при `tip > TAIL_BLOCK_CAP`.
- **`pwmd`:** при **`cluster.enabled` + `ClusterRole::Attester`** цикл **`spawn_seal_loop`** не вызывает lease/cluster gate и **`Chain::seal`** (RFC16 non-committer), чтобы не спамить `quorum_pending` после отключения `--debug-disable-seal-loop` у attester. Тест `seal_loop_attester_no_seal`. Тикет: `tasks/20260513-slice-cluster-attester-no-seal-loop.json`.
- **`pwmd`:** для **`cluster.enabled` + `ClusterRole::Attester`** локальный **`SealRole`** выводится как **standby** (RFC16 §8.2) без обязательного `--debug-disable-seal-loop`; старт с `--seal-role active` на attester отклоняется. Лаб: `cy-cluster-attester.ps1` без этого флага; комментарии в `cy-cluster-common.ps1`. Тикет/ревью: `tasks/20260513-slice-cluster-attester-seal-derive.json`, `docs/reviews/20260513-cluster-attester-seal-derive-slice.md`.
- **RFC16** (`docs/rfc/16-validator-clone-attestation.md`): версия **0.4.8**, информативный **§8.2** (выравнивание pwmd / attest path vs полноты §6).
- **`pwmd`:** после успешного **peer-sync** `apply_blk_batch` периодический autosnapshot на границах **`AUTOSNAPSHOT_BLOCK_INTERVAL`** (100 блоков), с тем же `save_seal_persist(Periodic)` / `apply_snapshot_init_state`, что после seal, лог `source=sync_apply`; откат батча при ошибке записи. Регрессионный тест `batch_cross_ckpt_writes_snap`. Тикет и ревью: `tasks/20260512-slice-nonsealing-sync-snapshot-persist.json`, `docs/reviews/20260512-sync-snapshot-persist-slice.md`.

---

## 2026-05-11 (MVP v2 Sprint V2-9 — RFC 16 cluster attestation: wire gate, follower sync) <!-- pending tip -->

### Added / changed

- **`pwmd`:** Cluster attestation wire tests (2-of-2 / 2-of-3, negatives with log asserts), `record_cluster_propose_originated` mirror for proposer harness.
- **`pwmd`:** `sync_live::on_tip` — peer-behind (`head_h < local_h`) no longer triggers false `sync_tip_divergence`; regression `tip_behind_no_divergence`.
- **`pwmd`:** Integration `same_shard_follower_tcp_tip` — same-shard cluster-enabled source vs cluster-off follower, bidirectional TCP, converge `tip_h`/`tip_hash`.
- **Docs:** RFC 16 v0.4.7 §9.5 (busy attester / quorum slot demotion seed); slice B/C reviews and wave notes; task `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`.
- **`pwmd`:** optional `--node-instance-id` / `PWM_NODE_INSTANCE_ID` — stable wire id for RFC16 `cluster-members` across restarts (default remains `node_id-pid-time_ms`); warn when cluster uses static members without override.
- **Ops:** `cy-cluster-common.ps1`, `cy-cluster-proposer.ps1`, `cy-cluster-attester.ps1`, `cy-cluster-follower.ps1` — lab launchers for shard CY (domain `0x2C`, `test-cluster-CY`) long-run quorum + follower soak.
- **`pwmd`:** Production RFC16 2-of-2 path — proposer sends `ClusterPropose` on live peer TCP, mirrors round locally for `tip_h+1`, attester auto-signs/sends `ClusterAttest`; seal loop can pass `run_cluster_gate` when peers are up. See ticket `tasks/20260510-v2-9-cluster-seal-e2e.json`.
- **Ops:** CY lab scripts use `--seal-lease-backend process-local` on proposer/attester to avoid file-lease CAS fights in local two-process runs; note in `issues-report.md`.

---

## 2026-05-06 (MVP v2 Sprint V2-7 — burn UX fixes + genesis marks) <!-- tip: c8dc3c5 -->

### Fixed / changed

- **Protocol:** cross-domain `BurnMark` beneficiary now allowed — `burn_ctx_source_dom` removed from validation; burn is unilateral (debits sender's marks only). `pwmd 0.1.52`.
- **Protocol:** `accrue_marks` / `accrue_marks_v2` no longer called in `Chain::seal()` — per-block automatic marks accrual eliminated.
- **Protocol:** genesis accounts receive starting marks = `bal / 1_000_000` (1 mark per whole preminted PWM) at `state0()` init.
- **TUI:** F5 BurnForm pre-fills beneficiary from selected receiver row.
- **TUI:** Burn attempt with locked wallet now shows "Wallet is locked — press F3 to unlock".
- **TUI:** Account balance shows `spendable / staked` when staked > 0.
- **TUI:** Footer compact: `F7/⇧F7 Stake/Unstake`.
- **Nits/backlog:** doc-comment referencing old `burn_ctx_source_dom`; "Wallet is locked" message alignment between F5 block and submit path.
- **Task:** `tasks/20260506-v2-sprint7-burn-fixes.json` (closed).

---

## 2026-05-06 (MVP v2 Sprint V2-6 — TUI Stake/Unstake + F5 auto-claim) <!-- tip: 837daa0 -->

### Added / changed

- **`pwm-tui`:** F7 opens Stake form; Shift+F7 opens Unstake form (`stake_form.rs`, `StakeMode { Stake, Unstake }`).
- **`pwm-tui`:** F5 refactored — silent `ClaimTx(CLAIM_ALL)` before opening BurnForm; hint shown if `staked==0 AND marks==0`.
- **`pwm-tui`:** `AcctRow.staked: u128` added (parsed from `/v1/account`); `last_claim_wall` wall-clock guard removed.
- **`pwm-tui`:** `marks_modal.rs` deleted; `MarksModal` fully removed.
- **`tx_submit.rs`:** `submit_stake` / `submit_unstake` added.
- **Footer:** updated with F7 / Shift+F7 hints.
- **Nits/backlog:** F8 fallback for Unstake not implemented; F5 branch in `tui_loop.rs` is a refactor candidate.
- **Task:** `tasks/20260506-v2-sprint6-tui-stake.json` (closed).

---

## 2026-05-06 (MVP v2 Sprint V2-5 — marks u32 + formula normalization) <!-- tip: 0af10b0 -->

### Changed (breaking — protocol/state)

- **`Account.marks` type: `u128 → u32`** across all crates (core, pwmd API, pwm-cli, pwm-tui).
- **Maturity formula:** `matured = (staked / 1_000_000) * hours` — **1 whole PWM staked for 1 hour = 1 mark** (was `staked_raw * hours`; raw-unit formula caused hyperinflation).
- **`CLAIM_ALL: u32 = u32::MAX`** (was `u64::MAX as u128`); all claim/burn tx fields are `u32`.
- **`DEF_MARKS_STAKE_MIN = 1_000_000`** (= 1 whole PWM minimum to earn marks; was 1 raw unit).
- **Snapshot migration:** legacy JSON `marks` values `> u32::MAX` are divided by `1_000_000` and clamped to `u32::MAX` on load.
- **pwmd version:** `0.1.50 → 0.1.51` (API contract change).
- **RFC 11, RFC 12, WHITE_SPEC:** updated with new formula and `u32` types.
- **Nits/backlog:** bincode snapshot round-trip (`snap_keep_imp_replay_guard` ignored) pending separate fix; `accrue_marks` doc drift minor nit.
- **Task:** `tasks/20260506-v2-sprint5-marks-u32.json` (closed).

---

## 2026-05-06 (MVP v2 Sprint V2-4 — BURN_MARK end-to-end, marks display CLI/TUI)

### Added / changed

- **`pwm-tui`:** `AcctRow` now carries `marks: u128`; account table has a `Marks` column; F5 burn-mark form shows `Current marks: N` read-only header.
- **`pwm-cli`:** `run_tx_burn_mark` fetches and prints current marks before submitting (`/v1/account/:id`); prints confirmation after successful submit. New `fetch_marks` helper in `rpc_helpers.rs`.
- **Tests:** `tx_burn_err_insufficient_marks` unit test in `pwm-cli` (HTTP 400 `E_BURN_OVER_BALANCE` mock).
- **Docs:** tester guide `§11 stake → accrue → burn` scenario; UX freeze spec `v2-4-slice0-ux-freeze-20260506.md`; error wire format clarified (`InsufficientMarks` → `E_BURN_OVER_BALANCE / STATE_CONFLICT`).
- **Nits/backlog:** post-submit marks refetch and `acct show` marks column deferred to backlog; e2e `InsufficientMarks` via live `pwmd` deferred.
- **Task:** `tasks/20260506-v2-sprint4-burn-clients.json` (closed).

---

## 2026-05-06 (MVP v2 Sprint V2-3 - validator emission policy)

### Added / changed

- **`pwm-core`:** added policy-gated emission runtime: legacy `policy_ver=1` keeps old reward/marks path; V2 policy uses `pwm_stake_min`, `marks_stake_min`, and deterministic `season_coeff_ppm`.
- **Genesis/schema:** `GenCfg` carries V2-3 policy fields; `pwm genesis-build` emits schema v5 while `pwmd` keeps schema v4/v5 load compatibility.
- **Replay/snapshot:** `pwmd` replay paths are aligned with `Chain::seal` for the V2-3 policy branch; Slice 2 closeout and tests cover snapshot/replay gates.
- **Docs/demo:** V2-3 design freeze, Slice 1/2/3 reviews, demo guide for schema v5 / policy_v2, and `AGENTS.md` + `.cursorrules` guardrails for orchestrator role preservation.
- **Task:** `tasks/20260506-v2-sprint3-emission-whales.json` (closed).

---

## 2026-05-06 (MVP v2 Sprint V2-2 — единый `marks` в state)

### Added / changed

- **`pwm-core`:** удалено зеркало **`State.marks_quota`**; марки в консенсусе только в **`Account.marks`** (`BURN_MARK`, `Claim`, `accrue_marks`).
- **`pwmd`:** snapshot JSON больше не пишет `state.marks_quota`; загрузка legacy снапшотов валидирует строки `marks_quota` строго (**`quota == account.marks`**).
- **`pwm-cli`:** help burn — формулировка в **marks**, не `marks_quota`.
- **Docs:** `docs/pwm-core.md`, `docs/plans/mvp_v2.md` (ориентиры кода), freeze `docs/reviews/sprint-v2-2-slice0-account-api-freeze.md`; ревью `docs/reviews/sprint-v2-2-slice1-review-20260506.md`.
- **Task:** `tasks/20260506-v2-sprint2-double-balance.json` (закрыт).

---

## 2026-05-05 (E-3 clients, handoff slice)

### Added / changed

- **`pwm-core`:** `reject_wire::summarize_tx_reject_json` for CLI/TUI error lines (`code`, `response_class`, `phase`, `tx_kind`, …).
- **`pwm-cli`:** `tx-burn-mark --purpose` (v2 dedication); stderr note when omitted; **`tx-claim`** (`--claim-mode`, `--claim-units`, `--anchor-ref`, `--fee`); `post_signed_tx` uses structured reject hint when body matches pwmd JSON.
- **`pwm-tui`:** **F5** opens burn modal (marks / beneficiary / purpose / confirm) and submits `BurnMark` via RPC worker; transfer errors use the same reject summarizer; footer `F5 burn` (no longer «burn→CLI»); `SendStepFlow` helpers `pub(crate)` for reuse.
- **Docs:** `docs/tester-guide-cli-tui-scenarios.md` §6/§7 updated for v2 burn/claim and F5 modal.
- **Task:** `tasks/20260505-v2-e3-clients-claim-burn.json`.

---

## 2026-05-05T18:41:09Z

### MVP v2 V2-1 batch — RFC pack, core claims, pwmd reject parity (slices E-1/E-2)

Formal RFCs 0011–0014 (`docs/rfc/`), WHITE_SPEC §9 extension, plan `docs/plans/mvp_v2.md`, review/task traceability under `docs/reviews/` and `tasks/20260505-v2-*.json`.

### Added / changed

- **Core (`pwm-core`):** `ClaimTx` baseline (free/paid, anchor, maturity `floor(staked_pwm_coins * hours)`), auto-claim on stake-management txs when `matured_units > 0`, `BURN_MARK` with `purpose` (trim, 1..80 UTF-8 bytes, C0/C1 gate), unified `marks` path with legacy `marks_quota` mirror; `IMPORT` minimal fee **0.01 PWM** to target shard `fee_pool`. Tip-aware `TxContext` for `precheck_apply` / replay / repair / CH replay / lifecycle diagnostics (`Chain::next_apply_ctx`, `apply_tx_with_ctx`).
- **Node (`pwmd`):** stable JSON reject wire (`phase`, `tx_kind`, `response_class`, `error.code` / `message` / `trace_id`), centralized `TxError` → `E_*` + response class mapping; HTTP tests **preflight/apply parity** for burn purpose invalid, free-claim daily limit, import fee too low (`crates/pwmd/src/tests/http_status.rs`). Build marker bump in `crates/pwmd/Cargo.toml`.
- **Docs/process:** orchestrator anchors for `mvp_v2.md`; slice reviews and testing reports; ticket `tasks/20260505-v2-e2-api-preflight-parity.json` closed after **pwm-review** PASS (nits: optional `claim_mode` in JSON per RFC 0014, some import prefilter plaintext — tracked for E-3).

---

## 2026-05-04T07:44:46Z (`85bec28`)

### Added / changed

- **Mempool / seal:** `POST /v1/tx` runs tip `precheck_apply_tip` before enqueue; underfunded txs return **409** without entering the pool. **Seal loop** drops the first failing tx on apply errors instead of infinitely requeuing the same batch.
- **Bridge federation:** bridge-only `BridgeFederationCommitment` digest on compatible peers; `bridge_federation_trust` / `bridge_refusal_reason` on `/v1/status`; `POST /v1/bridge-federation/reset`; relay and peer hellos carry commitment where applicable.
- **Debug:** `--broke-trust-test` advertises a fake genesis digest in transport `NodeHello` so honest peers reject handshakes (operator negative testing).
- **Docs/tests:** tester guide updates; HTTP tests for underfunded transfer and export-readiness when bridge trust is latched; slice20 e2e coverage updates; operator/review notes.

---

## 2026-05-04T04:43:30Z (`38dcdc4`)

### Fixed

- **Cross-shard send from pwm-tui:** relayed `POST /v1/tx` (`Import`) could return HTTP **502** on the source and **400** on the peer (`invalid import: export_id is not known and embedded provenance is missing`), while the **source balance still decreased** once `Export` was sealed — no credit on the target. **Cause:** `pwm-tui` built a bare signed `Import` without `import_provenance`, unlike `pwm tx-import`, so `enforce_import_provenance_prefilter` on the recipient rejected the relay. **Fix:** before submitting the relay, fetch matching rows from **`GET /v1/cross-shard/facts`** on the **target** RPC (with backoff), build `ExportProvenance`, **`set_import_provenance_signed`** — same contract as **`pwm-cli`**. Extended retry backoff for transient `embedded provenance` messages.

---

## 2026-05-03 — cross-shard stabilization & snapshot stack

Batch on **2026-05-03** (commits through `b979153`; intermediate `chore(tasks)` traceability-only commits omitted here).

### Added / changed

- **JsonFile runtime save:** epoch persistence without monolithic full-epoch encode on each seal (`2212fbd`).
- **Snapshot diagnostics / repair:** replay mismatch diagnostics (`61fa3d4`); offline snapshot repair tool (`669a41a`).
- **Cross-shard:** import provenance replayable on target (`1270b06`); `GET`-style cross-shard backfill endpoint (`d56a699`); stabilization contract docs (`353d814`).
- **ClickHouse / incremental (slice7 wave4):** DDL alignment, `shard_balance`, validators table cfg (`551ce84`).
- **Docs:** sprint-15 closeout gate (`0584c84`), architecture review (`63a4200`), testing preflight scripts (`7d37f8d`, `bb9856d`), CH JsonFile fallback design (`f3a2d12`).

---

## Archive — since MVP multi-sprint plan (anchor `10b0b47`)

**Anchor:** `10b0b47` *feat(wallet,docs): Sprint 14 slices 1–3 and orchestrator guardrails* — **2026-04-28T07:06:33Z** — introduces `docs/plans/mvp_v1_testnet_multi-sprint.md` and related governance. Everything below summarizes **284** commits from `10b0b47~1..HEAD` (through **2026-05-04**); pure `chore(tasks)` / traceability-only SHAs are omitted in prose.

### 2026-05-02 (UTC day) — Sprint 15 **Slice-O.1** modularization waves

- **pwmd:** incremental decomposition of the transport stack into focused modules (metrics, tick, dial, lifecycle, spawn; `peer_session` → wire / inbound / seed with connect–handshake–session; `handshake_state`, `incoming_hello`; transport and seed-session test trees).
- **pwm-cli:** `main.rs` split into `cli_config`, `rpc_helpers`, `cli_cmd`, `cmd_*`, `wallet/`, `cli_parse`, `signer`, subprocess integration tests, and related docs closeouts (waves 5–18 narrative in commit subjects).
- **pwm-tui:** S15-O.1 waves 5–12 — extract models, status, config, modals, roaming/send_form/history, layout/footer, term.draw panels, `test_support`, narrow crate-root `pub`.
- **Meta:** `//!` module banners; test function names ≤5 segments; reviewer/orchestrator checklist traceability docs.

### 2026-05-01 — Cross-shard behavior, relay wiring, S15-O cleanup

- **pwmd:** federation table + gossip-style relay path; relay HTTP uses **RPC port − 100** vs peer convention (`ab9f9ad`); mirror roaming after relay import (`01b57dc`); cross-shard observability (handoff register, relay flow ids); identifier shortening after style review.
- **pwm-tui:** cross-shard Import after `relayed` + step-5 target balance; shared `TextInput` for modals / SendForm.
- **Docs / hygiene:** Sprint **3.17** roaming closeout & xshard doc sync (`5672fdd`); rename `ROUMING*` → `ROAMING*` (`6204dc2`); S15-O group A cleanup (`1b6c5a0`: TUI xflow, dial, deprecated `--shard` note).
- **Core / UI:** S15-O-B display, `wallet_io`, RPC helpers (`0ac777c`).

### 2026-04-30 — Stateful peer transport & operator validation

- Stateful **peer listener** transport (`4458c6a`); HTTP peer-seed handshake diagnostics (`105e401`); sprint-15 live connectivity / import-balance review captures.

### 2026-04-29 — Sprint 15 architecture track & one-window relay

- Export **readiness** preflight (`0afc8f9`); **foreign balance** semantics split (`98cf1b2`); genesis **join guardrails** (`91cb84a`); **trusted relay** for one-window cross-shard (`678fe82`); TUI staged cross-shard diagnostics (`1d550c9`, `042abc8`).
- Planning: sprint-15 architecture checklist (`4684517`); slice reviews (genesis consistency, import visibility).

### 2026-04-28 — Sprint 14 tail + plan landing

- **`10b0b47`:** multi-sprint plan + orchestrator rules on disk.
- Same & adjacent commits: Sprint 14 slices **4–11** / genesis / cluster / hardening (`f43550a` … `065c5f2`); logging fixes (`1d1fb9c`, …); `data_file` wiring for snapshots (`ca9df3e`).

---

### Machine-readable full history

```bash
git log 10b0b471e96f4f24f8c4e02074023701e588cdba~1..HEAD --reverse --format="%cI %h %s"
```

Use this for exact ordering, authors, and subjects; the narrative sections above are **grouped by calendar period** for readability only.
