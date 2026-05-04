# Sprint 15 — S15-O: codebase cleanup — Testing handoff

## Wave 2 — S15-O.1 wave2 (`pwm-testing`, 2026-05-01)

Проверка по тикету **`tasks/20260504-s15-slice-O1-wave2-tui-modules.json`** после **`git log`** (ожидание коммитов pwm-coding или контроль через ~20 мин).

### Поиск

| Метод | Результат |
|--------|-----------|
| **`user-cqds_mcp_mini`** `cq_project_ctl#select_project` (`project_id`: **5**) | **OK** |
| **`cq_files_ctl#start_grep`** (`query`: `rpc_account`) | **OK** — `status: ok`, продолжение через `chunk_continuation`; на первых чанках **hits пустые** (совпадений в проиндексированном объёме не найдено при частичном скане). |
| **`cq_project_ctl#fetch_result`** (второй чанк, `offset`: 50) | **OK** — `status: ok`, `hits: []`, `next_offset`: 100. |
| **Fallback** `rg` (`P:\opt\docker\PWM-cryptocurrency\crates\pwm-tui`) | **OK** — например `crates/pwm-tui/src/tx_submit.rs` (локальное дерево); см. примечание про Git ниже. |

### Коммиты (`git log -8`)

| Hash | Subject |
|------|---------|
| `ff1357e` | chore(tasks): S15-O.1 wave2 ticket + kickoff slice |
| `70b4113` | docs: S15-O.1 wave1 checklist, issues-report, ticket O.1 |
| `c6d5b09` | refactor(pwm-tui): extract models module from main.rs |
| `995194b` | refactor(pwm-tui): extract status module from main.rs |
| `8e0ba54` | refactor(pwm-tui): extract config module from main.rs |
| `3b7253d` | chore(tasks): close S15-O, open O.1 modular wave; CQDS/env notes |
| `b86cf16` | chore(tasks): record S15-O-B commits on slice ticket |
| `0ac777c` | feat(pwm-core,cli,tui): S15-O-B display, rpc, wallet_io |

**Примечание по wave2-коду:** в проверенном локальном клоне файлы `modals.rs`, `wallet.rs`, `rpc_account.rs`, `signing.rs`, `tx_submit.rs` присутствуют на диске, но могут быть **не закоммичены** (`git status` → неотслеживаемые). В **`git log -8` нет отдельного коммита `refactor(pwm-tui): …` для wave2** — вершина истории для этой проверки совпадает с тикетом **`ff1357e`**. Прогоны ниже относятся к **текущему рабочему дереву** (включая неотслеживаемые файлы, если они есть).

### Форматирование

- `cargo fmt --check` — **PASS**.

### Тесты

Репозиторий: `P:\opt\docker\PWM-cryptocurrency`. Фоновые `pwmd` / `pwm-tui` не поднимались.

| Команда | Итог | Примечание |
|---------|------|------------|
| `cargo test -p pwm-tui` | **PASS** | 88 passed; ~5 s |
| `cargo test --workspace` | **PASS** | полный прогон ~114 s; все crate зелёные |

### Риски / долги

- После появления **закоммиченных** коммитов wave2 в `main` — повторить `cargo test -p pwm-tui` и при необходимости **`start_grep`** на CQDS после **rebuild_index** (индекс CQDS не отражает непушенный локальный диск).
- Декомпозиция «fat-file» по-прежнему без отдельных UI-автотестов сверх регрессии (см. `AGENT_PROMPT_testing.md` § TUI).

### Participation (machine-copy)

```yaml
participation:
  agent: pwm-testing
  slice: S15-O.1-wave2
  result: PASS
  date: 2026-05-01
  baseline_head: ff1357e
  note: wave2_files_on_disk_may_be_untracked; rerun_tests_after_pwm-coding_commits
  artifacts:
    - docs/reviews/sprint-15-slice-O-testing.md
    - tasks/20260504-s15-slice-O1-wave2-tui-modules.json
  commands:
    - cmd: cargo fmt --check
      outcome: PASS
    - cmd: cargo test -p pwm-tui
      outcome: PASS
      tests: 88
    - cmd: cargo test --workspace
      outcome: PASS
  mcp:
    cqds_select_project_5: OK
    cqds_start_grep_rpc_account: OK
    cqds_fetch_result_chunk2: OK
  cleanup: no background daemons
```

---

## Round 3 — 2026-05-01 (env recovery)

Проверка после восстановления **CQDS** и **Git Bash**: MCP доступен; прогон против ветки с коммитами **группы B** (`5099486`, `0ac777c`, …).

### Поиск

| Метод | Результат |
|--------|-----------|
| **`user-cqds_mcp_mini`** `cq_project_ctl#select_project` (`project_id`: **5**) | **OK** — проект выбран. |
| **`cq_files_ctl#start_grep`** (`query`: `TextInput`) | **OK** — попадания (например `crates/pwm-tui/src/main.rs`, `struct TextInput`). |
| **`cq_project_ctl#fetch_result`** (`chunk_continuation` со второго чанка) | **OK** — ответ `status: ok`, пустые hits на этом смещении ожидаемы при продолжении сканирования файлов. |
| **Fallback** `rg` | не использовался — MCP отвечает. |

### Коммиты (`git log -5`)

| Hash | Subject |
|------|---------|
| `3b7253d` | chore(tasks): close S15-O, open O.1 modular wave; CQDS/env notes |
| `b86cf16` | chore(tasks): record S15-O-B commits on slice ticket |
| `0ac777c` | feat(pwm-core,cli,tui): S15-O-B display, rpc, wallet_io |
| `5099486` | feat(tui): shared TextInput for modals and SendForm |
| `60df329` | chore(tasks): S15-O record group A commit 1b6c5a0 |

### Форматирование

- `cargo fmt --check` — **PASS**.

### Тесты

Репозиторий: `P:\opt\docker\PWM-cryptocurrency`. Фоновые `pwmd` / `pwm-tui` не поднимались.

| Команда | Итог | Примечание |
|---------|------|------------|
| `cargo test --workspace` | **PASS** | ~135 s; предупреждение компиляции: `pwm-tui` неиспользуемый импорт `Duration` в `main.rs` (~3969) — на результат тестов не влияет |

### Риски / долги

- Как в предыдущих раундах: «fat-file» декомпозиция не верифицируется автотестами сверх регрессии.
- Убрать предупреждение `unused import: Duration` в `pwm-tui` при следующем касании файла (не блокер Round 3).

### Participation (machine-copy)

```yaml
participation:
  agent: pwm-testing
  slice: S15-O-env-recovery
  result: PASS
  round: 3
  date: 2026-05-01
  artifacts:
    - docs/reviews/sprint-15-slice-O-testing.md
  commands:
    - cmd: cargo fmt --check
      outcome: PASS
    - cmd: cargo test --workspace
      outcome: PASS
  mcp:
    cqds_start_grep: OK
    cqds_fetch_result: OK
  cleanup: no background daemons
```

---

## Round 2 — 2026-05-01

**NOTE:** Отдельного коммита pwm-coding по **группе B** в `git log -5` нет; прогон выполнен против **baseline** после группы A (`1b6c5a0`) и записи тикета (`60df329`).

### Поиск

| Метод | Результат |
|--------|-----------|
| **`user-cqds_mcp_mini`** `cq_files_ctl#start_grep` (`project_id`: 5) | **FAILED**: `All connection attempts failed` — CQDS недоступен из сессии. |
| **Fallback** `rg` из `P:\opt\docker\PWM-cryptocurrency` | **OK**: совпадения в `docs/` (`MVP-checklist.md`, `CODEBASE_REFACTORING.md`, sprint-15 slice-O docs). |

### Коммиты (`git log -5`)

| Hash | Subject |
|------|---------|
| `60df329` | chore(tasks): S15-O record group A commit 1b6c5a0 |
| `1b6c5a0` | S15-O: group A cleanup (TUI xflow inline, transport dial, --shard deprecated, TODO policy) |
| `823bca3` | chore(tasks): S15-O ticket commit ref |
| `6204dc2` | docs: rename ROUMING* to ROAMING*; add S15-O codebase cleanup plan |
| `7d32919` | chore(tasks): S3.17 closeout commit ref 5672fdd |

### Форматирование

- `cargo fmt --check` — **PASS**.

### Тесты

Репозиторий: `P:\opt\docker\PWM-cryptocurrency`. Фоновые `pwmd` / `pwm-tui` не поднимались.

| Команда | Итог | Примечание |
|---------|------|------------|
| `cargo test -p pwm-core` | **PASS** | 78 passed |
| `cargo test -p pwm-tui` | **PASS** | 92 passed |
| `cargo test -p pwm-cli` | **PASS** | 142 passed |
| `cargo test -p pwmd --lib` | **PASS** | 194 passed |
| `cargo test --workspace` | **PASS** | полный прогон (~101 s); все crate зелёные |

### Риски / долги

- Как в Round 1: нет автоматической верификации «fat-file» декомпозиции — только регрессия по тестам.
- Группа B чеклиста O всё ещё не закрыта кодом — см. тикет / follow-up.

### Participation (machine-copy)

```yaml
participation:
  agent: pwm-testing
  slice: S15-O-B
  result: PASS
  round: 2
  date: 2026-05-01
  baseline_note: no_group_B_commit_in_log_top5
  artifacts:
    - docs/reviews/sprint-15-slice-O-testing.md
    - tasks/20260502-s15-slice-O-codebase-cleanup.json
  commands:
    - cmd: cargo fmt --check
      outcome: PASS
    - cmd: cargo test -p pwm-core
      outcome: PASS
      tests: 78
    - cmd: cargo test -p pwm-tui
      outcome: PASS
      tests: 92
    - cmd: cargo test -p pwm-cli
      outcome: PASS
      tests: 142
    - cmd: cargo test -p pwmd --lib
      outcome: PASS
      tests: 194
    - cmd: cargo test --workspace
      outcome: PASS
  cleanup: no background daemons
```

---

## Round 1

### Scope

Рефакторинг / очистка по слайсу **O** (`CODEBASE_REFACTORING`, чеклист `docs/reviews/sprint-15-slice-O-checklist.md`). Прогон затронутых crate после последних коммитов по тикету.

### Поиск

| Метод | Результат |
|--------|-----------|
| **`user-cqds_mcp_mini`** `cq_files_ctl#start_grep` (`project_id`: 5) | **FAILED**: `All connection attempts failed` — сессия без CQDS. |
| **Fallback** `rg` из `P:\opt\docker\PWM-cryptocurrency` | **OK**: совпадения `S15-O` / `slice-O` / `CODEBASE_REFACTOR` в `docs/`, `tasks/` (план, чеклист, `CODEBASE_REFACTORING.md`, `MVP-checklist.md`). |

### Коммиты (`git log -3`)

| Hash | Subject |
|------|---------|
| `823bca3` | chore(tasks): S15-O ticket commit ref |
| `6204dc2` | docs: rename ROUMING* to ROAMING*; add S15-O codebase cleanup plan |
| `7d32919` | chore(tasks): S3.17 closeout commit ref 5672fdd |

Свежие коммиты по **S15-O** / задаче: **`823bca3`**, **`6204dc2`** (доки и тикет; отдельного крупного коммита «рефакторинг кода» от pwm-coding в топ-3 нет).

### Форматирование

- `cargo fmt --check` — **PASS** (exit 0).

### Тесты

Репозиторий: `P:\opt\docker\PWM-cryptocurrency`. Локальный shell (PowerShell), без фоновых `pwmd` / `pwm-tui`; зависших процессов не создавалось.

| Команда | Итог | Примечание |
|---------|------|------------|
| `cargo test -p pwm-tui` | **PASS** | 92 passed; ~3.5s |
| `cargo test -p pwmd --lib` | **PASS** | 194 passed; ~14s |
| `cargo test -p pwm-cli` | **PASS** | 142 passed; ~56s (кратковременный file lock на build dir — ожидаемо при параллельной сборке) |

**Итог Round 1:** `cargo test` по указанным пакетам — зелёный; полный `cargo test --workspace` не запускался (достаточный минимум по запросу).

### Риски / долги

- Нет автоматической верификации «fat-file» декомпозиции — только регрессия по тестам.
- При недоступности CQDS повторить `start_grep` при следующей сессии при необходимости индекс-поиска.

### Participation Round 1 (archive)

```yaml
participation:
  agent: pwm-testing
  result: PASS
  artifacts:
    - docs/reviews/sprint-15-slice-O-testing.md
    - tasks/20260502-s15-slice-O-codebase-cleanup.json
  commands:
    - cmd: cargo fmt --check
      outcome: PASS
    - cmd: cargo test -p pwm-tui
      outcome: PASS
      tests: 92
    - cmd: cargo test -p pwmd --lib
      outcome: PASS
      tests: 194
    - cmd: cargo test -p pwm-cli
      outcome: PASS
      tests: 142
  cleanup: no background daemons; nothing to kill
  token_usage:
    source: estimate
    confidence: low
```
