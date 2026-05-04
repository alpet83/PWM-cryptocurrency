# Sprint 15 — слайс O.1 wave16: ревью выноса `transport::dial`

**Коммиты:** `54c8618`, `7c29822` (код), `831049d` (ревью + тикет + план/чеклист)

## Область

HTTP seed dial по `CODEBASE_REFACTORING.md` §2.2 строка #4: `retryable_connect_outcome`, `build_local_node_hello`, `attempt_seed_connect`, `SeedStatus` / `PeerHelloAck`.

## Выводы

- Вынос механический; путь **`crate::transport::build_local_node_hello`** сохранён.
- Первичное замечание: **`attempt_seed_connect`** был **`pub(crate)`** в `dial.rs` (шире прежней приватности). Исправлено: **`pub(super)`** в `dial` + приватная обёртка в **`transport.rs`** для **`transport_tick`** (дочерние модули видят приватные fn родителя).

## Тесты (оркестратор)

`cargo fmt`, `cargo test -p pwmd` — **197** OK; после `7c29822` — повторный прогон OK.

## Вердикт

**PASS** (после коммита `7c29822`)

---

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-slice-O1-wave16-review.md
token_usage:
  source: estimate
  total: 7600
  confidence: medium
notes:
  - "Первичный обзор дал PARTIAL из‑за pub(crate) attempt_seed_connect; устранено в 7c29822."
```
