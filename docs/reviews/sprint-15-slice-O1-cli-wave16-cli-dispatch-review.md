# Review: pwm-cli wave16 `cli_dispatch` (`db519a2`)

**Commits:** `db519a27a3645fdaf2ea22d0104bed7172bb00bf` (**pwm-coding**), `61219e9ddf6d9908a54e41e10d6b0f14d716488d` (тикет), `baee67ef937ca538c8c0ff75d70adb2064b84073` (оркестратор — checklist/plan/review/`CODEBASE_REFACTORING`/тикет).

## Scope

Вынести подготовку после **`Cli::parse()`** и полный **`match cli.cmd`** в **`cli_dispatch::run`**, **`main.rs`** — реэкспорты и **`main()` → cli_dispatch::run(Cli::parse())**.

## Requirements fit

Паритет с **`main` до рефакторинга**: **`rpc_base`**, клоны passphrase / **`upgrade_wallet`**, те же вызовы **`cmd_*`**. **`WalletAccountCmd`** не импортируется в **`cli_dispatch`** — в этом **`match`** не используется; реэкспорт из **`main.rs`** сохранён для остального crate.

## Tests

**pwm-testing:** fmt, **`cargo test -p pwm-cli`**, **`cargo check --workspace`** — PASS по тикету.

## Verdict

**PASS.**

## powershell `# git-handoff`

```powershell
git show --stat baee67ef937ca538c8c0ff75d70adb2064b84073
```
