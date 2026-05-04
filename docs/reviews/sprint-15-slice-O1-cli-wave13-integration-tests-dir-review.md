# Review: pwm-cli wave13 `crates/pwm-cli/tests/` subprocess smoke (`47bff57`)

**Commits:** `47bff575fcce811f84e504acf59b327cba08d58f` (**pwm-coding**), `72c769a433db85e16091f25ed760e0270a18affe`, `1fe010e6d43a28770dbd55d3f7ddacf738d45267` (chores тикета), `e61538e4c81b265a8e59f0dc8d38f308412ff474` (оркестратор — checklist/plan/ticket/review/`CODEBASE_REFACTORING`).

## Scope

Тикет `tasks/20260607-s15-slice-O1-cli-wave13-integration-tests-dir.json`: каталог **`crates/pwm-cli/tests/`**, чёрный ящик через **`std::process::Command`** и **`CARGO_BIN_EXE_pwm`**. Фактически **`tests/cli_smoke.rs`**: `--help`, `key-gen --help`, `key-gen` (64 hex). **`src/tests/mod.rs`** не трогали — ок по границам wave13.

## Requirements fit

Цель выполнена; прод-код не менялся. **`cargo fmt`**, **`cargo test -p pwm-cli`**, **`cargo check --workspace`** — PASS (**pwm-coding** / **pwm-testing** по телу тикета).

## Safety / subprocess

Локальный spawn без RPC в этих трёх тестах; **`trim_end_matches`** для CRLF разумно; **`option_env!("CARGO_BIN_EXE_pwm")`** — принятый паттерн Cargo для интеграционных тестов бинарника **`pwm`**.

## Nit

Подстрочные проверки help (**`Usage:`**, **`seed`** и т.д.) могут сломаться при косметическом изменении текста clap без изменения семантики CLI — для smoke допустимо; при регрессиях можно ослабить до более общих условий.

## Verdict

**PARTIAL** — approve with nit выше (gate для merge ок при принятии хрупкости строк).

## powershell `# git-handoff`

```powershell
git show --stat e61538e4c81b265a8e59f0dc8d38f308412ff474
```
