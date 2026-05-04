# Review: pwmd lib inline tests split waves33–36 (`6b5eaec`)

**Commits:** `6b5eaec639097e0f1d17237eabcd3ef98b8972a6` (refactor), `db2c5d2efd7a2df5ae196fcdc34aeddfb30aa0ac` (ticket chore).  
**Scope:** inline `#[cfg(test)] mod tests { … }` в `crates/pwmd/src/lib.rs` → дерево `crates/pwmd/src/tests/` (`mod.rs`, `prelude.rs`, `helpers.rs`, `http_status.rs`, `transport_peer.rs`, `http_export.rs`, `snapshot_roaming.rs`); хвост `slice20_e2e_tests` сохранён.

_Источник: pwm-review (approve with nits); файл сохранён оркестратором — субагент в Ask mode._

## Requirements fit

Цели тикета соблюдены: отдельный `#[cfg(test)] mod tests;`, ≥4 смысловых подмодуля, прод-каркас `lib.rs` без изменения поведения API. Приёмка: **fmt**, **`cargo test -p pwmd`** (194+3), **`cargo check --workspace`** — PASS.

## Style / safety

Тестовые имена могут быть длиннее прод-правил — ок. **`tests/prelude.rs`** с **`pub(crate) use crate::*`** ускоряет миграцию; **nit:** долгосрок — сузить до явного списка импортов (техдолг).

Риск «утечки» тестового модуля в прод-сборку стандартно низкий (`#[cfg(test)]`).

## Verdict

**Approve with nits** (prelude wildcard; опционально вынести правки `issues-report.md` в отдельный коммит в будущем).
