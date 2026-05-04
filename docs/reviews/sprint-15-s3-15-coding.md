# S15-S3.15 — relay handoff / шаг 3 TUI (coding)

## Симптом и корневая причина (типовой сценарий)

Шаг 3 в `pwm-tui` показывал «relay не подтверждён», пока `GET /v1/roaming-intents/:id` не вернёт `status == relayed`. При сбое цепочки `relay_handoff` вызывается `mark_relay_err`: в `RoamingPool` заполняется `last_error`, а **`mark_relay_error` не меняет `IntentStatus`** — намерение часто остаётся **`exported`**. В этом случае TUI не видел `relayed` и при этом **терял** `last_error` после цикла опроса (передавался `None` в финальный отчёт).

Кандидаты срыва relay (по коду `select_target` → `post_peer_hello` → POST `/v1/export-provenance`):

1. Нет или неверный `--transport-peer-seed` (нет совпадения `cluster_domain_hi` / `ready` / `genesis_guard`).
2. Отказ `POST /v1/peer/hello` (HTTP, `accepted: false`, genesis/network и т.д.).
3. Отказ target на `/v1/export-provenance` (подпись handoff, trusted peer policy, дубликаты).

## Что сделано

### `crates/pwmd/src/relay.rs`

- Контекст логов **`RelayTrace`**: `op` (`handoff` | `import`), `intent_id`, `export_id`, целевой `domain_hi`.
- **`select_target`**: `info` на старт; для каждого seed — `warn` при mismatch (`ready`, `cluster_domain_hi`, `genesis_guard`), HTTP-коде или ошибке сети; `info` при успешном матче.
- **`post_peer_hello`**: лог до запроса; при ошибке — HTTP-код и **`http_body_log_snippet`** (JSON → только значения полей, без ключей; усечение).
- **`relay_handoff`**: лог перед `export-provenance`; при ошибке — код и snippet; при успехе — структурированный `info`.
- **`relay_import`**: те же принципы для `POST /v1/tx` (поведение без лишнего `peer/hello`, как было).
- **После pwm-review:** сообщения `RelayErr`, попадающие в `last_error` и TUI, используют тот же **`http_body_log_snippet`**, что и логи — без полного сырого HTTP body; для `peer/hello` с `accepted: false` причина также проходит через snippet.

### `crates/pwm-tui/src/main.rs`

- При **`exported`** и **`last_error.is_some()`**: шаг 3 — **FAIL** с текстом relay из API, а не общая фраза «проверьте вручную».
- Цикл опроса: **12** итераций (было 8), пауза **600 ms** (было 500).
- После цикла в отчёт передаётся **`last_poll_error`** с последнего успешного GET статуса (исправлена потеря `last_error`).

### `issues-report.md`

- Запись про ожидание **`imported` на source** после импорта на target: **`mark_imported_by_export_id`** работает только в локальном пуле с тем же `export_id`; межнодовая синхронизация намерений в текущей модели не подразумевается — отдельный **follow-up**, если понадобится продуктовый контракт.

## Файлы

- `crates/pwmd/src/relay.rs`
- `crates/pwm-tui/src/main.rs`
- `issues-report.md`
- `docs/reviews/sprint-15-s3-15-coding.md`

## Команды

```text
cargo fmt
cargo test -p pwmd --lib   # 192 passed
cargo check -p pwm-tui     # ok
```

Публичный HTTP-контракт `pwmd` не менялся (только логи и клиентский TUI); маркер сборки **не** повышался.

## CQDS index

Фоновый `rebuild_index` для проекта Colloquium **не выполнен** в этой сессии (MCP `user-cqds_mcp_mini` недоступен с хоста агента).

## Follow-up

- **pwm-testing**: регрессия двухнодового сценария + при желании сценарий «relay fail → exported + last_error → текст в TUI».
- **pwm-review**: проверка того, что `http_body_log_snippet` не протекает секретами в типичных ответах ошибок peer/target.

## Optimization note

Вынесены **`http_body_log_snippet` / `json_values_flat`** и **`RelayTrace`**, чтобы не дублировать поля корреляции на каждом шаге relay; дальнейший кандидат на вынос — общий helper для «HTTP POST + warn snippet» если появится третий похожий путь.

---

```yaml
agent: pwm-coding
result: PASS
artifacts:
  - crates/pwmd/src/relay.rs
  - crates/pwm-tui/src/main.rs
  - issues-report.md
  - docs/reviews/sprint-15-s3-15-coding.md
commands:
  - cargo fmt
  - cargo test -p pwmd --lib
  - cargo check -p pwm-tui
token_usage:
  source: estimate
  input: null
  output: null
  total: 18000
  confidence: low
```
