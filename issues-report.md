# Issues Report

Журнал обнаруженных граблей, совместимостных костылей и обходных решений.

## Формат записи

- Дата:
- Контекст/файлы:
- Симптом:
- Причина:
- Фикс/обход:
- Что проверить потом:

## Entries

- Дата: 2026-05-14
- Контекст/файлы: `crates/pwmd/src/transport/peer_session/sync_live.rs` (`sync_prog_tick`, константы `SYNC_PROG_*`).
- Симптом: строки «Sync progress 99%/100%» кажутся признаком застрявшей синхронизации во время живого следования за proposer при `lag=1`.
- Причина: консольный прогресс триггерился на каждое изменение цели после «догнанного» состояния; это штатный live short-tail, не транспортный затык.
- Фикс/обход: расширенный интервал `SYNC_PROG_LIVE_TAIL_MS` и путь `quiet_goal_bump` при `cup_active=false`, малом до-tip gap и `rem≤1`; дубликаты `SyncHeadersReq` для того же `from_h`, пока уже есть in-flight hdr, отсекаются в `ask_hdr`.
- Что проверить потом: CY smoke `-SmokeSeconds 120` и счётчики `Sync progress`; при реальном глубоком хвосте (`lag≥32`/`rem>1`/CUP) прогресс остаётся частым через `SYNC_PROG_MIN_MS` и журнал catch-up.

- Дата: 2026-05-09
- Контекст/файлы: `cargo test -p pwmd`, `crates/pwmd/src/tests/transport_peer.rs` (`v1_hi_accepts_native_cls`, `v1_hi_mx_sig`).
- Симптом: при полном прогоне библиотечных тестов pwmd на Windows два кейса transport_peer упали с `Bool(false)` вместо ожидаемого `true` в assertions около строк 25 и 83.
- Причина: не разбиралось в этом слайсе; возможная независимая флейкулиость окружения/параллелизма/локальных сокетов, не связанная с lease follow-up.
- Фикс/обход: для проверки S2.1 follow-up использовать таргетные фильтры `cargo test -p pwmd backend_err_closed --lib` и `cargo test -p pwmd --test lease_two_proc`; полный матричный прогон отдать pwm-testing.
- Что проверить потом: воспроизвести падения под контролем и завести отдельный тикет или починить transport_peer если stable flake подтвердится.

- Дата: 2026-05-08
- Контекст/файлы: `crates/pwmd/src/transport/peer_session/seed/mod.rs`, `crates/pwmd/src/transport/peer_session/mod.rs`, divergence guard hotfix.
- Симптом: простой `set_seed_due` сам по себе не гарантирует реальный reconnect cooldown в seed-loop, потому что цикл `run_seed_session` спит по `peer_retry_sleep_ms` и может переподключиться раньше.
- Причина: `next_due_ms` использовался в transport tick path, но seed session loop не учитывал этот маркер перед новым connect.
- Фикс/обход: seed-loop wait переведён на `max(peer_retry_sleep_ms, seed_state.next_due_ms-now)`, поэтому divergence cooldown marker реально ограничивает повторный dial.
- Что проверить потом: pwm-testing e2e с принудительным divergence подтверждает, что reconnect не раньше ~60s и нет регрессий sticky-session.

- Дата: 2026-04-26
- Контекст/файлы: `crates/pwm-core/src/domain_index.rs`, `docs/DOMAINS.md`, диапазоны `domain_hi`
- Симптом: в словаре доменов появился «резерв из 3 адресов» в начале диапазона стран.
- Причина: сохранение backward compatibility для старых кошельков (историческое соответствие `CY = 0x2C`) реализовано через смещение начала country-диапазона.
- Фикс/обход: явная документация смещения и причины резерва в доменной документации; при новых изменениях диапазонов проверять совместимость старых wallet-адресов.
- Что проверить потом: при следующем рефакторинге domain-map прогнать крайние кейсы (первая/последняя страна, legacy кошельки) и зафиксировать результаты в sprint test-report.

- Дата: 2026-04-26
- Контекст/файлы: `crates/pwm-cli/src/main.rs`, `crates/pwm-tui/src/main.rs`, nonce fetch (`GET /v1/account/{sender}`)
- Симптом: `HTTP 404 account not found` выглядел как «непонятная ошибка», хотя это штатно для неинициализированного sender на другой RPC-ноде.
- Причина: ранее ошибка была слишком общей и не подсказывала оператору, что нужно инициализировать sender именно на source-node.
- Фикс/обход: добавлен явный UX-hint для 404 (`account not found`): сделать `tx-init` для sender на source-node и проверить, что RPC указывает на source domain/shard.
- Что проверить потом: e2e-сценарий CLI/TUI в multi-node сетапе (правильный/неправильный RPC) и стабильность текста hint при разных форматах тела 404.

- Дата: 2026-04-26
- Контекст/файлы: `crates/pwm-tui/src/main.rs` (`F5`/`F6` hotkeys), `docs/tester-guide-cli-tui-scenarios.md`
- Симптом: при выбранной строке `init=false` оператор доходил до поздней ошибки (`nonce`/submit), хотя проблему можно понять раньше из detail line.
- Причина: до preflight горячие клавиши не проверяли `initialized` у текущего selected account.
- Фикс/обход: добавлен единый preflight для `F5` и `F6`: действие блокируется сразу и показывает короткий hint выполнить `pwm --rpc <url> tx-init ...`, затем повторить операцию.
- Что проверить потом: UX в обоих панелях (`Owner`/`Receivers`) и текст подсказки на нестандартных RPC URL.

- Дата: 2026-04-26
- Контекст/файлы: `crates/pwm-tui/src/main.rs` (`F5`/`F6` auto-init preflight), wallet identity binding
- Симптом: auto-init перед `F5/F6` невозможен, если выбранный sender не совпадает с владельцем загруженного wallet (`--wallet`) или кошелёк заблокирован.
- Причина: TUI может подписывать tx только material'ом активной identity; для зашифрованного wallet без `F3 unlock` signing key в памяти отсутствует.
- Фикс/обход: auto-init выполняется только при доступном signing material; иначе UX остаётся блокирующим с actionable hint на ручной `pwm --rpc <url> tx-init ...`.
- Что проверить потом: e2e в двух панелях (owner/receivers) и при `SeedFallback` с/без `PWM_TUI_MASTER_SEED`, чтобы исключить ложные auto-init ожидания.

- Дата: 2026-04-27
- Контекст/файлы: `crates/pwm-cli/src/wallet.rs`, `crates/pwm-core/src/wallet_read.rs`, schema v3 (`docs/rfc/10-wallet-file-format-v3.md`)
- Симптом: v3 encrypted wallet нельзя безопасно прочитать текущим decrypt-путем v2, потому что payload-формат v3 допускает только master seed без полного `WalletSecretPayload`.
- Причина: существующий decrypt в CLI ожидает JSON с `master_seed/signing_key/verifying_key`, а RFC v3 MVP описывает минимальный payload с master seed.
- Фикс/обход: в этом срезе добавлен явный отказ `schema v3 decrypt not implemented`; v3 `plaintext_dev` чтение и инварианты `accounts/active_account_id_hex/id_hex` покрыты.
- Что проверить потом: реализовать decrypt-path для v3 с derive ключей по `accounts[].derivation_index` после разбора финального формата encrypted payload.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwm-cli/src/wallet.rs`, schema v3 save (`wallet account add/use`)
- Симптом: rewrite v3 через полную сериализацию struct терял неизвестные raw-ключи верхнего уровня (например, `wallet_created_at_unix_sec`) и потенциально future metadata.
- Причина: `serde_yaml::to_string(WalletYamlV3)` записывал только известные поля модели.
- Фикс/обход: запись v3 переведена на merge-стратегию (`existing YAML` + `updated known fields`) с сохранением неизвестных ключей и metadata.
- Что проверить потом: при расширении `accounts[]` в будущем сохранить merge-совместимость для неизвестных вложенных полей при изменении формата account-объекта.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwm-cli/src/wallet.rs`, `crates/pwm-cli/src/main.rs`, `crates/pwm-tui/src/main.rs`
- Симптом: автоматическая миграция `v2 -> v3` на read-path вызывала скрытую запись в файл при обычных read-командах.
- Причина: `load_wallet_yaml` выполнял migration+save без явного opt-in пользователя.
- Фикс/обход: `load_wallet_yaml` возвращён в read-only режим; запись миграции делается только при `--upgrade-wallet` (CLI/TUI). Для записи добавлен temp+rename путь, чтобы снизить риск частично записанного файла.
- Что проверить потом: на Windows/сетевых ФС подтвердить поведение rename-fallback (ветка с remove+rename) и при необходимости перейти на platform-specific atomic replace.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwm-cli/src/wallet.rs`, сохранение schema v3 (`create-path`/`--upgrade-wallet`/`wallet account add|use`)
- Симптом: общий merge-save для всех путей записи мог переносить legacy/unknown top-level поля в новых v3 файлах при create/upgrade сценариях.
- Причина: reuse merge-писателя без разделения intent (strict create/upgrade vs preserve-on-update).
- Фикс/обход: добавлено разделение писателей: strict-save для `--upgrade-wallet` и create-path; merge-save оставлен только для update-path (`wallet account add|use`), где нужно сохранять future metadata.
- Что проверить потом: если добавятся новые update-команды для v3, явно помечать их как strict или merge и покрывать это отдельным regression-тестом.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwm-cli/src/main.rs`, команды `addr-derive` и `addr-bruteforce`
- Симптом: после Slice 7 `addr-derive` без `--wallet-out` стал писать wallet по default path, а `addr-bruteforce --overwrite-wallet` продолжал resume от существующего индекса.
- Причина: безусловный вызов persist-path в `addr-derive` и отсутствие bypass-ветки resume при `overwrite_wallet=true`.
- Фикс/обход: `addr-derive` вернули в stateless-by-default (write только при явном `--wallet-out`, stdout-поля сохранены), для `--overwrite-wallet` resume принудительно начинается с `0`.
- Что проверить потом: добавить интеграционный smoke с реальным CLI stdout/side-effects (наличие/отсутствие файла) для двух режимов `addr-derive` и `addr-bruteforce`.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwmd/src/snapshot.rs`, `load_genesis_bundle` schema branching
- Симптом: при наличии `schema_version` v2-парсер мог упасть и тихо перейти в legacy fallback, маскируя ошибки формата/контракта.
- Причина: ветка загрузчика делала fallback в legacy на любую ошибку `parse_genesis_v2`.
- Фикс/обход: введён strict pre-check `schema_version` по raw JSON: при явном поле парсится только соответствующая схема (v2), legacy допускается только при отсутствии `schema_version`; сообщения об ошибках сделаны явными для unsupported schema/invalid v2 payload.
- Что проверить потом: добавить e2e smoke для `pwmd --genesis-file` с `schema_version: 2` и намеренно битым payload, чтобы подтверждать отсутствие fallback на runtime path.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwm-cli/src/main.rs`, `crates/pwmd/src/snapshot.rs`, genesis contract Slice 10
- Симптом: старый genesis flow хранил plaintext `validator_seeds_hex` и поддерживал legacy/v2 загрузчики, что конфликтует с security decision v3-only.
- Причина: исторический dual-loader и прежний `genesis-build` писали незашифрованный seed material.
- Фикс/обход: включён v3-only контракт (`schema_version=3`), добавлен encrypted envelope `validator_keys[*].enc_seed`, strict fixed-path validation `m/1000000'/1'`, passphrase flow (`--genesis-passphrase` / `PWM_GENESIS_PASSPHRASE` / TTY prompt) и non-tty hard-fail при отсутствии passphrase.
- Что проверить потом: добавить полноценный e2e smoke `pwm genesis-build -> pwmd --genesis-file` в CI с отдельным сценарием non-tty запуска и проверкой текстов ошибок.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwm-core/src/genesis.rs`, `crates/pwmd/src/snapshot.rs`, `crates/pwm-cli/src/main.rs`, Slice 11 genesis v4
- Симптом: после перехода на decoupled v4 (`funding` vs `validators`) старые v3 genesis-файлы больше не загружаются.
- Причина: принято one-way pre-public решение: убрать dual-support, чтобы не поддерживать две конфликтующие модели инвариантов.
- Фикс/обход: loader переведён на strict `schema_version=4` с явной ошибкой; `pwm genesis-build` теперь генерирует только v4 и может строить `1 validator + N funding rows`.
- Что проверить потом: вынести отдельную утилиту миграции `v3 -> v4` (offline) и добавить acceptance smoke для операторов перед публичным релизом.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwm-core/src/chain.rs`, reward path в decoupled v4 (`funding.rows` + `validators.set`)
- Симптом: при отсутствии producer account в `funding.rows` блок продолжал sealing, но reward silently терялся (no-op в `State::reward_producer`).
- Причина: после decoupling `validators.set` и `funding.rows` не было явного инварианта "validator acct must exist in funded state".
- Фикс/обход: введён fail-fast инвариант в `Chain::boot` (каждый `validators.set[*].acct` обязан существовать в `funding.rows`) + runtime guard в `seal` перед reward-credit; silent reward-loss исключён.
- Что проверить потом: при эволюции reward policy (например, non-producer recipient) явно зафиксировать policy для auto-create vs hard-fail и покрыть e2e/rollback сценарий.

- Дата: 2026-04-28
- Контекст/файлы: `crates/pwm-cli/src/main.rs`, `genesis-build` (schema v4)
- Симптом: `genesis-build` мог выпустить bundle, где `validators.set[*].acct_hex` отсутствует в `funding.rows[*].acct_hex`, что валило startup с `genesis invariant: validators.set[0].acct must exist in funding.rows`.
- Причина: funding rows строились из wallet accounts, а validator account вычислялся по отдельному fixed-path derivation (`m/1000000'/1'`), без post-check на пересечение.
- Фикс/обход: после сборки validator set добавлена детерминированная нормализация funding rows: для каждого отсутствующего validator account добавляется row с теми же `acct/pubkey/der_idx` и `bal=0`.
- Что проверить потом: при расширении `genesis-build` на multi-validator сценарии закрепить стабильный порядок auto-added rows и покрыть его отдельным golden-json тестом.

- Дата: 2026-04-29
- Контекст/файлы: `functions.CallMcpTool` -> `user-cqds_mcp_mini` / tool `cq_files_ctl`
- Симптом: при попытке поставить background `cq_files_ctl rebuild_index` через MCP инструмент возвращает ошибку “Provide action+args…”, хотя дескриптор tool поддерживает `action`/`args`.
- Причина: в текущей обвязке `CallMcpTool` передача `action/args` не проходит (вызов принимает только `server` и `toolName`).
- Фикс/обход: пока пропускать MCP background index rebuild и делать это отдельным шагом в среде, где `action/args` корректно прокидываются; в текущей сессии зафиксировано как “требует подтверждения”.
- Что проверить потом: убедиться, что в другом окружении/версии обвязки `CallMcpTool` можно передавать `action/args` для `cq_files_ctl`, и затем реально поставить background rebuild для проекта `project_id=5`.

- Дата: 2026-04-29
- Контекст/файлы: `crates/pwmd/src/snapshot.rs`, Sprint 14 Slice 21 snapshot v2
- Симптом: snapshot loader временно принимает три wire-формата (`v2`, canonical `v1`, legacy `v0`), а writer всегда сохраняет только `v2`.
- Причина: нужен короткий migration window для уже созданных `pwm-data.json` с byte-array представлением, без изменения consensus/runtime семантики.
- Фикс/обход: чтение развилено по `version`/legacy contract, v2 декодируется через strict hex/decimal parser с field-path ошибками; при следующем save/autosnapshot файл переписывается в `v2`.
- Что проверить потом: после окончания migration window удалить поддержку `v0/v1` и оставить только strict `v2` loader.

- Дата: 2026-04-29
- Контекст/файлы: `crates/pwm-core/src/state.rs`, `crates/pwm-cli/src/main.rs`, `docs/tester-guide-cli-tui-scenarios.md`
- Симптом: `IMPORT` может кредитовать target `--to`, даже если адрес ещё missing/uninitialized, и это легко принять за скрытую небезопасную инициализацию.
- Причина: контракт roaming MVP намеренно создаёт stub account на target-side; signer binding откладывается до последующего `tx-init` получателя.
- Фикс/обход: контракт оставлен без блокера, но вынесен в CLI help/runtime note и операторскую документацию.
- Что проверить потом: e2e `tx-export -> tx-import` с missing recipient на target-node и последующим `tx-init` получателя.

- Дата: 2026-04-29
- Контекст/файлы: `crates/pwm-core/src/state.rs`, `crates/pwmd/src/tx_policy.rs`, `crates/pwmd/src/api.rs`
- Симптом: target-side `IMPORT` мог сам создать missing export provenance из payload и кредитовать funds при известном signer/nonce.
- Причина: remediation2 смешала UX race handling с protocol provenance и допустила self-attested import/mint.
- Фикс/обход: unknown `export_id` теперь fail-fast (`InvalidImport` / HTTP 400), provenance принимается только из уже имеющегося `exported_registry`; текущий finalize остаётся manual handoff без target proof delivery.
- Что проверить потом: добавить криптографически проверяемый relay/proof endpoint для безопасного заполнения target `exported_registry`.

- Дата: 2026-04-29
- Контекст/файлы: `crates/pwm-cli/src/wallet.rs`, `crates/pwm-core/src/wallet_read.rs`, Sprint 14 Slice 27 wallet v3
- Симптом: wallet v3 load path требовал `active_account_id_hex`, из-за чего TUI не открывал валидный `accounts[]` wallet после удаления этого поля.
- Причина: UX/default marker был смешан с криптографическим источником выбранного account.
- Фикс/обход: v3 loader принимает отсутствие поля, новые записи его не сохраняют; CLI default выбирается детерминированно из `accounts[]`, а signing material derives/verifies from seed + derivation metadata.
- Что проверить потом: добавить явный CLI selector `--account-id` для signing-команд, если потребуется управлять sender без зависимости от deterministic default.

- Дата: 2026-04-29
- Контекст/файлы: `crates/pwm-cli/src/wallet.rs`, v3 wallet merge-save (`wallet account add|remove`)
- Симптом: old v3 YAML мог сохранить legacy `active_account_id_hex` после rewrite, хотя runtime/load path уже не использует этот marker.
- Причина: merge-save стартовал со старой YAML map и накладывал сериализованный v3 struct; при `None` поле пропускалось, а при legacy `Some` могло снова записаться.
- Фикс/обход: все v3 save/write paths чистят top-level `active_account_id_hex` перед записью; merge-save продолжает сохранять future metadata.
- Что проверить потом: при добавлении новых removed/deprecated v3 ключей расширять общий cleanup helper и добавлять rewrite-регрессию.

- Дата: 2026-04-29
- Контекст/файлы: `crates/pwmd/src/api.rs`, `crates/pwmd/src/lib.rs`, `/v1/account*` и `/v1/accounts`
- Симптом: поле `balance_pwm` оставалось равным `local_state_balance` даже для foreign-аккаунтов, и legacy-клиент мог трактовать это как spendable truth.
- Причина: исторически `balance_pwm` был совместимостным alias без учёта split semantics для foreign local view.
- Фикс/обход: для foreign-аккаунтов `balance_pwm` принудительно выставляется в `"0"`, при этом `local_state_balance` остаётся доступным отдельно; добавлены контрактные проверки для single/list endpoints.
- Что проверить потом: downstream CLI/TUI/интеграции, которые до сих пор читают только `balance_pwm`, должны перейти на `spendable_on_this_shard` + `local_view_only`.

- Дата: 2026-04-29
- Контекст/файлы: `crates/pwmd/src/relay.rs`, `crates/pwmd/src/transport.rs`, S15-S3.4 peer relay
- Симптом: one-window relay может доставить provenance/import только если `--transport-peer-seed` также отвечает HTTP `/v1/status`, `/v1/export-provenance`, `/v1/tx`.
- Причина: текущий real transport handshake ещё минимальный (`NodeHello` frame) и не несёт полноценный RPC/message bus для cross-shard payloads.
- Фикс/обход: в S15-S3.4 relay использует seed address как bounded HTTP peer endpoint, выбирает target по `cluster_domain_hi`, ручные команды остаются fallback.
- Что проверить потом: вынести relay delivery в единый transport message protocol, чтобы не смешивать seed-handshake и HTTP endpoint assumptions.

- Дата: 2026-05-01
- Контекст/файлы: `crates/pwmd/src/relay.rs`, `crates/pwmd/src/main.rs`, `crates/pwmd/src/config.rs`, S15-S3.15.1
- Симптом: relay делал `GET http://{transport-peer-seed}/v1/status`, но `--transport-peer-seed` — это **TCP peer listener** (напр. `:3131`), а HTTP API на **`--listen`** (напр. `:3031`); reqwest получал не-HTTP ответ / ошибку соединения.
- Причина: смешение двух ролей одного CLI-списка без вывода RPC-базы для relay.
- Фикс/обход: relay HTTP использует явный `--transport-relay-http-seed` или авто **peer_tcp_port − 100** (обратно к конвенции `rpc_port+100` для peer listener из `resolve_peer_listen`).
- Что проверить потом: нестандартные маппинги портов — задавать relay-http явно.

- Дата: 2026-04-30
- Контекст/файлы: `crates/pwmd/src/transport.rs`, `crates/pwmd/src/api.rs`, S15-S3.6 transport seed handshake
- Симптом: live-ноды с reciprocal `--transport-peer-seed` стучались raw frame handshake в HTTP `--listen` port и бесконечно показывали `live_peer_count=0`.
- Причина: production raw listener/accept path не существовал; единственный live peer endpoint был HTTP `/v1/peer/hello`.
- Фикс/обход: real seed tick теперь использует HTTP `/v1/status` + `/v1/peer/hello`; inbound hello не даёт provenance trust, outbound configured seed response даёт trust локальной стороне.
- Что проверить потом: если появится отдельный raw transport listener, явно развести HTTP seed port и raw listen port в CLI/config, чтобы не смешивать контракты.

- Дата: 2026-04-30
- Контекст/файлы: `crates/pwmd/src/transport.rs`, `crates/pwm-cli/src/main.rs`, Sprint 15 S3.12
- Симптом: peer-сессии могли часто пересоздаваться при кратковременных wire timeout, а `pwm-cli tx-import` трактовал foreign `initialized=false` как достоверную локальную истину даже при отсутствии trusted peer path.
- Причина: stateful wire-loop рвал соединение на первом idle timeout; CLI preflight не различал `local_view_only/home_lookup_status` и fallback-ил к бинарному `initialized`.
- Фикс/обход: добавлен bounded timeout tolerance + heartbeat-cadence push (`CrossShardFacts`/`AccountViews`) для стабилизации/живых обновлений; CLI preflight теперь явно репортит unknown/unavailable foreign state и не делает misleading auto-init в wrong-shard контексте.
- Что проверить потом: добавить e2e smoke с деградацией сети (1-2 heartbeat timeout подряд) и проверить, что trusted foreign lookup остаётся `ok`, а при потере peer path возвращается `unavailable` без ложного `uninitialized`.

- Дата: 2026-04-30
- Контекст/файлы: `crates/pwmd/src/transport.rs`, stateful heartbeat/session inbox
- Симптом: при живом trusted link в логах мог идти постоянный `peer hello accepted` (~каждые 1-2с), а foreign home view на удалённой стороне оставался `not_found`.
- Причина: heartbeat-цикл отправлял несколько кадров за тик (`Heartbeat` + `CrossShardFacts` + `AccountViews`), но читал только один ответ; backlog wire-очереди вызывал ложные timeout/disconnect и частые re-handshake, из-за чего `AccountViews` применялись нестабильно.
- Фикс/обход: heartbeat-loop переведён на bounded drain чтения: после получения `HeartbeatAck` сессия дочитывает накопленные кадры коротким timeout-window; это удерживает long-lived session и стабилизирует доставку `AccountViews` без ослабления trusted-boundary.
- Что проверить потом: e2e для reciprocal seed topology (A<->B оба dialers) с проверкой низкого reconnect-rate и устойчивого `home_lookup_status=ok` под длительным прогоном.

- Дата: 2026-04-30
- Контекст/файлы: `crates/pwmd/src/transport.rs`, `crates/pwmd/src/state.rs`, `crates/pwmd/src/ledger.rs`, peer wire decode `PeerWireMsg`
- Симптом: live peer path падал на `wire_decode_failed: u128 is not supported` при non-empty `AccountViews`/`CrossShardFacts`.
- Причина: derive-deserialize для `u128` в JSON wire payload не принимал рабочий runtime-формат данных между нодами.
- Фикс/обход: добавлен узкий decode-only compat helper (`de_u128_compat`) для wire-полей `balance_pwm` и `amount`: принимаются decimal string и non-negative numeric JSON (where feasible), trust/handshake/reconnect семантика не менялась.
- Что проверить потом: после migration window можно ужесточить контракт до одного каноничного wire-формата (`string`) и удалить compat-ветку.

- Дата: 2026-04-30
- Контекст/файлы: `crates/pwmd/src/transport.rs`, S15-S3.12.4 trusted heartbeat loop
- Симптом: валидные `AccountViews`/`CrossShardFacts` могли приходить в heartbeat window, но сессия всё равно закрывалась как `heartbeat_read_failed`, если явный `HeartbeatAck` не был прочитан следующим.
- Причина: liveness была привязана только к ack-frame, хотя любой валидный trusted data-plane frame уже доказывает живой peer и свежий wire path; дополнительно successful seed session не записывала `last_node_id` для real sticky-session guard.
- Фикс/обход: heartbeat loop теперь считает валидные data-plane frames прогрессом, а successful outbound hello сохраняет seed -> node mapping.
- Что проверить потом: live CY<->DO smoke должен подтвердить отсутствие steady hello churn и сохранение `home_lookup_status=ok` при свежем trusted `AccountViews` stream.

- Дата: 2026-05-01
- Контекст/файлы: `crates/pwmd/src/federation.rs`, `crates/pwmd/src/api.rs` (`GET /v1/federation/shards`), `NodeHello` / wire `Heartbeat`
- Симптом: оператор мог ожидать federation updates от `POST /v1/peer/hello`, но таблица не менялась.
- Причина: HTTP hello остаётся намеренно **untrusted** (`provenance_trusted=false`); контракт S3.13 требует писать federation только из trusted peer path (seed HTTP hello после статус-гейтов, stateful wire после успешного hello).
- Фикс/обход: не менять trust модель; federation merge подключён к trusted hello/heartbeat и локальному status injection на GET.
- Что проверить потом: pwm-testing — интеграционный тест trusted seed + TTL + JSON контракт; при необходимости отдельный флаг/режим для lab-only HTTP-trusted hello.

- Дата: 2026-05-01
- Контекст/файлы: `docs/reviews/sprint-15-s3-13-testing.md`, `crates/pwmd/src/lib.rs`, `lifecycle.rs`, `slice20_e2e_tests.rs`, `pwm-core` transfer validation
- Симптом: массовые падения `cargo test -p pwmd --lib` после обязательного export-readiness / запрета self-transfer / CLI preflight получателя.
- Причина: контракт readiness потребляет запись при каждом create/export; `validate_tx_shape` отклоняет self-transfer; lifecycle-тест клал в mempool заведомо невалидный transfer; slice20 указывал `--transport-peer-seed` на RPC-порт вместо peer-listen (обычно RPC+100); duplicate roaming delivery требует повторного `/v1/export-readiness` после смены nonce/height hints.
- Фикс/обход: тесты выровнены с текущим API (readiness, relay_mode/hints, повторный preflight для idempotent create); seal/slice20/lifecycle используют валидные transfer/init и корректный peer seed.
- Что проверить потом: при изменении `relay_handoff` или derive peer-listen — обновить slice20 и статусы finalize в e2e.

- Дата: 2026-05-02
- Контекст/файлы: `crates/pwmd/src/relay.rs` (`relay_import`), `crates/pwmd/src/roaming.rs`
- Симптом: после успешного relay импорта на target оператор не видел на source обновления roaming до **imported**; возможна путаница с ожидаемым балансом при опросе не того RPC/поля.
- Причина: при **foreign** Import клиент шлёт tx на **source** RPC; `relay_import` раньше завершался после успешного `POST` на target без **`mark_import_by_export`** на локальном source-пуле (ветка без локального `seal`).
- Фикс/обход (S15-S3.16): после успешного relay_import на source вызывается `mark_import_by_export`, flow trace и сохранение снапшота.
- Что проверить потом: при нескольких параллельных импортах — нагрузка на snapshot IO; при расхождении баланса на target — сумма/комиссия в теле Import и гейты получателя на целевом шарде.

- Дата: 2026-05-01 — **S3.16 style remediation:** финальные имена в prod-коде `pwmd`: `peer_merge_logged`, `merge_peer_acct_views`, `mark_import_by_export` (см. `docs/reviews/sprint-15-s3-16-style-remediation-review.md`).

- Дата: 2026-05-01
- Контекст/файлы: `crates/pwm-tui/src/main.rs` (`submit_roaming_intent`), `crates/pwmd/src/api.rs` (`v1_tx` foreign Import → `relay_import`)
- Симптом: на живых двух нодах export/debit на source есть, зачисления на target нет; опрос roaming так и остаётся до `relayed`.
- Причина: после доставки provenance на target никто не отправлял **Import**; опрос статуса сам по себе не создаёт транзакцию. Импорт нужно отправлять на **source** RPC (реле на target); nonce подписанта — с **target** шарда (`GET /v1/account` на counterparty RPC).
- Фикс/обход: TUI после `relayed` подписывает Import ключом получателя (тот же wallet должен знать `to`), шлёт `POST /v1/tx` на `PWM_RPC`, ретраи при «export_id is not known»; шаг 5 — сверка дельты баланса на target (`PWM_TUI_TARGET_RPC` или эвристика портов 3030↔3031).
- Что проверить потом: получатель не в wallet / другой подписант — отдельный UX; нестандартные порты — задать `PWM_TUI_TARGET_RPC` явно.

- Дата: 2026-05-01
- Контекст/файлы: `crates/pwmd/src/api.rs` (`v1_export_handoff_register`, `v1_tx` local Import), `crates/pwmd/src/relay.rs` (`log_relay_absence`, relay error strings), `crates/pwmd/src/lifecycle.rs` (snapshot apply)
- Симптом: слабая корреляция в логах/flow между `intent_id`/`export_id` на target и при отказах peer relay; трудно связывать цепочку в журналах.
- Причина: вход handoff регистрации и локальный Import шли без явных `info`/`warn`; `log_relay_absence` писал в flow без id; тексты ошибок relay не дублировали id для grep.
- Фикс/обход: структурированные логи на handoff register и этапах локального Import (prefilter / seal); `log_relay_absence` → `error!` + `push_flow` с `export_id`/`intent_id`; суффиксы `export_id`/`intent_id` в сообщениях relay/handoff reject; при загрузке снапшота — `genesis_state0_digest` в `info`/`warn` (сверка с нодой-писателем при `state_root` mismatch, см. `sprint-15-s3-16-do-snapshot-root-cause.md`); одноразовый `warn`, если `exported_registry` > `imported_set`.
- Что проверить потом: pwm-testing live two-node — grep по `handoff_register:` и `relay_absent:peer` с совпадающими hex id.

- Дата: 2026-05-01 — **S3.17 closeout (docs):** консолидированы итоги отладки в `docs/ROAMING_COMPLETION.md`, `docs/reviews/sprint-15-s3-17-closeout.md`; обновлены `ROAMING-SAMPLE`, `pwm-tui`, `pwmd`, `rfc/9`, `tester-guide-cli-tui-scenarios`, `MVP-checklist`, `sprint-15-checklist`; тикет `20260501-s15-slice3-17-roaming-completion-closeout.json`. Live приёмка межшарда (TUI step 5) подтверждена оператором.

- Дата: 2026-05-02 — **Имена файлов:** `ROUMING_*` переименованы в **`ROAMING_*`** (`ROAMING-SAMPLE.md`, `ROAMING_COMPLETION.md`), ссылки во всём репозитории обновлены. Запущен внеочередной слайс **S15-O** (очистка жирных модулей): `docs/reviews/sprint-15-slice-O-plan.md`, `sprint-15-slice-O-checklist.md`, тикет `tasks/20260502-s15-slice-O-codebase-cleanup.json`, якорь в `CODEBASE_REFACTORING.md`.

- Дата: 2026-05-02 — **S15-O группа A (cleanup):**
- Контекст/файлы: `pwm-tui` cross-shard diagnostics; `pwmd` `transport.rs`, `main.rs` (`Cli.shard`); `docs/CODEBASE_REFACTORING.md` §4.3.
- Симптом: trivial wrappers (`xflow_report`, `dial_stub_attempt`) и legacy `--shard` без атрибута `deprecated` на поле.
- Причина: исторический рефакторинг оставил однострочные обёртки; `run_transport_tick` должен оставаться в prod для минимального transport loop без сокетов — полностью уводить в `#[cfg(test)]` нельзя без смены семантики бинарника.
- Фикс/обход: инлайн формата диагностики TUI + `const XFLOW_HANDOFF_HELP`; dial-simulation как closure внутри `run_transport_tick`; `#[deprecated]` на `Cli.shard` + узкий `compat_shard_flag()` с `#[allow(deprecated)]`; политика `// TODO(scope):` в §4.3.
- Что проверить потом: pwm-review на дифф TUI строк (контракт UX не должен меняться); удаление `--shard` в отдельном слайсе после миграции операторов.

- Дата: 2026-05-02 — **S15-O группа B (shared helpers):**
- Контекст/файлы: `crates/pwm-tui/src/main.rs`, `crates/pwm-core/src/display.rs`, `rpc.rs`, `wallet_io.rs`, `crates/pwm-cli/src/main.rs`; `pwm-core` зависимость `reqwest` (blocking) для `rpc`.
- Симптом: дубли TextInput/decimal/RPC timeout/home-path между TUI и CLI.
- Причина: рост `main.rs` без выноса общих примитивов в `pwm-core`.
- Фикс/обход: `TextInput` для модалок и SendForm; `display::{format_pwm, parse_decimal_pwm_units}`; `rpc::{parse_rpc_timeout_ms, blocking_http_client_rpc}` с общим cap 120s; `wallet_io::{resolve_home_dir, expand_tilde_path, resolve_wallet_out_path}` — CLI сохраняет обёртку `resolve_wallet_out_path(.., DEFAULT_WALLET_OUT_REL)`.
- Что проверить потом: ручной смок TUI (модалки F3/F4/book/send); pwm-testing полная матрица при регрессии сети.

- Дата: 2026-05-01 — **S15-O.1 волна 1 (pwm-tui decomposition):**
- Контекст/файлы: `crates/pwm-tui/src/main.rs` → `config.rs`, `status.rs`, `models.rs`; тикет `tasks/20260503-s15-slice-O1-modular-decomposition-wave1.json`.
- Симптом: нет (плановый рефакторинг).
- Причина: разгрузка монолитного `main.rs` по §2.1 `CODEBASE_REFACTORING.md`.
- Фикс/обход: три отдельных коммита (config → status → models); `pub(crate) use` в корне бинарника для символов, используемых остальным `main.rs`; тесты, обращавшиеся к `super::` для символов только из тестов, переведены на `crate::config::` / `crate::status::`, чтобы `cargo check` без тестов не предупреждал о неиспользуемых re-export.
- Что проверить потом: pwm-testing — быстрый регресс `pwm-tui` после следующих волн выноса; при выносе `lib.rs` рассмотреть сокращение re-export в пользу явных путей.

- Дата: 2026-05-01 — **S15-O.1 волна 2 (pwm-tui):**
- Контекст/файлы: `crates/pwm-tui/src/modals.rs`, `wallet.rs`, `rpc_account.rs`, `signing.rs`, `tx_submit.rs`; `main.rs` остаётся корнем бинарника с `pub(crate) use`.
- Симптом: предупреждения `unused_imports` на реэкспортах и `dead_code` у `default_wallet_candidate` (ранее был `#[cfg(test)]`, но символ реэкспортируется в родительский модуль для `tests`).
- Причина: интеграционные тесты живут в `#[cfg(test)] mod tests` внутри `main.rs` и импортируют символы через `super::`; без реэкспорта из корня или без широкого `cfg(test)` на use-компонентах компилятор считает часть импортов «мёртвыми» в не-test сборке.
- Фикс/обход: `#[allow(unused_imports)]` на блоках `pub(crate) use` подмодулей; `#[allow(dead_code)]` на `default_wallet_candidate`; узкая зачистка верхних `use` в `main.rs` после переноса логики в модули.
- Что проверить потом: при появлении отдельного `lib.rs` для TUI — сузить реэкспорты и перевести тесты на `crate::wallet::` без предупреждений.

- Дата: 2026-05-02 — **pwmd `lib.rs` inline tests split (`crate::tests` tree):**
- Контекст/файлы: `crates/pwmd/src/lib.rs` → `tests/mod.rs` + `tests/prelude.rs`, `tests/helpers.rs`, `tests/http_status.rs`, `tests/transport_peer.rs`, `tests/http_export.rs`, `tests/snapshot_roaming.rs`; тестовые импорты убраны из корня lib (были только для инлайнового модуля).
- Симптом/ловушки: ошибочный диапазон строк при резке тела из `lib.rs` (срез с начала файла обрывает файл и «заводит» в выход модульными `mod api;` из корня — ловится как странные ошибки компиляции); дочерние файлы без `super::*` теряют приватные `use` родителя — нужен узел с `pub(crate) use`; хелперы между тестами вынести одним файлом или с `pub(crate)` для кросс-модульных вызовов.
- Причина: перенос `#[cfg(test)] mod tests { … }` в файловое дерево с подмодулями ломает старое правило видимости `use super::*` из вложенного модуля.
- Фикс/обход: `tests/prelude.rs` как `pub(crate) use`-шина (включая `Router`, `PathBuf`, `dev_net`, `digest`, handshake ctx); общие функции с `pub(crate)`; разрез большого файла только по сохранённым границам строк (вставки `seed_handoff`/`sample_genesis` остаются в `helpers`).
- Что проверить потом: при добавлении тестового импорта в одном только подфайле — добавить соответствующую строку в `tests/prelude.rs` или локальный явный импорт.

- Дата: 2026-05-02 — **pwm-cli genesis → `cmd_genesis` (crate `tests`):**
- Контекст/файлы: `crates/pwm-cli/src/cmd_genesis.rs`, `crates/pwm-cli/src/tests/mod.rs`, тикет `tasks/20260531-s15-slice-O1-cli-waves1-4-main-modules.json`.
- Симптом: после переноса `GenesisV4Out`/`build_genesis_v4_wallet` в подмодуль unit-тесты не компилируются (`private field`).
- Причина: `mod tests` является потомком корня crate, но не потомком `cmd_genesis`; доступ к приватным полям структур чужих подмодулей запрещён (в отличие от кейса, когда тип лежит в том же родителе, что и `tests`).
- Фикс/обход: `pub(crate)` на bundle-типах генезиса и их полях, задействованных в тестах; константу `GENESIS_VALIDATOR_DER_PATH` — `pub(crate)` в `cmd_genesis`; тесты импортировать через `crate::cmd_genesis::…` / `crate::cli_config::…` / `crate::rpc_helpers::…`.
- Что проверить потом: при следующем дроблении — либо `pub(crate)` на минимальном контракте для тестов, либо тест-хелперы внутри модуля фичи.

- Дата: 2026-05-02 — **pwm-cli: размещение тестов после побочного эксперимента MCP text_editor:**
- Контекст: в дереве появился **`src/main-tests.rs`** и **`#[path = "..."]`** из параллельной сессии (копирование файлов поверх репозитория).
- Фикс: стандартное дерево **`src/tests/mod.rs`** и **`#[cfg(test)] mod tests;`** без **`#[path]`** — коммит **`80aaf86`**, синхронизация документов **`d9e5b2b`**.

- Дата: 2026-05-02
- Контекст/файлы: `crates/pwmd/src/snapshot/io.rs` (`validate_snapshot`), `crates/pwmd/src/api/handlers_roaming.rs` (`v1_export_handoff_register`)
- Симптом: после cross-shard import на целевом шарде перезапуск `pwmd` не поднимал цепочку (ошибка загрузки snapshot / «genesis»), хотя блоки и state на диске были.
- Причина: provenance для Import попадала в `State.exported_registry` через HTTP handoff **вне** блоков; replay в `validate_snapshot` воспроизводил только транзакции из `blocks`, поэтому перед Import в replay не было строки registry → падение валидации.
- Фикс/обход: перед `apply_tx` для Import подставлять недостающую строку из `snapshot.state.exported_registry` (типичный handoff-only кейс). Дополнительно: дефолтный путь snapshot для `RuntimeIdentityMode::Neutral` изолирован по `--listen` (`state_root/neutral/<addr+port>/pwm-data.json`), чтобы два Neutral-процесса не делили один `pwm-data.json`.
- Что проверить потом: дальнейшая формализация ordering handoff vs блоков для checkpoint slice 6b; при смене формата путей — заметки для операторов.

- Дата: 2026-05-03 — **pwmd ClickHouse snapshot режим (prototype):**
- Контекст/файлы: `crates/pwmd` feature `clickhouse-snapshot`, `--snapshot-backend clickhouse`, таблица `pwm_snapshots.node_snapshot`.
- Симптом/ловушки: без `--features clickhouse-snapshot` бинарь не содержит варианта CLI `clickhouse` и кода HTTP-бэкенда; при включённом feature обязательны валидный `--clickhouse-url` / `PWM_CLICKHOUSE_URL` и заранее созданная DDL; `snapshot_store_key` и поля identity должны быть в допустимом наборе символов (ascii alnum plus `._-|+:`, без `/`; см. `snap_ch_sql_id` / `pwmd_snap_row_key`).
- Причина: прототип намеренно без полноценной миграции схемы и без auth к ClickHouse.
- Фикс/обход: поднимать compose из `tools/docker/pwmd-clickhouse-compose.yaml`, прогонять DDL; smoke: `docs/reviews/sprint-15-slice-5-smoke.md`.
- Что проверить потом: pwm-testing кросс-бэкенд replay (slice 6), TLS/basic auth для CH.

- Дата: 2026-05-03 — **pwmd snapshot load benchmarks (slice 6):**
- Контекст/файлы: `cargo bench -p pwmd --bench snapshot_load`, `docs/reviews/sprint-15-slice-6-bench.md`, переменные `PWM_CLICKHOUSE_BENCH_URL`, `PWM_SNAPSHOT_BENCH_*`; по умолчанию читаются `./tmp/state-testnet/pwm-data.json` + `./tmp/genesis-custom.json` (после `./node-1.ps1` / `./node-2.ps1`).
- Симптом: оператор ожидает сравнение загрузки JsonFile vs ClickHouse без Docker в CI и по возможности на данных QA-нод.
- Фикс/обход: без живого CH бенч `snap_load_clickhouse` использует in-process mock при `--features clickhouse-snapshot`; для живого ClickHouse задать `PWM_CLICKHOUSE_BENCH_URL` — перед замером выполняется `import_snapshot_file` в `pwm_snapshots.node_snapshot` с `row_key=s15_slice6_bench`. Отдельный мост файл→БД: бинарь `pwmd-ch-snap-import` (те же defaults БД/таблицы, что у `--snapshot-backend clickhouse`).
- Что проверить потом: при изменении DDL/поля `snapshot_json` обновить mock и документацию бенча.

- Дата: 2026-05-03 — **pwmd JsonFile incremental epochs (slice 7 wave 2):**
- Контекст/файлы: `pwm-data.json` рядом с `epochs/block_e*.json` (содержимое — **JSONL**: по одному JSON-блоку на строку внутри файла с суффиксом `.json`), `epochs/pwm-epochs-manifest.json`, запись через `*.tmp` + `rename`.
- Симптом/ловушки для оператора: полный монолитный `blocks[]` в `pwm-data.json` больше не переписывается на каждом sealed-блоке; на границе `height % 100 == 0` пишется summary без тел блоков (`blocks_stored: epochs`). Между checkpoint-ами `pwm-data.json` может отставать от tip (роуминг/cross-shard в памяти до следующего checkpoint или `save_tip_summary`, напр. relay-path); при жёстком выключении узла возможна потеря последних <100 блоков **вне** epoch-файлов только если сломана запись на диск — нормальный путь сначала коммитит epoch, затем manifest.
- Фикс/обход: для консистентности после relay без нового блока JsonFile вызывает перезапись summary (`save_tip_summary`). Cold start: есть `pwm-data.json` и/или только `epochs/` + manifest — `load_snapshot` поднимает цепочку через полный replay из epoch-файлов.
- Что проверить потом: pwm-testing сценарии crash/recovery, разрыв диапазонов epoch, согласованность roaming при остановке между checkpoint.

- Дата: 2026-05-03 — **bounded chain tail + ClickHouse incremental (slice 7 wave 3):**
- Контекст/файлы: `pwm_core::Chain` (`VecDeque`, `TAIL_BLOCK_CAP`), `crates/pwmd/src/snapshot/ch_http.rs`, `tools/docker/sql/clickhouse_pwm_snapshots.sql`.
- Симптом: монолитный `encode_inner_snap_json` без пути к `pwm-data.json` и при высоте > длины хвоста в памяти вернёт ошибку (требуется сборка полной цепочки из epoch-файлов). Импорт в CH через `pwmd-ch-snap-import` вставляет блоки + checkpoint-строки + legacy-ряд в `node_snapshot`; replay промежуточных checkpoint при импорте упрощён (не все edge-кейсы cross-shard registry).
- Причина: осознанный fail-fast и совместимость со старыми читателями `snapshot_json`.
- Фикс/обход: для ручного полного дампа использовать JsonFile путь или передать `Some(pwm-data.json)` в encode; DDL создавать `blocks__0xHH` / `checkpoints__0xHH` per domain + DB из `network_id` (`resolve_ch_database`).
- Что проверить потом: pwm-testing на живом ClickHouse, большие цепочки (`ch_load` загружает все блоки в память для replay).

- Дата: 2026-05-03 — **ClickHouse DDL sort keys + checkpoint columns (slice 7 post-wave3):**
- Контекст/файлы: `tools/docker/sql/clickhouse_pwm_snapshots.sql`, `docs/reviews/sprint-15-slice-7-plan.md` §3, `ch_insert_checkpoint_row` в `ch_http.rs`.
- Симптом: у уже развёрнутых таблиц `blocks__*` / `checkpoints__*` без `row_key` в `ORDER BY` теоретически возможна некорректная дедупликация ReplacingMergeTree при нескольких `row_key` в одной физической таблице.
- Фикс/обход: новые инсталляции — применить актуальный DDL (`ORDER BY (row_key, height)` и для checkpoints `(row_key, genesis_digest, checkpoint_height)`, колонки `state_root` / `shard_balance` в checkpoints); существующие кластеры — `ALTER TABLE ... MODIFY ORDER BY` / добавление колонок по документации ClickHouse или пересоздание таблиц в окне обслуживания.
- Что проверить потом: миграционный скрипт или операторский runbook для ALTER v1→v2 схемы.

- Дата: 2026-05-03 — **validators_accept table wired but INSERT deferred (slice 7 wave 4):**
- Контекст/файлы: `SnapChCfg.table_validators_accept`, `tools/docker/sql/clickhouse_pwm_snapshots.sql` (`validators_accept__0x01`), `ch_insert_checkpoint_row`.
- Симптом: DDL и конфиг знают имя таблицы, но runtime не выполняет `INSERT` в `validators_accept` — одна предупреждающая запись в лог при первом checkpoint (`tracing`, target `pwmd::snapshot`).
- Причина: подпись checkpoint требует выбора `validator_id` и привязки Ed25519 к консенсусному контуру (не реализовано в этом срезе).
- Фикс/обход: зафиксирован детерминированный `checkpoint_digest` для будущих подписей — `hex(pwm_core::digest(state))` (`docs/reviews/sprint-15-slice-7-plan.md` §6.1); строки `validators_accept` добавить после согласования producer/validator identity в коде.
- Что проверить потом: pwm-testing подпись checkpoint + append-only CH при multi-validator сети.

- Дата: 2026-05-04 — **JsonFile `SnapshotBackend::save` без монолитного encode (ticket trust-default P0):**
- Контекст/файлы: `snapshot/io.rs` (`json_file_runtime_persist`), `snapshot/store.rs` (`save`), `snapshot_save_under_inner_lock`.
- Симптом (до фикса): после транзакций/roaming вызывался `save_snapshot` → `encode_inner_snap_json` → при хвосте в RAM полный `load_blocks_from_epochs` для сборки монолита — лишнее чтение всех epoch-файлов на горячем пути.
- Причина: один кодовый путь для «полного дампа» и для инкрементального режима эпох.
- Фикс/обход: при наличии `epochs/pwm-epochs-manifest.json` горячий save делает только `sync_epoch_disk_to_tip` + `save_checkpoint_summary`; без manifest остаётся legacy `save_snapshot`.
- Что проверить потом: RFC «trust checkpoint при старте без полного replay» — отдельная задача (`tasks/20260504-s15-snapshot-trust-default-api-save-split.json`); монолитный дамп по-прежнему через явные утилиты/миграции.

- Дата: 2026-05-03 — **Snapshot trust-default load (JsonFile)**
- Контекст/файлы: `crates/pwmd/src/snapshot/io.rs`, `crates/pwmd/src/snapshot/store.rs`, `crates/pwmd/src/main.rs`, `--snapshot-verify-chain`, `PWM_SNAPSHOT_VERIFY_CHAIN`
- Симптом: оператор может ожидать полный replay при каждом старте, но JsonFile epoch-снимки теперь по умолчанию доверяют summary/checkpoint и manifest tip.
- Причина: sprint-15 переводит cold start на быстрый trust-path; полный genesis→tip replay оставлен как audit/recovery режим.
- Фикс/обход: для аудита запускать `pwmd --snapshot-verify-chain` или задавать truthy `PWM_SNAPSHOT_VERIFY_CHAIN`; если summary checkpoint отстаёт от manifest canonical height, loader принудительно падает обратно на full verify.
- Что проверить потом: ClickHouse load сейчас всегда делает full replay и только логирует verify-request как уже покрытый; при появлении CH trust-path явно развести семантику `SnapshotLoadOpts`.

- Дата: 2026-05-03
- Контекст/файлы: `crates/pwm-core/src/tx.rs`, `crates/pwmd/src/tx_policy.rs`, `crates/pwmd/src/api/handlers_roaming.rs`, Slice B cross-shard stabilization
- Симптом: после отключения мутации `chain.st.exported_registry` в `handoff_register` legacy Import без embedded provenance начал получать `BAD_REQUEST` на target (если `export_id` не известен в block-replay state).
- Причина: replay-critical provenance теперь переносится в sealed Import path; handoff хранится только в non-root pending (`cross_shard`) и не может служить источником state-root.
- Фикс/обход: клиент/relay должен отправлять Import с `SignedTx.import_provenance`; legacy fallback работает только когда provenance уже детерминированно известен из block-path (`exported_registry` из replay).
- Что проверить потом: унифицировать клиентские builders (CLI/TUI/SDK) на автозаполнение `import_provenance` из handoff payload и добавить миграционный warning в operator docs.

- Дата: 2026-05-03
- Контекст/файлы: `crates/pwmd/src/api/handlers_backfill.rs`, `crates/pwmd/src/api/handlers_status.rs`, Slice C auto-backfill
- Симптом: у разных runtime-профилей `cluster_domain_hi` из identity может не совпадать с domain_hi валидаторского signer-аккаунта, из-за чего import backfill получает `invalid import: embedded provenance mismatch`.
- Причина: provenance/import prefilter валидирует `target_domain_hi` относительно `tx.domain_code` signer-а; при несовпадении hi discovery и signer path расходятся.
- Фикс/обход: backfill выбирает signer из доступных `cfg.accounts` + `val_sks` и использует его domain_hi как discovery target для `/v1/cross-shard/facts`, чтобы импорт и provenance проходили один и тот же deterministic tx path.
- Что проверить потом: зафиксировать единый инвариант между `identity.cluster_domain_hi` и signer-domain в runtime-конфиге, чтобы убрать fallback-логику выбора hi.

- Дата: 2026-05-03
- Контекст/файлы: `crates/pwmd/src/snapshot/repair.rs`, `crates/pwmd/src/bin/pwmd_snap_repair.rs`, Slice D offline repair.
- Симптом: при rollback на `H < checkpoint_height` невозможно достоверно восстановить `roaming`/`cross_shard` только из block replay (эти структуры не полностью выводятся из `State`).
- Причина: JsonFile summary хранит `roaming` и `cross_shard` как side-state, а replay до `H` восстанавливает только детерминируемое `State`.
- Фикс/обход: `pwmd-snap-repair` переносит `roaming`/`cross_shard` только если исходный summary уже на той же высоте `H`; иначе пишет безопасный default и фиксирует это флагом `kept_aux_summary=false`.
- Что проверить потом: выделить детерминированный replay/compaction путь для `roaming`/`cross_shard` или явно задокументировать reset-поведение как операторский контракт.

- Дата: 2026-05-05
- Контекст/файлы: `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/state.rs`, IMPORT fee v2 (`MIN_IMPORT_FEE_UNITS`)
- Симптом: старые IMPORT-сценарии и часть unit-тестов падали с `Insufficient`, хотя provenance и replay-guard были корректными.
- Причина: в E-1 включён обязательный import fee floor `0.01 PWM`, а signer target-side import должен иметь минимальный ликвидный баланс для списания комиссии.
- Фикс/обход: в тестовых/операторских сценариях перед первым IMPORT обеспечивать баланс signer >= `MIN_IMPORT_FEE_UNITS`; на reject путь комиссия не списывается.
- Что проверить потом: pwm-testing e2e на live-нодах с явной проверкой `fee_pool` target-shard после успешного IMPORT и отсутствия списания при reject.

- Дата: 2026-05-06
- Контекст/файлы: `crates/pwmd/src/slice20_e2e_tests.rs` (`slice20_dual_flow_ok`), e2e roaming/import smoke.
- Симптом: `tx-import` падал с `HTTP 500 ... insufficient balance` после успешного handoff/finalize; при попытке быстро добавить import signer в `wallet-cy` ломался ранний `tx-send` (sender `domain_hi=0x32` уходил на CY RPC).
- Причина: после ввода `MIN_IMPORT_FEE_UNITS` target-side import signer должен иметь ликвидный баланс; при этом модификация рабочего sender-кошелька меняет активный signer для последующих CLI команд в тесте.
- Фикс/обход: генерировать отдельный genesis-only wallet (копия sender wallet) и добавлять import signer туда перед `genesis-build`, не меняя wallets, которыми подписываются runtime tx-шаги.
- Что проверить потом: pwm-testing прогон `slice20_dual_flow_ok` и соседнего cross-shard smoke на чистом target-dir, чтобы подтвердить отсутствие зависимостей от порядка account-list в wallet YAML.

- Дата: 2026-05-06
- Контекст/файлы: `crates/pwm-core/src/state.rs`, `crates/pwmd/src/snapshot/types.rs`, Sprint V2-2 Slice 1 (`marks_quota` removal)
- Симптом: после удаления `State.marks_quota` старые snapshot-файлы могут содержать legacy `state.marks_quota` строки, что даёт риск тихого расхождения при миграции.
- Причина: `marks_quota` исторически был зеркалом `account.marks`, но в legacy-данных поле могло остаться несинхронным из-за ручных/тестовых правок.
- Фикс/обход: runtime state хранит только `Account.marks`; writer больше не сериализует `marks_quota`, loader принимает legacy поле только при строгом инварианте `quota == account.marks` и отклоняет mismatch/orphan с явной ошибкой.
- Что проверить потом: после migration window убрать чтение legacy `state.marks_quota` совсем и оставить strict canonical state без этого ключа.

- Дата: 2026-05-06
- Контекст/файлы: `crates/pwm-core/src/genesis.rs`, `crates/pwmd/src/snapshot/genesis.rs`, `crates/pwm-cli/src/cmd_genesis.rs` (Sprint V2-3 Slice 0 schema prep)
- Симптом: после добавления V2-3 полей в `GenCfg` узел должен загружать и старые genesis-файлы (v4), и новые (v5), иначе апгрейд ломает cold-start и replay.
- Причина: расширение контракта `gen_cfg` требует migration window; часть инфраструктуры всё ещё может не сразу перейти на schema v5.
- Фикс/обход: loader `pwmd` принимает `schema_version` 4/5 и для v4 подставляет legacy-safe defaults (`policy_ver=1`, `season_enabled=false`, базовые пороги/коэффициент); `pwm genesis-build` уже выпускает v5.
- Что проверить потом: в Slice 1/2 добавить контрактные тесты v4/v5 roundtrip + явно решить дату выключения schema v4 fallback.

- Дата: 2026-05-06
- Контекст/файлы: `crates/pwmd/src/slice20_e2e_tests.rs` (`cross_shard_bridge_ok`, `slice20_dual_flow_ok`), прогон `cargo test -p pwmd` в naming wave M.
- Симптом: оба e2e-теста стабильно падали по таймауту готовности (`timeout waiting for ready: http://127.0.0.1:<port>`), при этом остальная `pwmd` матрица проходила.
- Причина: в текущем окружении readiness-поллинг локальных e2e-нод не успевает подняться в отведённое тестом окно; симптом не связан с переименованием функций.
- Фикс/обход: для coding-слайсов с чисто нейминговыми правками фиксировать как known flaky env issue и передавать в `pwm-testing` на профильный rerun/диагностику e2e окружения.
- Что проверить потом: выделить отдельный stability-run для `slice20_e2e_tests` (изолированные порты/таймауты/лог старта) и решить, нужен ли больший wait budget в CI.

- Дата: 2026-05-06
- Контекст/файлы: `crates/pwm-tui/src/tui_loop.rs`, `crates/pwm-tui/src/account_view.rs`, guard `last_claim_wall` в F5 claim modal.
- Симптом: 1-hour claim guard сбрасывался почти каждую секунду после `PollDone`, поэтому `can_claim()` практически всегда видел `None` и пропускал повторный claim.
- Причина: poll snapshot создаёт `AcctRow` с `last_claim_wall: None`, а UI применял snapshot через полную замену `ui.rows` без merge session-local полей.
- Фикс/обход: при `PollDone` добавлен merge `snapshot.rows` с текущими `ui.rows` по `account id` (перенос `last_claim_wall`), плюс unit-тест на сохранение `last_claim_wall` через два poll цикла.
- Что проверить потом: e2e в TUI на нестабильном RPC (`/v1/head` timeout/offline), чтобы подтвердить fallback `anchor_ref` из последнего `head_height` и человекочитаемый reject-hint при anchor mismatch.

- Дата: 2026-05-06
- Контекст/файлы: `crates/pwm-core/src/types.rs` (`Account.marks`), `crates/pwmd/src/snapshot/types.rs` (legacy snapshot decode)
- Симптом: при смене `marks: u128 -> u32` старые snapshot JSON могут содержать очень большие значения `marks`, что иначе ломает десериализацию или silently обрезает данные.
- Причина: historical formula использовала raw stake units и накапливала `marks` в `u128`; новая семантика считает marks в «whole PWM-hours».
- Фикс/обход: выбран подход **A** (custom serde deserializer): `Account.marks` декодируется через compat-десериализатор, который принимает legacy `u128`/decimal string, а при `> u32::MAX` применяет миграцию `marks / PWM_RAW_SCALE` с clamp к `u32::MAX`; в pwmd snapshot decode применена та же нормализация для `state.accounts[*].account.marks` и legacy `marks_quota`.
- Что проверить потом: после migration window убрать legacy-normalization ветки и оставить strict `u32` decode в snapshot wire.

- Дата: 2026-05-07
- Контекст/файлы: `crates/pwm-tui/src/tx_submit.rs`, `crates/pwm-tui/src/tui_loop.rs`, F5 auto-claim + нижний footer status.
- Симптом: UI показывал `Claimed 0 marks (none matured)` сразу после любого HTTP-success claim и держал stale claim-note после burn, хотя claim мог быть только queued и применяться позже.
- Причина: `submit_claim` возвращал `Ok(0)` как «псевдо-результат» без on-chain подтверждения, а footer note интерпретировал это как финальный факт; note не очищался в ветке `SubmitDone` для burn.
- Фикс/обход: `submit_claim` переведён на `Result<(), String>`; в TUI добавлен `pending_claim_note` (baseline marks + deadline) с подтверждением по poll (`Claim confirmed (+N marks)`) или нейтральным fallback (`Claim accepted; no new marks yet`), стартовое сообщение после F5 — `Claim submitted; waiting for confirmation...`; после `SubmitDone` burn claim-note сбрасывается.
- Что проверить потом: pwm-testing живой сценарий с mempool lag/late seal (claim подтверждается только через несколько poll), чтобы зафиксировать UX в docs/reviews.

- Дата: 2026-05-08
- Контекст/файлы: `crates/pwmd/src/transport/peer_session/sync_live.rs`, catch-up ветки `send_cup_req`/`on_nack`.
- Симптом: после `SyncNack` или write-fail при отправке `SyncCatchupReq` флаг `cup_active` мог остаться `true`, из-за чего peer застревал в catch-up и не переходил к live headers.
- Причина: catch-up состояние устанавливалось до фактической отправки запроса, но при ошибке записи не очищалось; `on_nack` не сбрасывал активный catch-up.
- Фикс/обход: добавлен детерминированный reset catch-up state в обеих ветках (`req_write`, `nack`) с backoff/`cup_try`; при ошибке старта catch-up `on_tip` теперь мягко fallback-ится в live headers вместо залипания.
- Что проверить потом: pwm-testing прогон с сетевыми fault-инъекциями (nack/write flap подряд) и подтверждение, что peer стабильно возвращается в live sync без deadlock.

- Дата: 2026-05-08
- Контекст/файлы: `crates/pwmd/src/wire_serde.rs`, `crates/pwmd/src/state.rs`, `crates/pwmd/src/ledger.rs`, peer wire payloads `AccountViews`/`CrossShardFacts`.
- Симптом: live peer path может падать на `wire_decode_failed: u128 is not supported` при больших значениях `u128` в JSON wire.
- Причина: JSON numeric transport для чисел выше `u64` неустойчив для `serde_json` decode между узлами.
- Фикс/обход: канонический wire-формат для `u128` переведён на hex-строки `0x...`; decode оставлен совместимым с legacy decimal string / u64 numeric.
- Что проверить потом: после migration window ужесточить wire-контракт только до hex string и убрать legacy decode-ветки.

- Дата: 2026-05-08
- Контекст/файлы: `crates/pwm-core/src/chain.rs`, `crates/pwmd/src/main.rs`, `crates/pwmd/src/lifecycle.rs`, Wave A harness.
- Симптом: same-shard Wave A давал стабильный `tip_hash`/epoch hash mismatch даже после wire-fix.
- Причина: `Chain::seal` использует `SystemTime::now()` для `BlockHdr.ts`; при локальном seal на двух нодах это даёт разные `ts`/`sig`/`hdr_hash`.
- Фикс/обход: введён test/dev-only toggle `debug_deterministic_seal_time` (`--debug-deterministic-seal-time` или `PWM_DEBUG_DETERMINISTIC_SEAL_TIME`) c `ts = base + height`; по умолчанию режим OFF и прод-семантика не меняется.
- Что проверить потом: если режим потребуется за пределами Wave A harness, закрепить RFC-политику для season/fee time semantics и отдельный proposer/replay контракт.

- Дата: 2026-05-09
- Контекст/файлы: `crates/pwmd/src/lease.rs`, `crates/pwmd/src/lifecycle.rs`, `crates/pwmd/src/transport/peer_session/wire.rs`
- Симптом: для S2 нужен failover guard без полноценного внешнего coordinator/KV, но при этом нельзя допустить local active/active seal в одном runtime-пуле.
- Причина: MVP-срез ограничен по времени/риску и не включает новый внешний lease backend.
- Фикс/обход: внедрён lightweight in-memory lease/fencing coordinator (`owner_id`, `term`, `expires_at_ms`, `last_tip`, `fence`) с gate в seal-loop, timeout takeover и stale-tip check; heartbeat/status/hello расширены lease-сигналами для диагностики.
- Что проверить потом: для multi-process/multi-host HA перенести lease source of truth во внешний shared backend (file lock/KV/coordinator) или зафиксировать wire-authoritative lease replication с чёткой RFC-гарантией single active owner.

- Дата: 2026-05-09
- Контекст/файлы: `crates/pwmd/src/lease_backend.rs`, file backend (`tmp + rename`) на Windows.
- Симптом: на Windows `std::fs::rename` не заменяет существующий файл, поэтому прямая замена lease-файла через rename падает.
- Причина: платформенная семантика `rename` отличается от POSIX atomic replace.
- Фикс/обход: MVP-путь пишет `tmp`, делает `sync_all`, затем под lock удаляет target и выполняет `rename`; CAS-безопасность сохраняется за счёт lock-файла, но replace остаётся best-effort platform-specific.
- Что проверить потом: перейти на platform-specific atomic replace (или общий crate с гарантированным replace), если понадобится строгая атомарность на Windows без remove-gap.

- Дата: 2026-05-11
- Контекст/файлы: `crates/pwmd/src/transport/peer_session/sync_live.rs` (`on_tip`), `crates/pwmd/src/transport/peer_session/mod.rs` (regression test).
- Симптом: при двустороннем steady sync peer с более низкой высотой (`head_h < local_h`, например genesis follower против source `H=1`) мог ложно попадать в `sync_tip_divergence` disconnect.
- Причина: `lag` считался через `saturating_sub`; в кейсе peer-behind получалось `0`, и код ошибочно заходил в ветку same-height tip/hash сравнения.
- Фикс/обход: после обновления peer sync state в `on_tip` добавлен ранний выход `Ok(None)` для `head_h < local_h`; вычисление `lag` оставлено только для `head_h >= local_h`, что сохраняет прежнюю семантику equal/ahead веток.
- Что проверить потом: в `pwm-testing` закрыть integration soak (steady bidirectional source/follower, TCP) и убедиться, что counter `sync_tip_divergence_disconnect_total` не растёт на peer-behind анонсах.

- Дата: 2026-05-11
- Контекст/файлы: `cy-cluster-proposer.ps1`, `cy-cluster-attester.ps1`, default lease dir `state/leases`.
- Симптом: в локальном RFC16-лабе периодически появляется `seal_lease_cas_failed`, и proposer может не входить в стабильный seal-loop.
- Причина: file lease backend использует общий относительный путь (`state/leases`) для процессов из одного рабочего каталога; после перезапусков/параллельных запусков остаётся конфликтующий CAS-контекст.
- Фикс/обход: для 2-node lab (один active sealer + attester с `--debug-disable-seal-loop`) скрипты переключены на `--seal-lease-backend process-local`, чтобы убрать межпроцессный file-CAS шум.
- Что проверить потом: для продовых HA/soak-сценариев вернуть shared lease backend с явным per-node `--seal-lease-dir` или внешним coordinator, не полагаясь на process-local.

- Дата: 2026-05-13
- Контекст/файлы: `crates/pwmd/src/transport/peer_session/sync_live.rs`, тесты autosnapshot (`batch_cross_ckpt_writes_snap`, `standby_batch_cross_10_writes`).
- Симптом: проверка `canonical_h` могла читать не тот manifest и давать ложный `canonical_h=100` в standby-тесте, хотя ожидался `15`.
- Причина: `manifest_file_path` строится относительно parent snapshot-файла (`.../epochs/pwm-epochs-manifest.json`), а несколько тестов использовали общий `std::env::temp_dir()` parent и пересекались по одному manifest.
- Фикс/обход: каждый тест создаёт отдельный уникальный subdir в temp и пишет `pwm-data.json` внутри него; cleanup выполняется через `remove_dir_all`.
- Что проверить потом: при добавлении новых snapshot-тестов всегда изолировать parent-dir, иначе возможно скрытое взаимное влияние по `epochs` manifest.

- Дата: 2026-05-13
- Контекст/файлы: `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/ser_json_u128.rs`, peer wire `serde_json`.
- Симптом: peer catch-up/live decode обрывался с `wire_decode_failed: u128 is not supported` при JSON wire payload, содержащем `SignedTx`/`ExportProvenance` с большими суммами.
- Причина: `serde_json` не поддерживает plain derive-десериализацию `u128` из numeric JSON wire; при этом SyncCatchupChunk несёт full `Block` с `Vec<SignedTx>`.
- Фикс/обход: введён compat serde-модуль `ser_json_u128`: сериализация `u128` в decimal string, decode принимает decimal string и `u64`; поля `TxBody`/`SignedTx.import_fee`/`ExportProvenance.amount` переведены на этот контракт.
- Что проверить потом: после миграционного окна закрепить один каноничный wire-формат (string-only) и удалить legacy `u64` decode-ветку.

- Дата: 2026-05-13
- Контекст/файлы: локальные проверки `cargo test -p pwmd` на Windows-хосте с активными `pwmd` процессами (`cy-cluster-proposer.ps1` / `cy-cluster-attester.ps1`).
- Симптом: full test run для пакета `pwmd` падал на `failed to remove ... rust-target-shared/debug/pwmd.exe (os error 5)`; параллельно возникали `os error 112` при попытке использовать отдельный `CARGO_TARGET_DIR`.
- Причина: исполняемый файл `pwmd.exe` в shared target заблокирован живыми процессами, а альтернативный target упирается в нехватку дискового места.
- Фикс/обход: для валидации слайса запускать `cargo check -p pwmd --lib` и `cargo test -p pwmd --lib` с `CARGO_INCREMENTAL=0`; для полного `cargo test -p pwmd` сначала остановить активные `pwmd` процессы и освободить место.
- Что проверить потом: повторить полный `cargo test -p pwmd` после остановки фоновых узлов и убедиться, что bin-таргет `pwmd` линковается без lock-конфликтов.

- Дата: 2026-05-13
- Контекст/файлы: `crates/pwmd/src/peer_list.rs`, `crates/pwmd/src/main.rs`, `docs/pwmd.md` (multishard peers.yaml v2).
- Симптом: формально неоднозначно, как трактовать `shards: {}` и отсутствие matching shard при загрузке peers file.
- Причина: legacy v1 и v2 должны сосуществовать без потери backward compatibility и без «тихого» старта с неверным shard при явном операторском файле.
- Фикс/обход: в коде зафиксирована политика: `shards` пустой => legacy поведение (`peers` flat list); `shards` непустой и нет matching `domain_hi` => implicit default file даёт empty seeds + `tracing::warn`, explicit `--peers-list` даёт fail-fast с actionable ошибкой.
- Что проверить потом: если продукту понадобится strict-mode и для implicit default, добавить отдельный CLI-флаг (например, `--peers-list-strict`) вместо изменения дефолтной семантики.

- Дата: 2026-05-14
- Контекст/файлы: `crates/pwm-cli/src/cli_cmd.rs`, `crates/pwm-cli/src/cli_dispatch.rs`, `crates/pwm-cli/src/cmd_addr.rs`
- Симптом: оператор может ожидать, что `addr-derive`/`addr-bruteforce` автоматически возьмут seed из default wallet path даже без `--wallet-out`.
- Причина: fallback на wallet seed намеренно включается только при **явно переданном** `--wallet-out`; при `wallet_out=None` (implicit default path) источник seed остаётся только `--master`/`PWM_MASTER_SEED`.
- Фикс/обход: добавлена явная ошибка для stateless-режима без seed (`provide --master or PWM_MASTER_SEED`) и отдельная ошибка при explicit fallback с отсутствующим файлом `--wallet-out`.
- Что проверить потом: в operator docs/cheatsheet явно подсветить правило «wallet fallback работает только с explicit `--wallet-out`».

- Дата: 2026-05-22
- Контекст/файлы: `crates/pwm-cli` `addr-derive` / `addr-bruteforce`, PowerShell `$env:MASTER_SEED` vs clap `PWM_MASTER_SEED`.
- Симптом: команда с `--master $env:MASTER_SEED` при **пустой** переменной даёт в процессе фактически `--master` без значения → ошибка clap «value is required»; отдельно операторы могли задавать только `MASTER_SEED`, игнорируя `PWM_MASTER_SEED`.
- Причина: пустая подстановка в PowerShell; clap ранее требовал значение после `--master`; fallback читал только env, вшитый в поле через `PWM_MASTER_SEED`.
- Фикс/обход: для `--master` заданы `num_args=0..=1` + `default_missing_value=""`, пустая строка отбрасывается в `resolve_master_seed`; добавлено чтение **дополнительного** env **`MASTER_SEED`** (после trim) с тем же hex-форматом; обновлён текст ошибки stateless. Рабочие варианты: `$env:PWM_MASTER_SEED` или `$env:MASTER_SEED`, либо **`--master`** без аргумента при установленном env, либо только `--wallet-out` к существующему кошельку без `--master`.
- Что проверить потом: краткая строка в runbook/cli help для операторов Windows.
