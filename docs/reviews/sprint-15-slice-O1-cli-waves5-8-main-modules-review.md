# Review: pwm-cli `main.rs` waves 5–8 (`91e9796`)

**Commits:** `91e97964c8deb5a07282169b2f8c098164b45b60` (рефакторинг — `cmd_tx`, `cmd_roaming`, `cmd_wallet`, `cmd_book`), `f517e4b045d91c03607c4a97f8ecb78129659ce3` (обновление тикета делегацией pwm-coding), `0dda232d646d88fe11358d33cb2dea778e7d4d5d` (оркестратор — checklist/plan/issues/review и финальный `commits[]`).  
**Scope:** вынос подкоманд §2.3 строки 5–8 из **`crates/pwm-cli/src/main.rs`** в отдельные модули по **`CODEBASE_REFACTORING.md`**.

_Запись оркестратора по сводке **pwm-coding** и локальной приёмке **pwm-testing**._

## Requirements fit

Четыре модуля подключены из корня binary crate; **`main.rs`** остаётся диспетчеризацией **`Cli`** / **`Cmd`**. **`src/tests/mod.rs`** не сломан (141 тест в дереве `tests`). Приёмка на хосте: **`cargo fmt --all -- --check`**, **`cargo test -p pwm-cli`** (141), **`cargo check --workspace`** — PASS.

## Style / safety

**Nit:** блоки **`#[cfg(test)] pub(crate) use`** в **`main.rs`** для символов, нужных только юнит-тестам — тот же компромисс, что на ранних волнах TUI/cli; долгосрок — явные пути **`crate::cmd_*::`** в тестах и сужение видимости.

## Verdict

**Approve with nits.**

## powershell `# git-handoff`

```powershell
git show --stat 0dda232d646d88fe11358d33cb2dea778e7d4d5d
```
