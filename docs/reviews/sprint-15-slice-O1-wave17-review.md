# Sprint 15 — слайс O.1 wave17: ревью выноса `transport::peer_session`

**Коммиты:** `03b44e5`, `686026e` (код + chore тикета), `fb6eada` (ревью + полный тикет + план/чеклист)

## Область

Wire/TCP peer-session по `CODEBASE_REFACTORING.md` §2.2 строка #5: `PeerWireMsg`, framed read/write, heartbeat и cross-shard wire merge helpers, sticky session, `process_inbound_socket`, `run_seed_session`; точки входа **`spawn_*_loop`** остаются в **`transport.rs`**.

## Выводы

- Перенос выглядит механическим; поведение и границы доверия по diff не расширены.
- Видимость: **`pub(super)`** в **`peer_session`**, без лишнего **`pub(crate)`** на внутренних helper’ах (урок wave16 соблюдён).

## Тесты (оркестратор)

`cargo fmt --check`, `cargo test -p pwmd` — **197** OK, `cargo check --workspace` — OK.

## Вердикт

**PASS**

---

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-slice-O1-wave17-review.md
token_usage:
  source: estimate
  total: 14000
  confidence: low
```
