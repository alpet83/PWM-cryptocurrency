# Sprint 15 Slice 1 Coding (`S15-S1-XSHARD-HARDEN`)

Дата: 2026-04-29  
Статус: completed (coding slice)

## Scope

- Внедрён source-side preflight для `EXPORT` через новый endpoint `POST /v1/export-readiness`.
- В `POST /v1/tx` для `TxBody::Export` включён fail-closed guard до `seal/debit`:
  - обязательный preflight,
  - TTL freshness check,
  - binding check (`from/to/amount/target_domain`),
  - source hints check (`nonce`, `height`).
- Добавлен явный diagnostics contract:
  - reject ответ содержит `code` + `hint`,
  - `/v1/status` публикует `last_readiness_reject_code` и `last_readiness_reject_hint`,
  - `/v1/flow/recent` получает trace-row `rejected:export_readiness`.
- Sprint14 finalize/handoff/provenance flow сохранён без изменения.

## Changed files

- `crates/pwmd/src/roaming.rs`
- `crates/pwmd/src/api.rs`
- `crates/pwmd/src/lib.rs`
- `crates/pwmd/Cargo.toml`

## Tests run

- `cargo fmt`
- `cargo check -p pwmd`
- `cargo test -p pwmd v1_export_`
  - `v1_export_rejects_without_readiness_and_keeps_balance`
  - `v1_export_rejects_stale_readiness_and_cannot_reuse_it`
  - `v1_export_applies_with_valid_readiness_preflight`

## Known limits (intentional in S15-S1)

- Readiness preflight применяется к submit path `POST /v1/tx` для `EXPORT`; существующий `roaming-intents` flow из Sprint14 оставлен совместимым.
- Target-side online readiness probe не добавлялся в этом слайсе; используется source-side fail-closed gate с TTL/binding.
- Причина последнего reject в `/v1/status` берётся из последнего flow события readiness reject (операторская диагностика, не новый persistent snapshot contract).

## Remediation (post-review blockers S15-S1)

- `POST /v1/export-readiness`: введён server-side cap для `ttl_sec` (`MAX_READINESS_TTL_SEC`), клиентский oversized TTL теперь принудительно ограничивается безопасным максимумом.
- `POST /v1/roaming-intents`: закрыт readiness bypass для `EXPORT` — добавлен тот же fail-closed `consume_readiness` guard до `register_export`/`seal` side effects.
- Readiness reject contract усилен: reject body возвращается как стабильный JSON с полями `code`, `hint`, `message` (для сохранения операторской читаемости в legacy-формате `code=...; hint=...` внутри `message`).
- Добавлены тесты на:
  - cap oversized `ttl_sec` в `/v1/export-readiness`;
  - отсутствие debit/state intent mutation при reject в `/v1/roaming-intents` без preflight;
  - structured reject response contract для `/v1/tx` readiness reject и сохранение stale/reuse поведения.

## Remediation2 (S15-S1 testing FAIL narrow fix)

- Обновлены только 2 legacy HTTP flow теста в `crates/pwmd/src/lib.rs`:
  - `v1_status_bridge_counters_grow_after_http_export_import`
  - `v1_tx_http_export_import_advances_head_height_via_sync_seal`
- В обоих кейсах перед `POST /v1/tx` для `TxBody::Export` добавлен обязательный preflight:
  - `POST /v1/export-readiness` с `{"tx": <export>, "ttl_sec": 30}`
  - assert на `200 OK`, после чего сохранены исходные проверки `204 NO_CONTENT` для export/import happy path.
- Политика readiness не ослаблялась: тесты теперь явно соблюдают fail-closed контракт вместо обхода.

### Commands / results

- `cargo test -p pwmd v1_status_bridge_counters_grow_after_http_export_import -- --nocapture` -> PASS
- `cargo test -p pwmd v1_tx_http_export_import_advances_head_height_via_sync_seal -- --nocapture` -> PASS
