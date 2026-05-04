# Review: pwm-cli wave11 main → `rpc_helpers` (`1715c49`)

**Commits:** `1715c49bf1237e9d4659899c55bc22a077eee211` (**pwm-coding**), `20e4c5887b662539aec864ff977ab1a2ff97133d` (оркестратор — документирование волны + cleanup импортов без `unused_imports`).  

## powershell `# git-handoff`

```powershell
git show --stat 20e4c5887b662539aec864ff977ab1a2ff97133d
```

**Scope:** §2.3 строка 10 аудита — nonce/preflight/post tx/handoff JSON рядом с остальными RPC-хелперами.

_Источник: pwm-review по диффу **pwm-coding** + локальная приёмка оркестратора (**pwm-testing**)._ 

## Requirements fit

Поведение и тексты ошибок сохранены (перенос без рефакторинга семантики). Импорты в **`cmd_*`** указывают на **`crate::rpc_helpers`** там, где нужно.

Приёмка: **`cargo fmt --all -- --check`**, **`cargo test -p pwm-cli`** (141), **`cargo check --workspace`** — PASS (оркестратор перепроверил после правки импортов).

## Style / safety

**Approve.** Корень crate теряет лишние реэкспорты — меньше шума в `cargo check` для binary target.

## Verdict

**PASS.**
