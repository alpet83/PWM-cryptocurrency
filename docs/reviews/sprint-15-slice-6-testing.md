# Sprint 15 — Slice 6: pwm-testing

**Коммиты док:** префлайт в промпте — **`5664441`** (`docs/AGENT_PROMPT_testing.md`, оркестратор).

## Preflight `target/debug`

- Инструмент: **`git_bash_exec`**, **`du -sm target/debug`**, порог **4096 MiB (4 GiB)** (актуальный порог всегда в **`docs/AGENT_PROMPT_testing.md`** §Preflight).
- Прогон оркестратора до делегирования: **243 MiB**, удаления не было.

## Команды (pwm-testing субагент)

| Шаг | Результат |
|-----|-----------|
| `cargo fmt --check` | PASS |
| `cargo test --workspace` | PASS (после необходимости предсборки артефактов для e2e — см. заметку ниже) |
| `cargo test -p pwmd --features clickhouse-snapshot` + фильтр `snap_ch_wire_jsonfile_mock` | PASS |
| `cargo check -p pwmd --features clickhouse-snapshot --bin pwmd-ch-snap-import` | PASS (после правок реэкспорта **`SnapChCfg`** / **`pub`** для CSV-хелперов) |
| `cargo bench -p pwmd --bench snapshot_load --no-run` (+ `--features clickhouse-snapshot`) | PASS |

## Заметка

Первый прогон **`cargo test --workspace`** может падать на **`slice20_dual_flow_ok`**, если ещё не собраны бинарники под **`target/debug`** — достаточно **`cargo build -p pwmd -p pwm-cli`** перед матрицей (или повторить тест после первой сборки).

## Следующий шаг

**pwm-review** по изменениям Slice 6 + артефакт **`docs/reviews/sprint-15-slice-6-review.md`**.
