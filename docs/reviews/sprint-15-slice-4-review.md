# Sprint 15 — Slice 4 ревью (`SnapshotBackend`)

**Коммит реализации:** `20ee8fa`  
**Вердикт:** **PASS with nits**

## Кратко

- Введён **`SnapshotBackend`** (`JsonFile { path }` + заглушка **`Db`** с явной ошибкой `use JsonFile`). **`JsonFile`** делегирует в **`snapshot/io.rs`** без изменения **`validate_snapshot`** и формата JSON на диске.
- Рантайм-границы (**`lifecycle`**, **`bootstrap`**, **`api/common`**, **`relay`**) переведены на **`.load` / `.save`**; прямых **`io::load_snapshot` / `save_snapshot`** вне **`store.rs`** нет (pwm-testing).
- **`PwmdConfig::snapshot_backend()`** пока всегда возвращает **`JsonFile`** по **`data_file`** — расширение под Slice 5 ожидаемо через конфиг/поля у **`Db`**.

## Ниты

- При появлении реального **`Db`** лучше единый конструктор backend из конфига (включая **`bootstrap`**, где сейчас явный **`JsonFile`** для пути загрузки genesis+snapshot).

После ревью добавлен симметричный тест **`db_stub_save_hints_jsonfile`** (коммит см. историю после **`20ee8fa`**).

## Тестирование

- **pwm-testing:** `cargo fmt --check`, `cargo test --workspace` — PASS на **`20ee8fa`**.
