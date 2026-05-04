# Review: transport tests split (`14ab8972`)

**Commit:** `14ab897206fbfd6c1aae3fc9665d3961001938b9`  
**Scope:** `crates/pwmd/src/transport/tests.rs` → `transport/tests/{mod,harness,production,wire_decode,peer_harness}.rs`; в `transport.rs` по-прежнему `#[cfg(test)] mod tests;` (Rust подхватывает `tests/mod.rs`).

_Источник: pwm-review (вердикт approve with nits); файл сохранён оркестратором — субагент был в Ask mode и не смог выполнить `git commit`._

## 1. Scope recap

- Тикет: механическое разбиение монолитного `transport/tests.rs` на поддерево: общий harness, tokio-интеграции, unit wire/decode; только код под `#[cfg(test)]`.
- MVP: `docs/reviews/sprint-15-slice-O-checklist.md` §C — декомпозиция **transport** §2.2 (продолжение волн O.1).

## 2. Requirements fit

- Контракт `transport.rs` не менялся: модуль всё ещё **`tests`**.
- Структура совпадает с брифом: **`harness.rs`** — общие типы и peer-only логика; **`production.rs`** — интеграционные тесты и синхронный тест **`production_close_detail_includes_low_level_error`**; **`wire_decode.rs`** — `decode_wire_msg_payload`; **`peer_harness.rs`** — двухузловой harness.
- По диффу перенос без смены смысла ассертов; добавлены границы модулей (**`pub(super)`**, **`use super::super::*`**, явный **`crate::handshake::NodeHello`**).

## 3. Style

- Прод-идентификаторы не трогались. **`pub(super)`** в harness — уместно.
- **Nit:** в **`production.rs`** смешаны тяжёлые интеграционные тесты и лёгкий unit **`production_close_detail_includes_low_level_error`**; при желании можно вынести unit в отдельный подмодуль — не обязательно для приёмки.

## 4. Safety

- Только тестовое дерево; крипто/wire в проде в этом коммите не менялись.

## 5. Tests

- **pwm-testing PASS** (`fmt`, `cargo test -p pwmd`, `cargo check --workspace`; 194 lib + 3 bin тестов pwmd).

## 6. Verdict

**Approve with nits** (нит — группировка в §3). Для конвейера: приёмлемо как **PASS** по качеству с фиксацией нита здесь.
