# Review: pwm-cli wave10 `cmd_addr` (`759284e`)

**Commits:** `759284e9d2d592e80337e7b3c0cb082d0ffa5960` (рефакторинг — `cmd_addr.rs`, …), `9bda0a501d082a01a36017cd4b4664ae926bad16` (оркестратор — аудит §2.3 строка 4, checklist/plan, тикет).  
**Scope:** §2.3 строка 4 аудита — **`addr-derive`** / **`addr-bruteforce`** и связанные хелперы (persist/resume/summary/auto tx-init).

_Запись оркестратора; приёмка **pwm-testing** на хосте._

## Requirements fit

Поведение команд и вывод не менялись. Юнит-тесты для resume/persist/format/RPC-hint переведены на **`crate::cmd_addr::`** где нужно.

В **`main.rs`** добавлены **`pub(crate) use`** для **`http_client_for_rpc`**, **`resolve_wallet_out_path`**, **`DomainMatchMode`** — восстановление контракта для **`cmd_tx`** / **`cmd_roaming`** и **`tests`** после снятия прямых `use cli_config::…` из корня.

Приёмка: **`cargo fmt --all -- --check`**, **`cargo test -p pwm-cli`** (141), **`cargo check --workspace`** — PASS.

## Style / safety

**Nit:** долгосрок — часть общих парсеров/форматов ошибок можно собрать в отдельный модуль (чеклист «error_format»), чтобы разгрузить корень crate.

## Verdict

**Approve with nits.**

## powershell `# git-handoff`

```powershell
git show --stat 9bda0a501d082a01a36017cd4b4664ae926bad16
```
