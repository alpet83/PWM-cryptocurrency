# Sprint 15 hotfix — ревью (snapshot handoff Import + Neutral paths)

**Вердикт:** **PASS with nits**

## Суть изменений

1. **`validate_snapshot` (`snapshot/io.rs`):** перед `apply_tx` для `TxBody::Import`, если в replay ещё нет строки в `exported_registry`, подставляется запись из **`snapshot.state.exported_registry`** (типичный handoff-only provenance). Полная целостность по-прежнему проверяется подписями блоков и сверкой `state_root` / финального `digest(state)`.

2. **Neutral default snapshot path:** `state/neutral/<listen-tag>/pwm-data.json` (`neutral_listen_dir_tag`), чтобы два Neutral-процесса с общим `--state-root` не перезаписывали один файл.

## Ниты

- Верхнеуровневая документация частично расходилась с CLI; **README «Storage Layout»** обновлён в том же спринте. Остальные `docs/*.md` — при обнаружении старых путей править точечно.

## Тесты

- **`snap_rt_handoff_import_ok`** — достаточная регрессия на описанный класс сбоев при reload.
- **`neutral_listen_tag_ok`** — формат listen-тега.
