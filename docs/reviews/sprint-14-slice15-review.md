# Sprint 14 — Slice 15 review (runtime remediation)

## Verdict
`approve with nits`

## Confirmed
- strict persistence включён: при сбое snapshot-save API возвращает `500`, статус переходит в `ready_degraded`.
- добавлены наблюдаемость и диагностика: `GET /v1/flow/recent`, `relay_mode`/`relay_hint`.
- autosnapshot checkpoint-политика заведена с интервалом 100 блоков.

## Nits
- зафиксировать в docs явнее семантику autosnapshot (“не реже 100 блоков” vs “ровно каждые 100”).
- явно описать в API docs, что при strict mode возможен `500` после in-memory apply (клиентам нужен аккуратный retry/idempotency).
