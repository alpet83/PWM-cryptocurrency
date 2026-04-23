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

Структура `PwmdConfig` содержит три runtime-параметра:
- `listen: SocketAddr` — адрес bind HTTP-сервера;
- `genesis: GenesisSource` — источник genesis (`DevNet` или `JsonFile(PathBuf)`);
- `data_file: PathBuf` — путь JSON snapshot-файла.

`PwmdConfig::default()`:
- `listen = 127.0.0.1:3030`;
- `genesis = DevNet`;
- `data_file = pwm-data.json`.

## CLI-флаги `pwmd`

Бинарник `crates/pwmd/src/main.rs` мапит аргументы в `PwmdConfig`:
- `--listen <ADDR>` (default `127.0.0.1:3030`);
- `--genesis-file <PATH>` (если не задан, используется встроенный `dev_net()`);
- `--data-file <PATH>` (default `pwm-data.json`).

После успешного bind `pwmd` всегда печатает в stderr строку старта:
- `pwmd listening on http://<addr>` (например `pwmd listening on http://127.0.0.1:3030`),
даже если `RUST_LOG`/tracing-level скрывает `info!`-логи.

Дополнительно startup UX печатает фазу инициализации snapshot:
- `pwmd startup phase: loading_snapshot (...)` при старте фоновой загрузки;
- `pwmd startup phase: ready (...)` при успешном завершении;
- `pwmd startup phase: ready_degraded (snapshot error: ...)` при ошибке загрузки с fallback на genesis-state.

## ENV-переменные

- `PWM_CORS_ORIGINS` — обязателен для non-loopback bind (`0.0.0.0`, публичные интерфейсы), формат: список origin через запятую.
- `PWM_RPC` — не параметр процесса `pwmd`, но операционно критичен: клиенты используют его как endpoint ноды.

## Bootstrap пути

## 1) Встроенный devnet (`dev_net`)

Если `--genesis-file` не указан, нода поднимается на `GenesisSource::DevNet` через `dev_net()` из `pwm-core`.

## 2) Внешний genesis (`--genesis-file`)

`load_genesis_bundle(path)` читает JSON вида:
- `gen_cfg`;
- `validator_seeds_hex`.

Проверки на загрузке:
- длина `validator_seeds_hex` совпадает с `gen_cfg.rows`;
- каждый seed декодируется в 32 байта;
- из seed выводится ключ (`SLIP-0010 m/0'/0'`), и pubkey/account_id обязаны совпасть с соответствующей строкой `gen_cfg`.

При несовместимости нода завершится ошибкой старта.

## 3) Загрузка snapshot (`--data-file`)

`run_with(...)` использует fast-start путь:
- сначала строит chain из genesis и поднимает HTTP listener как можно раньше;
- затем запускает фоновую задачу snapshot-load (`load_snapshot + validate_snapshot`);
- после успешной валидации атомарно заменяет in-memory `chain.blocks` и `chain.st`.

Пока snapshot инициализация не завершена, нода считается not-ready и защищает read/write API от гонок.

Если snapshot-файла нет, старт завершается в `ready` на genesis-state.

## HTTP API (`/v1/*`) и валидация

Роутер:
- `GET /v1/status`
- `GET /v1/head`
- `GET /v1/accounts`
- `GET /v1/account/:id`
- `POST /v1/tx`

На роутере включен `DefaultBodyLimit::max(256 * 1024)` для всех маршрутов.

## `GET /v1/status`
- Возвращает runtime-readiness и фазу старта.
- Поля:
  - `phase`: `starting | loading_snapshot | ready | ready_degraded`;
  - `ready`: bool;
  - `snapshot_file`: путь snapshot (если задан);
  - `snapshot_error`: текст ошибки (только для деградированного старта).

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
- `validate_tx_shape(&tx)` до попадания в mempool -> при ошибке `400 BAD_REQUEST`;
- `Mpool::push` при переполнении -> `507 INSUFFICIENT_STORAGE`.

На успехе:
- tx кладется в mempool;
- возвращается `204 NO_CONTENT`;
- если настроен `data_file`, нода пытается сразу сохранить snapshot (ошибка логируется, но запрос остается успешным).

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
- `Err((msg, txs))` (`SealAbort` из `pwm-core`): tx не теряются, возвращаются в head пула через `Mpool::prepend_block(txs)`, а ошибка уходит в warn-лог.

Это обеспечивает fail-safe поведение mempool при неуспешном seal.

## Модель snapshot persistence (`--data-file`)

Формат `SnapshotData`:
- `version` (текущая `SNAPSHOT_VERSION = 1`);
- `genesis_rows` (acct/pubkey/der_idx для контроля совместимости genesis);
- `blocks`;
- `state` (accounts + fee_pool).

Canonical-only policy:
- источником истины считаются только canonical поля (`version`, `genesis_rows`, `blocks`, `state`) и бинарные данные внутри них;
- любые user-facing/derived поля (например `pretty`, `hints` и т.п.) игнорируются при загрузке и не участвуют в верификации консенсусной истины;
- canonical snapshot обязан содержать одновременно и `version`, и `genesis_rows` (частично-canonical формат отклоняется).

Сохранение (`save_snapshot`):
- сериализация JSON pretty;
- запись во временный файл (`.tmp`) и `rename` поверх целевого файла (атомарная замена на уровне FS).

Когда сохраняется:
- после принятия tx в `POST /v1/tx`;
- после успешного `seal`.

Проверки совместимости (`validate_snapshot`):
- совпадение `version`;
- полное совпадение набора `genesis_rows`;
- последовательные высоты блоков (`1..N`);
- непрерывная цепочка `prev_hash` (включая привязку первого блока к genesis sentinel);
- совпадение `tx_root` заголовка с пересчитанным Merkle root по `txs`;
- корректный `prod_idx` по расписанию PoA и валидная подпись заголовка для producer pubkey из genesis;
- self-verification replay: каждый блок последовательно переигрывается от genesis state (`apply_tx` + `accrue_marks` + `reward_producer`), и `hdr.state_root` каждого блока обязан совпасть с `digest(replay_state)`;
- для каждого state-аккаунта: `account_id == H(pubkey, derivation_index)` (внутренняя непротиворечивость state-key);
- если есть tip-блок, его `state_root` должен совпадать с `digest(snapshot.state)`.
- итоговый replay-state обязан совпасть с `snapshot.state` по digest;
- если блоков нет, `digest(snapshot.state)` должен совпадать с genesis state digest.

При провале любой проверки snapshot не применяется к chain-state:
- нода остается на genesis-state;
- `phase` переходит в `ready_degraded`;
- текст ошибки публикуется в `/v1/status` (`snapshot_error`) и в startup-логе.

Legacy handling:
- legacy snapshot без `version` и `genesis_rows` (формат `blocks + state`) мигрируется автоматически и безопасно: `version` и `genesis_rows` вычисляются из текущего genesis-конфига, затем запускается полный набор валидаций;
- неоднозначные/неполные форматы (например есть только одно из полей `version`/`genesis_rows`, либо отсутствует `blocks`/`state`) отвергаются с явной ошибкой и инструкцией пересоздать snapshot текущим `pwmd`.

## Security / ops заметки

- По умолчанию нода слушает только loopback (`127.0.0.1:3030`), что безопаснее для dev-режима.
- CORS permissive включается только на loopback bind.
- Для non-loopback bind требуется явный allowlist через `PWM_CORS_ORIGINS`; иначе запуск блокируется.
- Лимит тела `POST /v1/tx` фиксирован 256 KiB, чтобы ограничить перегрузку JSON parser/mempool path.
- Snapshot сохранение best-effort: ошибки записи логируются, но процесс ноды и HTTP не падают.

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
  - `v1_tx_rejects_oversized_body` (проверка `DefaultBodyLimit`).
- Snapshot:
  - `snapshot_roundtrip_blocks_and_state` (save/load без потерь);
  - `snapshot_rejects_mismatched_genesis` (reject на несовместимом genesis);
  - `snapshot_ignores_non_canonical_derived_fields` (derived/non-canonical мусор не влияет на truth source);
  - `snapshot_rejects_tampered_block_header` (reject на tamper заголовка/подписи).

Что тесты пока не закрывают полноценно:
- e2e-путь с реальным TCP-сервером и внешним клиентом;
- длительные сценарии восстановления после частичных FS-сбоев;
- расширенные конкурентные сценарии нагрузки mempool/seal-loop.
