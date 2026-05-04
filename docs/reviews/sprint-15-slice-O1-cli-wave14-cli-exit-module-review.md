# Review: pwm-cli wave14 `cli_exit` (`eee1449`)

**Commits:** `eee144927c72891e4f2efac425b98ce4218e447f` (**pwm-coding**), `2642bff86be7ea6e2476969effaa3e10cc1e5a49` (тикет), `a21782940eb4d0cf6f29850fc77b185252a673f1` (оркестратор — checklist/plan/review/`CODEBASE_REFACTORING`/тикет).

## Scope

Вынести **`exit_user_error`** в **`cli_exit.rs`**, **`pub(crate) use`** из **`main.rs`** — минимальный дифф, **`cmd_*`** без массовых правок импортов.

## Requirements fit / safety

Поведение сохранено (`eprintln!`, **`exit(2)`**, **`!`**). **`use std::process`** перенесён из **`main.rs`** в модуль — корректно.

## Tests

По телу тикета: **fmt**, **`cargo test -p pwm-cli`**, **`cargo check --workspace`** — PASS (**pwm-testing**).

## Verdict

**PASS.**

## powershell `# git-handoff`

```powershell
git show --stat a21782940eb4d0cf6f29850fc77b185252a673f1
```
