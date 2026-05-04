# Sprint 15 — Slice 6 ревью (replay между backend, бенчи, `pwmd-ch-snap-import`)

**Вердикт:** **PASS** (approve with nits — см. §Ниты).

## Кратко

- Эквивалентность JsonFile vs ClickHouse при mock HTTP: **`snap_ch_wire_jsonfile_mock`** (`snap_wire_json_bytes` после полной валидации загрузки).
- **`import_snapshot_file`** согласован с **`ch_save`** через общий **`encode_snap_data_txt`**.
- **`pwmd-ch-snap-import`** и реэкспорты **`SnapChCfg`** / хелперов под **`clickhouse-snapshot`** — контракт оператора документирован (**`sprint-15-slice-6-bench.md`**, **`issues-report`**).
- Префлайт **`target/debug`** (**`git_bash_exec`** + **`du`**, порог 4 GiB) зафиксирован в **`AGENT_PROMPT_testing.md`** и **`sprint-15-slice-6-testing.md`**.

## Ниты

1. ~~Имя теста превышало бюджет сегментов~~ — переименовано в **`snap_ch_wire_jsonfile_mock`** (≤ 5 сегментов).
2. На огромных снимках ответ CH целиком в память — известный операторский риск прототипа (не блокер slice 6).

## Тестирование

См. **`docs/reviews/sprint-15-slice-6-testing.md`** (`cargo fmt`, `cargo test --workspace`, targeted pwmd CH test, **`cargo check`** для **`pwmd-ch-snap-import`**, **`cargo bench --no-run`**).
