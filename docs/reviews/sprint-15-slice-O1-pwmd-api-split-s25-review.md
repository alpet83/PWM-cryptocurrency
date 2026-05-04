# Review: pwmd wave18 `api/` split §2.5 (`54cc0bb`)

**Commits:** `54cc0bb24a473d9150af32b2091c58bdd963cab5` (**pwm-coding**), `e7b492c36d712b6e82a40bd774d433a8b9040a74` (тикет); документирование — см. **`commits[]`** после оркестратора.

## Scope

Монолит **`api.rs`** заменён каталогом **`api/`**: **`types`**, **`router`**, **`common`**, **`handlers_*`** по **`CODEBASE_REFACTORING.md` §2.5**.

## Requirements fit

- **`lib.rs`** **`pub use api::{ router, … V1_TX_BODY_LIMIT }`** — без изменений списка (по сверке pwm-review с базой коммита).
- **`relay.rs`:** **`crate::api::ExportHandoffOut`** — через **`pub use`** в **`api/mod.rs`** из **`types`**.
- Таблица маршрутов в **`router.rs`** — паритет с прежним **`api.rs`** (визуальная сверка ревью).

## Tests

**pwm-coding** / тикет: **`cargo test -p pwmd`**; оркестратор повторил **`cargo test -p pwmd`** — **194 + 3** integration, OK.

## Verdict

**PASS.**

## powershell `# git-handoff`

```powershell
git show --stat 0006b6f431ba38f196bb6d42318859f8148c86a5
```
