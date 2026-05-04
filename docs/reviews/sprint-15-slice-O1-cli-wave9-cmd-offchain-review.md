# Review: pwm-cli wave9 `cmd_offchain` (`af7cf48`)

**Commits:** `af7cf483ee8f3406df59d0df5e2f7336969fed62` (рефакторинг — модуль `cmd_offchain`, диспетчер в `main.rs`), `dedf967668a714c78176313928d1b430628a2a2c` (оркестратор — таблица §2.3, checklist/plan, тикет, ревью).  
**Scope:** строка 9 таблицы §2.3 **`CODEBASE_REFACTORING.md`**: локальные off-chain команды; фактически вынесена только **`off-demo`**.

_Запись оркестратора; приёмка **pwm-testing** на хосте._

## Requirements fit

Поведение **`pwm off-demo`** сохранено (тот же Merkle-демо и JSON на stdout). **`main.rs`** перестал импортировать **`merkle_root`** / **`sign_batch`** напрямую. Таблица §2.3 уточнена: **`off-demo`** сейчас, **`offchain-sign`** при появлении.

Приёмка: **`cargo fmt --all -- --check`**, **`cargo test -p pwm-cli`** (141), **`cargo check --workspace`** — PASS.

## Style / safety

Модуль минимальный; расширение под будущие подкоманды — тем же файлом **`cmd_offchain.rs`**.

## Verdict

**Approve.**

## powershell `# git-handoff`

```powershell
git show --stat dedf967668a714c78176313928d1b430628a2a2c
```
