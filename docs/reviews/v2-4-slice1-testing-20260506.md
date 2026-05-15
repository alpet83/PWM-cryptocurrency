# V2-4 Slice 1 — отчёт тестирования (pwm-testing)

**Дата:** 2026-05-06  
**Коммит:** `1ffb84015e19032de9b377fb451b1080c62b62a8` — `feat(v2-4-s1): marks display in TUI/CLI + burn pre-check`  
**Агент:** pwm-testing  

## Вердикт

**PASS**

Все указанные команды завершились успешно; контроль имён функций без нарушений.

## Префлайт артефактов

| Проверка | Результат |
|----------|-----------|
| `P:/opt/docker/rust-target-shared/debug/incremental` | Каталог существовал; размер ~1473 MiB (< 2 GiB), очистка не требовалась |
| `tools/dev/preflight_target_debug.ps1` | OK; сообщение о размере `target/debug`: ~226 MiB (ниже порога 4096 MiB) |

**CQDS:** прогоны выполнены локально в PowerShell в корне репозитория (без блокирующих вызовов `cq_process_ctl`).

## Матрица команд

| Команда | Результат |
|---------|-----------|
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test -p pwm-tui` | PASS (88 тестов: lib 1 + integration 87) |
| `cargo test -p pwm-cli` | PASS (146 unit + 3 smoke; включая `tx_burn_err_insufficient_marks`) |
| `cargo test -p pwm-core` | PASS (104 unit + 0 doc) |
| `python scripts/check_rust_fn_name_segments.py` … (список файлов из задачи) | PASS, `violations: []` по всем 8 файлам |

## Замечания

- Продуктовых дефектов и падений не выявлено.
- Интерактивный текст TUI визуально не проверялся (согласно `docs/AGENT_PROMPT_testing.md`): покрыты только автоматические тесты `pwm-tui`.

## Handoff для тикета

- `agent`: pwm-testing  
- `result`: PASS  
- `cleanup`: посторонних процессов (`pwmd`, `pwm-tui`) не запускалось — нечего останавливать

## Retest after fix commit 8e0161a

**Фикс-коммит:** `8e0161a` — `fix(v2-4-s1): correct fetch_marks URL, wire burn pre-check, add TUI marks column`

| Проверка | Результат |
|----------|-----------|
| Префлайт `P:/opt/docker/rust-target-shared/debug/incremental` | ~1.44 GiB (< 2 GiB), очистка не требовалась |
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test -p pwm-cli` | PASS (146 unit + 3 smoke) |
| `cargo test -p pwm-tui` | PASS (88: lib 1 + integration 87) |
| `cargo test -p pwm-core` | PASS (104 unit) |
| `python scripts/check_rust_fn_name_segments.py` (4 файла из задачи) | PASS, `violations: []` |

**Verdict:** PASS
