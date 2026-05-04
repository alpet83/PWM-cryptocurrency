# Sprint 14 - Slice 30 - testing report

## Scope

Проверен slice `log filename node_id` по 5 заявленным проверкам:

1. expansion + sanitization для `{node_id}`;
2. fallback `node-unknown`;
3. default template включает `{node_id}`;
4. консистентность docs/help для нового placeholder;
5. таргетные logging/config тесты и check-команда.

## Checks

### 1) `{node_id}` expansion и sanitization

- `logging::tests::template_expands_node_id_placeholder_with_sanitization` — **PASS**
- Подтверждено поведение: небезопасные символы в `node_id` заменяются на `_`, шаблон имени файла формируется корректно.

### 2) fallback `node-unknown`

- `logging::tests::template_uses_node_id_fallback_when_unavailable` — **PASS**
- При `runtime_node_id=None` имя файла использует `node-unknown`.

### 3) default template содержит `{node_id}`

- `config::tests::logging_defaults_match_slice30_template` — **PASS**
- Проверка фиксирует default `"{date}/{log_name}-{node_id}-{time}.log"` при `log_dir=logs`.

### 4) docs/help consistency

- Документация: **PASS**
  - `docs/LOGGING_STYLE.md` содержит default `logs/{date}/{log_name}-{node_id}-{time}.log` и контракт по `{node_id}`.
  - `docs/pwmd.md` содержит `{node_id}` в placeholder list, default шаблон и правило sanitization/fallback.
- CLI help command: **FAIL (environment/build blocked)**
  - `cargo run -p pwmd -- --help` не смог пересобрать бинарь: `failed to remove ... pwmd.exe (os error 5)`.
  - Запуск текущего `target/debug/pwmd.exe --help` показал старый help (без `{node_id}`, старый default с `_`), что выглядит как stale binary.
  - Попытка изолированной пересборки с `CARGO_TARGET_DIR=target_slice30` прервана ошибкой `no space on device`.

### 5) targeted tests + check

- `cargo test -p pwmd logging::tests::template_expands_node_id_placeholder_with_sanitization -- --nocapture` — **PASS**
- `cargo test -p pwmd logging::tests::template_uses_node_id_fallback_when_unavailable -- --nocapture` — **PASS**
- `cargo test -p pwmd config::tests::logging_defaults_match_slice30_template -- --nocapture` — **PASS**
- `cargo test -p pwmd logging::tests::template_expands_subdir_placeholders -- --nocapture` — **PASS**
- `cargo check -p pwmd` — **PASS**

## Verdict

**FAIL** (по формальному набору проверок в этом окружении):

- Тесты и `cargo check` для Slice30 проходят.
- Но command-level verification для актуального `--help` не завершена успешно из-за блокировок окружения (locked/stale `pwmd.exe` + нехватка диска на изолированной пересборке).
