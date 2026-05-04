# Sprint 15 — слайс O.1 wave14: ревью выноса `transport::metrics`

**Дата:** 2026-05-02  
**Коммиты:** `26db44b`, `e40130b` (код/тикет), `e1bc30d` (ревью + метаданные)

## Область

Механический перенос metrics-кластера из `crates/pwmd/src/transport.rs` в `crates/pwmd/src/transport/metrics.rs` (`CODEBASE_REFACTORING.md` §2.2, строка #2).

## Выводы

- Перенос выглядит механическим: типы счётчиков/snapshot и связанные helpers вынесены; в `transport.rs` — `mod metrics` и реэкспорты, внешний контракт сохранён.
- Видимость: публичные snapshot/counters через `pub use metrics::{…}`, внутреннее состояние — `pub(crate)` / `pub(super)` по необходимости.
- Замечания неблокирующие: возможная связность через `use super::*` / вызовы в родительский модуль — технический долг на будущее уплотнение границ, не признак drift поведения.

## Тесты (оркестратор)

После wave14: `cargo fmt --check`, `cargo test -p pwmd` — **197** OK, `cargo check --workspace` — OK.

## Вердикт

**PASS**

---

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-slice-O1-wave14-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 7800
  confidence: medium
```
