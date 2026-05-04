# `pwmd`: техническая документация

`pwmd` — devnet-нода PWM. Она поднимает HTTP API, держит in-memory `Chain` + `Mpool`, запускает фоновый цикл `seal` и сохраняет/восстанавливает состояние через JSON snapshot.

## Роль в системе и границы

**Роль `pwmd`**
- runtime-обвязка вокруг `pwm-core`: сеть (HTTP), lifecycle процесса, конфиг и персистентность;
- единая RPC-точка для клиентов (`pwm-cli`, `pwm-tui`) и внешних интеграций;
- периодическое формирование блоков из mempool в фоне.

**Граница с `pwm-core`**
- `pwm-core` реализует детерминированную доменную логику: типы tx/блоков, `validate_tx_shape`, `State::apply_tx`, `Chain::seal`, `Mpool`;
- `pwmd` не переопределяет правила консенсуса/валидности, а вызывает API `pwm-core`.

**Граница с клиентами**
- `pwm-cli` и `pwm-tui` инициируют запросы к `pwmd`, но не хранят chain-state;
- `PWM_RPC` задает базовый URL для клиентов (по умолчанию `http://127.0.0.1:3030`) и должен указывать на запущенный `pwmd`.

## Runtime-конфиг

## `PwmdConfig`

Структура `PwmdConfig` содержит runtime-параметры:
- `listen: SocketAddr` — адрес bind HTTP-сервера;
- `genesis: GenesisSource` — источник genesis (`DevNet` или `JsonFile(PathBuf)`);
- `data_file: PathBuf` — путь JSON snapshot-файла.
- `snapshot_verify_chain: bool` — audit/recovery режим полной replay-проверки epoch snapshot при загрузке; по умолчанию выключен.
- `shard: ShardId` — deprecated compat alias (`A|B`), сохранен для мягкой обратной совместимости.
- `identity: RuntimeIdentity` — эффективный launch identity tuple (`network_id`, `cluster_domain_hi`, `cluster_id`, `node_id`) + режим (`explicit` или `alias`).
- `transport: TransportConfig` — stateful peer transport (`peer_listen`, seed peers, connect/handshake/heartbeat timeout, retry knobs).

`PwmdConfig::default()`:
- `listen = 127.0.0.1:3030`;
- `genesis = DevNet`;
- `data_file = state/pwm-data.json` (neutral relay-baseline default, без shard-specific namespace);
- `identity` поднимается в relay-baseline профиле; shard-enforced semantics включаются только при explicit domain-конфиге.

## CLI-флаги `pwmd`

Бинарник `crates/pwmd/src/main.rs` мапит аргументы в `PwmdConfig`:
- `--shard <A|B>` (без implicit default) — deprecated compat alias для legacy namespace/identity path; не является primary domain-first UX;
- `--listen <ADDR>` (default `127.0.0.1:3030`);
- `--genesis-file <PATH>` (если не задан, используется встроенный `dev_net()`);
- `--state-root <DIR>` (default `state`, используется для default data path);
- `--data-file <PATH>` (по умолчанию `state/<effective-state-namespace>/pwm-data.json`);
- `--network-id <STRING>`;
- `--domain-hi <u8|0xNN>`;
- `--domain-cluster <u8|0xNN>` (primary alias для `--domain-hi`);
- `--cluster-domain-hi <u8|0xNN>` (deprecated compat alias);
- `--cluster-id <STRING>`;
- `--node-id <STRING>`.
- `--transport-real` — включить real transport loop (по умолчанию выключен, используется legacy stub transport loop);
- `--transport-peer-listen <ADDR>` (`PWM_PEER_LISTEN`) — выделенный TCP listener для peer-сессий; если не задан, используется fallback `rpc_port + 100`;
- `--transport-peer-seed <ADDR[,ADDR...]>` — список peer seed socket-адресов (`host:peer_port`) для stateful TCP сессий;
- `--transport-connect-timeout-ms <N>`;
- `--transport-handshake-timeout-ms <N>`;
- `--transport-retry-base-ms <N>`;
- `--transport-retry-max-ms <N>`.
- `--transport-soak-counter-cap <N>` — верхняя граница долгоживущих soak-счетчиков transport/churn.
- `--transport-soak-health-interval-ticks <N>` — период агрегации health snapshot в тиках (`0` отключает).
- `--transport-runaway-streak-limit <N>` — лимит подряд retryable тиков до safety stop.
- `--transport-runaway-cooldown-ms <N>` — cooldown safety stop при runaway reconnect.
- `--log-name <STRING>` (default `pwmd`) — имя лог-потока для template placeholders.
- `--log-dir <DIR>` (default `logs`) — корневой каталог файловых логов.
- `--log-file-template <REL_PATH>` (default `{date}/{log_name}-{node_id}-{time}.log`) — относительный путь внутри `log-dir`, поддерживает `{date}`, `{time}`, `{datetime}`, `{log_name}`, `{node_id}`, `{pid}` и `~UT` (`HH:MM:SS.mmm`, UTC).
- `--log-file <on|off|required>` (default `on`) — режим файлового sink:
  - `on`: best-effort, при ошибке файлового sink старт продолжается с console-only логированием;
  - `off`: только console sink;
  - `required`: ошибка файлового sink останавливает старт.
- `--peer-log-file <on|off|required>` (default `on`) — отдельный sink для transport/peer событий (`target=pwmd::peer`), в основной консоль/файл не дублируется.
- `--peer-log-file-template <REL_PATH>` (default `{date}/pwmd-peer-{node_id}-{time}.log`) — шаблон отдельного peer log файла (внутри `--log-dir`).
- `--log-console-color <auto|always|never>` (default `auto`) — policy ANSI-цвета console sink.
- `--log-rotate-size-mb <N>` (default `32`) — size-threshold ротации активного файла.
- `--log-rotate-max-files <N>` (default `7`) — retention cap для rotated файлов.
- `--snapshot-verify-chain` — audit/recovery: при загрузке JsonFile epoch snapshot выполнить полный genesis→tip replay вместо trust-default tail load.

One-window relay note:
- для happy-path cross-domain пользователь держит CLI/TUI на native/source RPC;
- target peer выбирается самим `pwmd` по configured `--transport-peer-seed` и `cluster_domain_hi` peer status;
- target-side `/v1/export-provenance` принимает handoff только от source peer, которому target доверяет через configured outbound seed context. Inbound/dev hello сам по себе trust root не создаёт.
- RPC и peer transport работают на разных сокетах: peer path не использует RPC listener как peer port в normal mode.
- `live_peer_count > 0` показывает обычную peer liveness, включая inbound/dev hello. `peer_relay_health=ok` требует `trusted_relay_peer_count > 0`: live peer, подтверждённый через configured outbound seed context. Диагностика stateful peer-сессий отражается в `/v1/status` (`peer_listen`, `peer_session_*`) и в `last_peer_error`.

Cross-shard observability (Sprint 15+): в логах искать префиксы **`relay:`** (исходящий relay с source), **`handoff_register:`** / **`import:`** (входящий handoff и локальный Import на target), **`genesis_state0_digest`** при загрузке снапшота (сверка с конфигом genesis при подозрении на `state_root mismatch`). Отказы relay помечаются **`peer relay`** / **`relay_failed`** с корреляцией `export_id`/`intent_id`. Подробнее: `docs/ROAMING_COMPLETION.md`, `docs/reviews/sprint-15-s3-16-do-snapshot-root-cause.md`.

Identity launch rules:
- если все четыре explicit-поля (`network_id`, `cluster_domain_hi`, `cluster_id`, `node_id`) заданы, `pwmd` стартует в `explicit` mode;
- если не задано ни одно и не передан `--shard` — стартует neutral relay baseline (без `A|B` affinity; shard-enforced guards не активируются);
- если explicit-поля не заданы, но передан `--shard A|B` — стартует deprecated compat alias path;
- частично заданный набор explicit-полей (например только `cluster_id`) отклоняется на старте.

Deterministic alias mapping (transition contract):
- `--shard A` -> `network_id=devnet`, `cluster_domain_hi=0x10`, `cluster_id=compat-shard-a`, `node_id=compat-node-a`;
- `--shard B` -> `network_id=devnet`, `cluster_domain_hi=0x20`, `cluster_id=compat-shard-b`, `node_id=compat-node-b`.

Важно: alias mapping фиксирован и явный; range-эвристики (`0x80 split` и аналоги) не используются.

Storage namespace migration policy (Sprint 11+):
- target: explicit identity mode использует domain-based namespace `domain-hi-0xNN` (по `cluster_domain_hi`);
- default neutral: запуск без `--shard` использует `state/pwm-data.json` и state namespace `neutral`;
- compat mapping: явный alias mode (`--shard`) сохраняет legacy namespace `shard-a|shard-b`;
- policy migration-only: без wire/API расширения и без hard-break для alias path.

## Operator quick path (domain-first)

Ниже минимальный операторский сценарий для актуальной модели Sprint 11.
Перед запуском с конкретным `domain_hi` см. словарь поддерживаемых доменных кластеров: `docs/DOMAINS.md`.

### 1) Поднять ноду для конкретного домена/кластера

```powershell
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3030 `
  --network-id devnet `
  --domain-hi 0x10 `
  --cluster-id local-cluster-a `
  --node-id local-node-a
```

Ожидание:
- startup-лог показывает `mode=explicit`;
- `state_ns=domain-hi-0x10`;
- shard-enforced semantics активны для explicit domain-конфига.

### 2) Поднять две ноды с разными `domain_hi`

```powershell
# Node A (domain_hi=0x10)
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3030 `
  --state-root state-a `
  --genesis-file ./tmp/genesis-custom.json `
  --genesis-passphrase "12345" `
  --network-id devnet `
  --domain-hi 0x10 `
  --cluster-id local-cluster-a `
  --node-id local-node-a

# Node B (domain_hi=0x20)
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3031 `
  --state-root state-b `
  --genesis-file ./tmp/genesis-custom.json `
  --genesis-passphrase "12345" `
  --network-id devnet `
  --domain-hi 0x20 `
  --cluster-id local-cluster-b `
  --node-id local-node-b
```

### 3) Связать ноды для smoke роуминга (seed peers + real transport)

```powershell
# Node A with seed to B (peer socket separated from RPC)
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3030 `
  --transport-peer-listen 127.0.0.1:3130 `
  --state-root state-a `
  --network-id devnet `
  --domain-hi 0x10 `
  --cluster-id local-cluster-a `
  --node-id local-node-a `
  --transport-real `
  --transport-peer-seed 127.0.0.1:3131

# Node B with seed to A (peer socket separated from RPC)
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3031 `
  --transport-peer-listen 127.0.0.1:3131 `
  --state-root state-b `
  --network-id devnet `
  --domain-hi 0x20 `
  --cluster-id local-cluster-b `
  --node-id local-node-b `
  --transport-real `
  --transport-peer-seed 127.0.0.1:3130
```

### 4) Минимальный smoke-check connected/mode

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/dev/peers"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/dev/peers"
```

Критерии:
- обе ноды в `phase=ready`;
- `effective_genesis_hash` и `network_id` совпадают на обеих нодах;
- у каждой ноды фиксируется peer в `/v1/dev/peers`;
- `live_peer_count >= 1`, `trusted_relay_peer_count >= 1`, `peer_relay_health=ok`, `last_peer_error` отсутствует;
- режимы и namespace соответствуют запуску (`explicit`, `domain-hi-0xNN`).

После успешного bind `pwmd` пишет startup-строку через logger.
Базовый формат console/file строки (Slice 18 contract):
- `[HH:MM:SS.mmm] #TAG: event | k1=v1 k2=v2 ...`;
- `#TAG` в uppercase: `#TRACE|#DEBUG|#INFO|#WARN|#ERROR`;
- `WARN/ERROR` маршрутизируются в `stderr`, остальные уровни в `stdout`.

Цветовая политика console sink:
- `auto`: ANSI только при TTY;
- `always`: ANSI принудительно;
- `never`: plain output;
- `NO_COLOR` имеет приоритет над `auto|always|never` и принудительно отключает ANSI.

Palette contract в TTY (если ANSI разрешен и `NO_COLOR` не задан):
- `#ERROR` — bright red;
- `#WARN` — dark red;
- числовые значения в message и `k=v` values — bright purple;
- timestamp и числа внутри id/hash-like токенов (hex/base58/base64-like) не подсвечиваются.

Файловый sink:
- активируется по `--log-file on|required`;
- использует size-based rotation и retention cap (`--log-rotate-size-mb`, `--log-rotate-max-files`);
- template path обязательно относительный к `--log-dir` (absolute path, drive prefix и `..` запрещены).
- placeholder `{node_id}` берется из effective runtime identity (значение `--node-id` после resolve) и проходит filesystem-safe sanitation (`[A-Za-z0-9._-]`, остальные символы заменяются на `_`, пустое значение -> `node-unknown`).
- как и console sink, фильтруется общим `RUST_LOG` (`EnvFilter`), то есть не является безусловно "полным" потоком;
- `RUST_LOG` управляет только уровнем/фильтром событий и не отменяет правила color policy/`NO_COLOR`.

Дополнительно startup UX печатает фазу инициализации snapshot:
- `pwmd startup phase: loading_snapshot (...)` при старте фоновой загрузки;
- `pwmd startup phase: ready (...)` при успешном завершении;
- `pwmd startup phase: ready_degraded (snapshot error: ...)` при ошибке загрузки с fallback на genesis-state.

На уровне `DEBUG` валидатор логирует включенные в блок транзакции с модификациями балансов по затронутым адресам (`tx_included`, поля `height`, `tx_kind`, `tx_id`, `addr`, `bal_before`, `bal_after`, `bal_delta`).

## ENV-переменные

- `PWM_CORS_ORIGINS` — обязателен для non-loopback bind (`0.0.0.0`, публичные интерфейсы), формат: список origin через запятую.
- `PWM_RPC` — не параметр процесса `pwmd`, но операционно критичен: клиенты используют его как endpoint ноды.
- `PWM_LOG_NAME` — alias для `--log-name`.
- `PWM_LOG_DIR` — alias для `--log-dir`.
- `PWM_LOG_FILE_TEMPLATE` — alias для `--log-file-template`.
- `PWM_LOG_FILE` — alias для `--log-file`.
- `PWM_PEER_LOG_FILE` — alias для `--peer-log-file`.
- `PWM_PEER_LOG_FILE_TEMPLATE` — alias для `--peer-log-file-template`.
- `PWM_LOG_CONSOLE_COLOR` — alias для `--log-console-color`.
- `PWM_LOG_ROTATE_SIZE_MB` — alias для `--log-rotate-size-mb`.
- `PWM_LOG_ROTATE_MAX_FILES` — alias для `--log-rotate-max-files`.
- `PWM_SNAPSHOT_VERIFY_CHAIN` — truthy значение включает полный replay при загрузке JsonFile epoch snapshot.
- `RUST_LOG` — сохраняет стандартное поведение `EnvFilter` и определяет общий фильтр потока для console/file sinks.
  - Для peer-потока применяется отдельный sink с целевым `target=pwmd::peer`.

## Балансовая семантика API (S15-S2)

Для контрактов `/v1/account` и `/v1/accounts` используется split-семантика:
- `local_state_balance` — локально наблюдаемое значение в state этой ноды;
- `authoritative_home_balance` — authoritative-значение home-shard (может быть `null`, если недоступно);
- `spendable_on_this_shard` — сумма, реально spendable на текущем шарде (для foreign адресов `null`);
- `local_view_only` — `true` для foreign-address local-view.

Compatibility policy:
- `balance_pwm` сохранён как legacy-алиас;
- для foreign-адресов `balance_pwm` принудительно `"0"` (safe clamp), чтобы старые клиенты не интерпретировали local-view как spendable truth;
- новым клиентам для spendability нужно опираться на `spendable_on_this_shard` и marker `balance_semantics` в `/v1/status`.

## Bootstrap пути

## 1) Встроенный devnet (`dev_net`)

Если `--genesis-file` не указан, нода поднимается на `GenesisSource::DevNet` через `dev_net()` из `pwm-core`.

## 2) Внешний genesis (`--genesis-file`)

`load_genesis_bundle(path, passphrase)` поддерживает только формат `schema_version=4`:

- `gen_cfg.funding.accounts[*]`: `acct_hex`, `pubkey_hex`, `der_idx`, `bal`;
- `gen_cfg.validators.set[*]`: `acct_hex`, `pubkey_hex`, `der_idx`;
- `gen_cfg.reward_policy.mode`: сейчас поддерживается `to_producer_account` (default);
- `validator_keys[*].enc_seed`: encrypted seed envelope (`kdf + aead`);
- `validator_keys[*].derivation_path`: строго `m/1000000'/1'`.

Проверки на загрузке:
- `schema_version` обязателен и равен `4` (v3/v2 hard-fail);
- `gen_cfg.validators.set` не пустой;
- длина `validator_keys` совпадает с `gen_cfg.validators.set`;
- `--genesis-passphrase` / `PWM_GENESIS_PASSPHRASE` обязателен для `--genesis-file`;
- в non-tty режиме отсутствие passphrase приводит к явной ошибке старта;
- каждый `enc_seed` успешно расшифровывается и даёт ровно 32 байта;
- derived `pubkey/account_id` по фиксированному пути `m/1000000'/1'` обязаны совпасть с `gen_cfg.validators.set[i]`.

При несовместимости нода завершится ошибкой старта.

## 3) Загрузка snapshot (`--data-file`)

`run_with(...)` использует fast-start путь:
- сначала строит chain из genesis и поднимает HTTP listener как можно раньше;
- затем запускает фоновую задачу snapshot-load;
- после успешной валидации атомарно заменяет in-memory `chain.blocks` и `chain.st`.

Для JsonFile epoch snapshot обычный режим теперь trust-default: loader читает summary `pwm-data.json`, manifest `epochs/pwm-epochs-manifest.json` и только хвост блоков из `epochs/`, затем проверяет `validate_snapshot_trusted` (genesis identity, `checkpoint_height == canonical_h`, `tip_hash`, связность/подписи хвоста, `state_root`). Полный genesis→tip replay включается `--snapshot-verify-chain` или truthy `PWM_SNAPSHOT_VERIFY_CHAIN`; если summary checkpoint отстаёт от manifest `canonical_h`, full replay включается принудительно.

ClickHouse snapshot load остаётся full-replay путём: JsonFile trust-load опции не ослабляют CH validation.

Пока snapshot инициализация не завершена, нода считается not-ready и защищает read/write API от гонок.

Если snapshot-файла и manifest нет, старт завершается в `ready` на genesis-state.

## Genesis roles note (operator)

- Роль validator key и путь расходования premine - разные вещи.
- Validator key в genesis используется для block production signing; расход premine определяется владельцем аккаунтной строки в `gen_cfg.funding.accounts[*].acct` и обычными правилами подписи/nonce.
- Подробный операторский разбор и pre-launch checklist: [GENESIS_BLOCK.md#validator-key-roles-operator-guide](GENESIS_BLOCK.md#validator-key-roles-operator-guide).

## HTTP API (`/v1/*`) и валидация

Роутер:
- `GET /v1/status`
- `GET /v1/head`
- `GET /v1/accounts`
- `GET /v1/account/:id`
- `POST /v1/tx`
- `POST /v1/roaming-intents`
- `GET /v1/roaming-intents/:id`
- `POST /v1/roaming-intents/:id/finalize`
- `POST /v1/export-provenance`
- `GET /v1/flow/recent`
- `POST /v1/peer/hello` (dev profile only)
- `GET /v1/dev/peers` (dev profile only)
- `POST /v1/bridge-federation/reset` — сброс локального latch `bridge_federation_trust_refused` (без изменения chain state); после сброса следующий успешный peer hello с совпадающим `bridge_commitment` снова открывает one-window. Тело пустое; ответ `204 NO_CONTENT` при `ready`.

На роутере включен `DefaultBodyLimit::max(256 * 1024)` для всех маршрутов.

## `GET /v1/status`
- Возвращает runtime-readiness и фазу старта.
- Поля:
  - `phase`: `starting | loading_snapshot | ready | ready_degraded`;
  - `ready`: bool;
  - `bridge_exported_registry_size`: размер `exported_registry` (кол-во известных `export_id`);
  - `bridge_imported_set_size`: размер `imported_set` (кол-во уже потреблённых `export_id`);
  - `snapshot_file`: путь snapshot (если задан);
  - `snapshot_error`: текст ошибки (только для деградированного старта).
  - `roaming_relay_mode`: текущий режим relay-пайплайна (`peer_relay_one_window` при relay через configured seed, либо fallback/manual diagnostic mode);
  - `roaming_relay_hint`: операторская подсказка по one-window relay или ручному trusted handoff fallback.
  - `bridge_federation_trust`: `ok | bridge_federation_trust_refused`;
  - `bridge_refusal_reason`: detail при отказе bridge trust (если есть).
  - **Семантика:** жёсткое сравнение `bridge_commitment` в hello делается только для пира с **тем же** `domain_hi`, что и нода (реплики одного шарда). Между **разными** шардами дайджесты level-2 по определению различаются; для такого пира отказ по несовпадению с *локальным* дайджестом не применяется.
  - Relay selection (`peer_relay_one_window`) читает те же поля с удалённого `GET /v1/status` seed-ноды; если там `bridge_federation_trust_refused`, relay не выберет этот seed.

Roaming MVP status contract (Sprint 13) формализован отдельно: `docs/rfc/9-crossdomain-roaming.md`.  
**Дополнение:** будущее направление протокола (блокировка на source до финализации импорта) описано в том же RFC, Appendix A.5; **текущая** нода не реализует escrow/lock-политику — `IMPORT` принимается при выполнении существующих проверок provenance/replay, без сквозного «гейта finality» между шардами.

## `GET /v1/head`
- Возвращает `{ height, tip }`.
- Данные читаются из `chain.tip_h()` и `chain.tip_hash()`.

## `GET /v1/accounts`
- Возвращает список аккаунтов из `chain.st.accounts`.
- Поля балансов сериализуются строками (`u128 -> String`).

## `GET /v1/account/:id`
- `:id` должен быть 32-байтным hex (`parse_id`), иначе `400 BAD_REQUEST`;
- если аккаунт не найден в state, `404 NOT_FOUND`;
- на успехе возвращается `AcctOut`.

## `POST /v1/tx`

Ключевые валидации и ограничения:
- JSON-тело больше 256 KiB -> `413 PAYLOAD_TOO_LARGE` (body-limit слой);
- до `validate_tx_shape` применяется dev/test process guard для `pwmd --shard A|B`:
  - Phase 1 **prefilter получателя** для user-facing потоков (`TRANSFER`, `BURN_MARK` с `beneficiary`) по правилам RFC 1 §7 (reserve/witness/unknown-domain) -> `400 BAD_REQUEST`; стабильный контракт сообщений: содержит `recipient domain` и class-specific причину (`reserve`, `witness-only`, `not recognized`);
  - тот же recipient-prefilter применяется и к `EXPORT` по полю `to` (`400 BAD_REQUEST` для reserve/witness/unknown-domain);
  - выбор **process shard** для ноды pinned к Phase 1 классу домена отправителя из `pwm_core::domain_index` (**Regulatory -> shard A**, **Sector -> shard B**); несовпадение с `--shard` -> `409 CONFLICT`;
  - для `TRANSFER` локальный путь допускает только `domain_hi(sender) == domain_hi(receiver)`; иначе -> `409 CONFLICT` (явный `EXPORT/IMPORT` track);
- для `IMPORT` до `apply_tx` проверяется provenance/replay guard на текущем state (RPC prefilter):
  - уже импортированный `export_id` -> `409 CONFLICT` (`duplicate import`);
  - неизвестный `export_id` или несовпадение `to/amount/target_domain` с экспортным provenance -> `400 BAD_REQUEST` (`invalid import`);
- `validate_tx_shape(&tx)` до попадания в mempool -> при ошибке `400 BAD_REQUEST`;
- `POST /v1/roaming-intents` (Slice 13.8+) принимает только подписанный `EXPORT` и создаёт home-shard roaming intent (`intent_id`, `export_id`, `status`, ttl); при configured seed peer source `pwmd` пытается доставить handoff provenance на target без target RPC со стороны пользователя;
- `GET /v1/roaming-intents/:id` возвращает lifecycle intent (`queued/exported/relayed/imported/expired/failed`) + `last_error` при наличии + `relay_mode`/`relay_hint`;
- `POST /v1/roaming-intents/:id/finalize` фиксирует операторский handoff `EXPORT -> IMPORT` как явный status-transition `queued|exported -> relayed` (идемпотентно; повторный вызов возвращает `200` и `changed=false` с детерминированным message);
- `POST /v1/export-provenance` регистрирует signed source handoff на target только при trusted peer context: source identity/key должны совпасть с peer state, полученным из configured outbound seed connectivity;
- `GET /v1/flow/recent` возвращает ограниченный in-memory trace последних runtime-событий (`accepted:*`, `applied:*`, `exported:*`, `imported:*`, `sealed:*`, `roaming_status:*`, `finalized:*`) для операторской диагностики и трассировки intent/tx lifecycle.
- `Mpool::push` при переполнении -> `507 INSUFFICIENT_STORAGE`.

На успехе:
- для `EXPORT/IMPORT`: tx применяется синхронно в state, затем выполняется `seal(vec![])` для фиксации результата в block chain;
- для остальных tx: tx кладется в mempool;
- возвращается `204 NO_CONTENT`;
- если настроен `data_file`, нода обязана сохранить snapshot; при ошибке сохранения HTTP-операция отвечает `500` с явной причиной (без silent success).

Ошибки `apply_tx` для `EXPORT/IMPORT` маппятся в HTTP из `pwm_core::tx::TxError` (plain text body):
- `DuplicateImport` -> `409 CONFLICT`;
- `InvalidExport` / `InvalidImport` / `InvalidTransfer` -> `400 BAD_REQUEST`;
- `BadNonce` / `Insufficient` / `InsufficientMarks` / `AlreadyInit` -> `409 CONFLICT`;
- ошибка `seal` после успешного `apply_tx` -> `500 INTERNAL_SERVER_ERROR`.

Операторская граница MVP: one-window клиентский UX теперь строится через roaming-intent API на native/source node; target peer достигается `pwmd` через trusted configured seed. Manual handoff (`finalize` -> `tx-handoff-register` -> `tx-import`) остаётся fallback/debug и требует, чтобы target уже доверял source peer через configured seed context.

## `POST /v1/peer/hello` (dev-only wire handshake probe)

- Назначение: минимальный wire-level path для RFC-8 handshake validation без полного p2p transport/policy engine.
- Принимает JSON `NodeHello` envelope из `crates/pwmd/src/handshake.rs`.
- Валидация использует utilities из slice #2:
  - mandatory fields,
  - `network_id` match,
  - `genesis_hash` match (runtime anchor),
  - подпись,
  - timestamp skew window,
  - replay nonce window.
- Ответ всегда в контракте:
  - `{"accepted": true, "class":"native|foreign"}` на success;
  - `{"accepted": false, "reason":"<stable_label>"}` на reject.
- Stable reject labels:
  - `bad_signature`, `replay_nonce`, `network_mismatch`, `genesis_mismatch`, `timestamp_skew`, `malformed`.
- Peer class assignment строго по RFC-8:
  - `native` iff `peer.cluster.domain_hi == local.cluster_domain_hi`,
  - `foreign` иначе.
- Никаких range heuristics (`0x80 split` и аналоги) не используется.

## `GET /v1/dev/peers` (dev-only peer registry, policy, transport/churn/soak snapshot)

- Возвращает lightweight in-memory peer state и базовые handshake counters:
  - `accepted_total`,
  - `rejected_total`,
  - `reject_reason_total`,
  - `class_accept_total`,
  - `connected_by_class`,
  - `peers[]` (`node_id`, `domain_hi`, `class`, `last_seen_ms`, `status`) в deterministic native-first порядке,
  - `policy` snapshot:
    - `config.native_outbound_target`,
    - `config.foreign_outbound_target`,
    - `config.native_min_live`,
    - `config.native_backoff { base_ms, max_ms }`,
    - `config.foreign_backoff { base_ms, max_ms }`,
    - `config.class_weights { native, foreign }`,
    - `counters.prioritize_runs`,
    - `counters.backoff_select_native`,
    - `counters.backoff_select_foreign`,
    - `counters.native_degraded_flips`,
    - `native_live`,
    - `native_degraded_state`;
  - `transport` snapshot:
    - `ticks_total`,
    - `counters.dial_attempt_by_class_result` (ключи формата `native:success`, `foreign:retryable_fail`),
    - `counters.backoff_skip_total`,
    - `last_attempt_ms_by_class`,
    - `last_result_by_class`,
    - `native_underflow_ticks`,
    - `native_underflow_threshold_ticks`,
    - `native_degraded_state`,
    - `native_degraded_transitions`,
    - `seed_rotation_cursor`,
    - `tick_attempt_budget`,
    - `last_tick_attempts`,
    - `soak_ticks_capped`,
    - `soak_health_snapshot_total`,
    - `soak_health_last_tick`,
    - `reconnect_runaway_stop_total`,
    - `reconnect_runaway_guard_active`;
  - `churn` snapshot:
    - `seed_rotation_total`,
    - `retrying_total`,
    - `disconnected_total`,
    - `bounded_retry_cooldowns_total`,
    - `seed_attempt_by_result`,
    - `reconnect_streak_current`,
    - `reconnect_streak_max`,
    - `stable_tick_total`,
    - `unstable_tick_total`;
  - `soak` confidence snapshot:
    - `loop_ticks_capped`,
    - `stable_ticks_capped`,
    - `unstable_ticks_capped`,
    - `reconnect_streak_current`,
    - `reconnect_streak_max`,
    - `runaway_stop_total`,
    - `runaway_guard_active`,
    - `health_snapshot_total`,
    - `health_last_tick`.
- Это groundwork observability path для RFC-8 до подключения full metrics backend.

## Native-first policy + minimal transport loop (RFC-8 slices #4 + #5 + #6 + #7 + #8)

- В `pwmd` добавлен минимальный in-memory policy-layer поверх dev handshake/registry без внедрения transport scheduler.
- Конфиг policy хранится в runtime-state и задает:
  - outbound targets по классам (`native_outbound_target`, `foreign_outbound_target`),
  - `native_min_live` для failover сигнализации,
  - раздельные backoff envelopes для `native` и `foreign`,
  - class weights (`native`/`foreign`) для дальнейшего scheduler wiring.
- Добавлены deterministic helpers:
  - `prioritize_peer_candidates(...)` — native-first сортировка кандидатов без range heuristics;
  - `select_backoff_for_class(...)` — выбор backoff envelope по классу;
  - `refresh_native_degraded_state(...)` — переключение failover-state при underflow native live peers.
- Что добавлено в slice #6 (controlled scope):
  - добавлен минимальный real socket outbound path к `transport.peer_seeds` с timeout/backoff knobs;
  - после connect выполняется handshake-on-connect: отправка локального `NodeHello` + чтение peer `NodeHello` в минимальном frame (`u32 len + json`);
  - входящий `NodeHello` валидируется через `validate_node_hello`, а accept/reject reason counters и логи обновляются на реальном path;
  - при `transport.enabled=false` или пустом seed-list поведение остается как раньше (legacy stub transport loop).
- Что добавлено в slice #7 (controlled hardening scope):
  - real transport tick обрабатывает multiple seeds с deterministic fairness rotation (round-robin cursor);
  - приоритет попыток class-aware native-first для seeds с известной peer-class;
  - введен bounded tick attempt budget как guard rail от storm loops;
  - reconnect semantics усилены bounded retries + cooldown + deterministic jitter;
  - peer transport states в registry явно переходят между `connected`, `retrying`, `disconnected`.
- Что добавлено в slice #8 (controlled long-run soak scope):
  - добавлены bounded long-run rollups/counters для transport/churn observability;
  - добавлена optional periodic health aggregation по transport ticks;
  - добавлен safety stop guard для runaway reconnect streak с cooldown;
  - `/v1/dev/peers` расширен additive soak confidence полями без изменения существующего контракта.
- Ограничение текущего slice:
  - нет полноценного mesh/peer discovery/network engine;
  - real transport сейчас только controlled outbound seed connect path с ограниченным churn hardening;
  - policy path и transport path по-прежнему ориентированы на dev-observability и unit/integration validation.

## Clarification: process shard vs spec-level geo-shard

- `pwmd --shard A|B` в текущем runtime - это исключительно dev/test process partition (операционная сегментация экземпляров процесса).
- Это не является протокольной гео-шард моделью и не должно трактоваться как mapping "A/B = диапазоны `domain_hi`".
- Spec-level geo-shard semantics фиксируется в спецификациях как кластер с фиксированным `domain_hi` и может поддерживать островизацию на уровне доменных кластеров.
- Практическое правило безопасности: не использовать диапазонные эвристики (`domain_hi < 0x80` vs `>= 0x80`) для маршрутизации или policy-решений.

## Readiness semantics во время startup

- `GET /v1/status` доступен всегда, включая ранний startup.
- До перехода в ready-state (`phase=starting|loading_snapshot`) запросы к `GET /v1/head`, `GET /v1/accounts`, `GET /v1/account/:id`, `POST /v1/tx` возвращают `503 SERVICE_UNAVAILABLE`.
- Это предотвращает гонки между фоновым snapshot-apply и клиентскими запросами.
- Фоновый `seal` loop также не печатает блоки до ready-state.

## Фоновый `seal` loop и `SealAbort`

`spawn_seal_loop(app)` запускает async-задачу:
- тик каждые 2 секунды;
- на каждом тике берет до 64 tx из пула (`Mpool::take(64)`);
- вызывает `Chain::seal(txs)`.

Ветки:
- `Ok(())`: блок добавлен, логируется новая высота, snapshot сохраняется в `data_file` (если задан);
- дополнительно действует block-based гарантия autosnapshot: checkpoint каждые `100` блоков (`AUTOSNAPSHOT_BLOCK_INTERVAL`, текущая конфигурируемая константа);
- `Err((msg, txs))` (`SealAbort` из `pwm-core`): tx не теряются, возвращаются в head пула через `Mpool::prepend_block(txs)`, а ошибка уходит в warn-лог.

Это обеспечивает fail-safe поведение mempool при неуспешном seal.

## Модель snapshot persistence (`--data-file`)

Формат `SnapshotData` summary:
- `version` (текущая `SNAPSHOT_VERSION = 1`);
- `genesis_accounts` (acct/pubkey/der_idx для контроля совместимости genesis);
- `blocks` — полный массив для legacy inline snapshot или tail при epoch load;
- `state` (accounts + fee_pool);
- `blocks_stored` и `checkpoint_height` для epoch-backed JsonFile.

JsonFile epoch layout:
- `pwm-data.json` — summary/checkpoint state;
- `epochs/pwm-epochs-manifest.json` — `canonical_h`, `tip_hash`, список epoch-файлов;
- `epochs/block_e*.json` — JSONL блоки, по `EPOCH_SPAN = 1000` высот на файл.

Canonical-only policy:
- источником истины считаются только canonical поля (`version`, `genesis_accounts`, `blocks`, `state`) и бинарные данные внутри них;
- любые user-facing/derived поля (например `pretty`, `hints` и т.п.) игнорируются при загрузке и не участвуют в верификации консенсусной истины;
- canonical snapshot обязан содержать одновременно и `version`, и `genesis_accounts` (частично-canonical формат отклоняется).

Сохранение:
- legacy/monolithic `save_snapshot` сериализует JSON pretty, пишет во временный файл (`.tmp`) и делает `rename` поверх целевого файла;
- JsonFile runtime path пишет/синхронизирует epoch JSONL и summary без полного чтения всех epoch-файлов на каждый API-save;
- checkpoint summary переписывается каждые `SNAP_CHK_BLK_IV = 100` блоков; это не то же самое, что `EPOCH_SPAN = 1000` для размеров epoch-файлов.

Когда сохраняется:
- после принятия tx в `POST /v1/tx`;
- после успешного `seal`.

Проверки полной совместимости (`validate_snapshot`, audit/full replay):
- совпадение `version`;
- полное совпадение набора `genesis_accounts`;
- последовательные высоты блоков (`1..N`);
- непрерывная цепочка `prev_hash` (включая привязку первого блока к genesis sentinel);
- совпадение `tx_root` заголовка с пересчитанным Merkle root по `txs`;
- корректный `prod_idx` по расписанию PoA и валидная подпись заголовка для producer pubkey из genesis;
- self-verification replay: каждый блок последовательно переигрывается от genesis state (`apply_tx` + `accrue_marks` + `reward_producer`), и `hdr.state_root` каждого блока обязан совпасть с `digest(replay_state)`;
- для каждого state-аккаунта: `account_id == H(pubkey, derivation_index)` (внутренняя непротиворечивость state-key);
- если есть tip-блок, его `state_root` должен совпадать с `digest(snapshot.state)`.
- итоговый replay-state обязан совпасть с `snapshot.state` по digest;
- если блоков нет, `digest(snapshot.state)` должен совпадать с genesis state digest.

Trust-default JsonFile load (`validate_snapshot_trusted`) вместо полного replay проверяет genesis identity, согласованность `checkpoint_height`/manifest `canonical_h`, manifest `tip_hash`, хвост блоков по `TAIL_BLOCK_CAP`, PoA header signatures/`tx_root`/linkage и совпадение persisted state root с tip-блоком.

При провале любой проверки snapshot не применяется к chain-state:
- нода остается на genesis-state;
- `phase` переходит в `ready_degraded`;
- текст ошибки публикуется в `/v1/status` (`snapshot_error`) и в startup-логе.

Runtime strictness (Slice 15):
- при runtime-ошибке `save_snapshot` после `POST /v1/tx`, `POST /v1/roaming-intents` и `POST /v1/roaming-intents/:id/finalize` API возвращает `500` (оператор явно видит проблему persistence);
- при runtime-expire roaming intent в `GET /v1/roaming-intents/:id` (когда ttl уже истёк и статус меняется на `expired`) snapshot также сохраняется сразу; ошибка persistence поднимается в HTTP как `500` вместо silent-update;
- при ошибке `save_snapshot` в фоновой seal-петле нода помечает startup-state как `ready_degraded` с заполненным `snapshot_error`, чтобы отказ был виден через `/v1/status`;
- при успешном последующем сохранении runtime-state возвращается в `ready`.

Legacy handling:
- legacy snapshot без `version` и `genesis_accounts` (формат `blocks + state`) мигрируется автоматически и безопасно: `version` и `genesis_accounts` вычисляются из текущего genesis-конфига, затем запускается полный набор валидаций;
- неоднозначные/неполные форматы (например есть только одно из полей `version`/`genesis_accounts`, либо отсутствует `blocks`/`state`) отвергаются с явной ошибкой и инструкцией пересоздать snapshot текущим `pwmd`.

## Security / ops заметки

- По умолчанию нода слушает только loopback (`127.0.0.1:3030`), что безопаснее для dev-режима.
- CORS permissive включается только на loopback bind.
- Для non-loopback bind требуется явный allowlist через `PWM_CORS_ORIGINS`; иначе запуск блокируется.
- Лимит тела `POST /v1/tx` фиксирован 256 KiB, чтобы ограничить перегрузку JSON parser/mempool path.
- Snapshot save errors surfaced to operators: API paths return `500` on persistence failure, and seal-loop failures mark `/v1/status` as `ready_degraded` until a later successful save.

## Карта текущих тестов `pwmd`

Встроенные тесты в `crates/pwmd/src/lib.rs` покрывают:
- CORS-политику:
  - loopback разрешен;
  - non-loopback требует `PWM_CORS_ORIGINS`.
- Bootstrap из файла genesis:
  - `genesis_json_roundtrip_dev_seed` проверяет корректную загрузку dev-seed bundle.
- HTTP smoke:
  - `v1_head_returns_tip_json`;
  - `v1_tx_rejects_domain_mismatch` (ранний reject shape);
  - `v1_tx_accepts_signed_init` (успешный enqueue в pool);
  - `v1_tx_rejects_oversized_body` (проверка `DefaultBodyLimit`);
  - `v1_status_bridge_counters_grow_after_http_export_import` (рост bridge counters после HTTP `EXPORT/IMPORT`);
  - `v1_tx_http_export_import_advances_head_height_via_sync_seal` (наблюдаемый `head.height` рост после sync `seal(vec![])` в HTTP-flow).
- Snapshot:
  - `snapshot_roundtrip_blocks_and_state` (save/load без потерь);
  - `snapshot_rejects_mismatched_genesis` (reject на несовместимом genesis);
  - `snapshot_ignores_non_canonical_derived_fields` (derived/non-canonical мусор не влияет на truth source);
  - `snapshot_rejects_tampered_block_header` (reject на tamper заголовка/подписи).

Что тесты пока не закрывают полноценно:
- e2e-путь с реальным TCP-сервером и внешним клиентом;
- длительные сценарии восстановления после частичных FS-сбоев;
- расширенные конкурентные сценарии нагрузки mempool/seal-loop.

## Handshake utilities groundwork (RFC-8 slice #2)

В `crates/pwmd/src/handshake.rs` добавлен локальный (без wire networking) слой для будущего handshake gate:
- `NodeHello` envelope со структурой из RFC-8:
  - `network_id`, `genesis_hash` (optional),
  - `cluster { domain_hi, cluster_id }`,
  - `node { node_id, pubkey }`,
  - `capabilities { protocol_version, tx_features, services }`,
  - `nonce`, `timestamp_ms`, `signature`.
- сериализация/десериализация через `serde`;
- подпись и верификация envelope (`NodeHello::sign`, `NodeHello::verify_signature`), где подпись покрывает все поля кроме `signature`;
- `validate_node_hello(...)` как тестируемый API для future p2p handshake gate:
  - mandatory field checks,
  - `network_id`/`genesis_hash` compatibility checks,
  - signature verification,
  - skew window check,
  - replay nonce window check через in-memory `ReplayNonceCache`.

Reason-coded reject surface:
- `HandshakeRejectReason::{BadSignature, ReplayNonce, NetworkMismatch, GenesisMismatch, TimestampSkew, Malformed}`;
- для observability определены стабильные labels/constants:
  - `bad_signature`,
  - `replay_nonce`,
  - `network_mismatch`,
  - `genesis_mismatch`,
  - `timestamp_skew`,
  - `malformed`;
- `HandshakeRejectReason::as_label()` дает reason->label mapping для будущих метрик/логов.

Ограничение текущего среза:
- handshake wire-path пока dev-only (`/v1/peer/hello` + `/v1/dev/peers`);
- full p2p transport/dial-backoff scheduler/policy engine пока не внедрены (есть только policy plumbing и counters).
