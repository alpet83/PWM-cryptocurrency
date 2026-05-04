# Review: pwm-cli wave12 `cli_parse` / `signer` / `wallet_shell` (`8fd29ad`)

**Commits:** `8fd29adf1001deff7b764cbbc4e9d1621287498c` (**pwm-coding**), `e74aac96afa527ba67e792fd9eb5e03a7d38fed6` (оркестратор — checklist/plan/ticket/review/`CODEBASE_REFACTORING`).  

## powershell `# git-handoff`

```powershell
git show --stat e74aac96afa527ba67e792fd9eb5e03a7d38fed6
```

**Scope:** §2.3 — разгрузка **`main.rs`**: парсинг ввода/seed/домена, пайплайн подписанта, оболочка wallet-показа и связанные проверки домена/derivation/bruteforce-профиля.

_Источник: pwm-review по сводке **pwm-coding**; приёмка совпадает с прогоном **pwm-testing** в теле **pwm-coding**._

## Requirements fit

Три новых модуля; **`Cli`/`Cmd`/wallet enums** и **`exit_user_error`** остаются в **`main.rs`**. **`cargo test -p pwm-cli`** (141) и **`cargo check --workspace`** — PASS (**pwm-coding**).

## Style / safety

**Nit:** у **`TxSignerSource`** поля **`pub(crate)`** — необходимый компромисс для доступа из **`cmd_tx`** / **`cmd_roaming`** и тестовых литералов после выноса типа из корня crate; долгосрок — узкие accessors или тип только внутри **`signer`** + facade API.

## Verdict

**PASS.**
