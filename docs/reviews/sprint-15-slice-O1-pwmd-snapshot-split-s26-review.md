# Review: pwmd wave19 `snapshot/` split §2.6 (`a0662c6`)

**Commits:** `a0662c608d8c7e69c8891783b40dfb59ddc8b83e` (**pwm-coding**), `0de6ac4fd2d8fa0835d2a69a9251f2aaa51c037f` (тикет), `710f79b39f8709fcdad5aab837660c13c9026b1e` (**docs(s15-o1)** closeout).

## Scope

Монолит **`snapshot.rs`** заменён каталогом **`snapshot/`**: **`types`**, **`io`**, **`genesis`**, фасад **`mod.rs`** по **`CODEBASE_REFACTORING.md` §2.6.

## Requirements fit

- **`lib.rs`:** **`pub use snapshot::load_genesis_bundle`** — сохранено.
- **`bootstrap.rs`:** `load_genesis_bundle`, `load_snapshot` через **`crate::snapshot::`** — OK.
- **`lifecycle.rs`:** `load_snapshot`, `save_snapshot` — OK.
- **`relay.rs`:** **`crate::snapshot::save_snapshot`** — OK.
- **`api/common.rs`:** **`save_snapshot`** — OK.
- **`tests/prelude.rs`:** `load_snapshot`, `save_snapshot`, `snapshot_genesis_accounts`, `SnapshotData`, `SnapshotRoamingWire`, `SNAPSHOT_VERSION` — OK.

Фасад использует **`#![allow(unused_imports)]`** из‑за реэкспортов, нужных в тестовом дереве и не всегда видимых при **`cargo check`** библиотеки без тестов — приемлемо для механического сплита; при желании позже можно сузить атрибуты.

## Tests

**pwm-coding:** `cargo fmt --all -- --check`, `cargo test -p pwmd`, `cargo check --workspace` — OK. Оркестратор повторил **`cargo test -p pwmd`** — **194 + 3** integration, OK.

## Verdict

**PASS.**

## powershell `# git-handoff`

```powershell
git show --stat 710f79b39f8709fcdad5aab837660c13c9026b1e
```
