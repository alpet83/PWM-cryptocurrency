# Sprint 15 — слайс O.1 wave13: ревью выноса `transport::peer_types`

**Дата:** 2026-05-02  
**Коммиты:** `da29dc5` (рефакторинг), `fa4712e` (тикет), `d516878` (ревью + delegations в тикете)

## Область

Механический перенос peer-types кластера из `crates/pwmd/src/transport.rs` в `crates/pwmd/src/transport/peer_types.rs` без изменения поведения (wave13, `CODEBASE_REFACTORING.md` §2.2, строка #1).

## Требования

- Вынос выполнен как структурный перенос; признаков функционального дрейфа по диффу не видно.
- Реэкспорты в `transport.rs`: публичные типы остаются доступны через `pub use peer_types::{…}`; внутренние (`TrustedPeer`, `DialAttemptResult`) — `pub(crate)`.
- Границы модуля соответствуют плану §2.2 (`PeerClass`, `PeerStatus`, `PeerRecord`, policy-типы, `ClassLabel`, причины close/reconnect и др.).
- Стиль: затронутые production-идентификаторы укладываются в разумный лимит длины имён; новых `unwrap`/`panic` в выносимом коде нет.

## Тесты

- Прогон оркестратором после wave13: `cargo fmt --check`, `cargo test -p pwmd` — **197** пройдено (194 lib + 3 bin), `cargo check --workspace` — OK.

## Вердикт

**PASS** — вынос корректен; API и видимость согласованы с задачей механического extraction.

---

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-slice-O1-wave13-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 8500
  confidence: medium
```
