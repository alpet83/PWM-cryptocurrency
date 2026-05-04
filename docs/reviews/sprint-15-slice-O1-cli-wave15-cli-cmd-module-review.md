# Review: pwm-cli wave15 `cli_cmd` (`767a3fb`)

**Commits:** `767a3fbf3678a76a441aac558bec84fc549c8361` (**pwm-coding**), `84d522ae481e5fc0eac22d5c6ac9aa31d8b8a75d` (тикет), `8ca19b10938955ccf55ea2773c17ca613f3d81f1` (оркестратор — checklist/plan/review/`CODEBASE_REFACTORING`/тикет).

## Scope

Вынести дерево **clap** (`Cli`, `Cmd`, `WalletCmd`, `WalletAccountCmd`) в **`cli_cmd.rs`**, реэкспорт из **`main.rs`** — без изменения семантики парсинга/help по замыслу диффа.

## Requirements fit

Реэкспорт сохраняет **`cmd_wallet`** и **`src/tests/mod.rs`**. **`pub(crate)`** на **`Cli`** и полях — нужно для доступа из **`main`** к подмодульному типу.

## Tests

**pwm-coding** и оркестратор: **`cargo fmt --check`**, **`cargo test -p pwm-cli`**, **`cargo check --workspace`** — PASS.

## Verdict

**PASS.**

## powershell `# git-handoff`

```powershell
git show --stat 8ca19b10938955ccf55ea2773c17ca613f3d81f1
```
