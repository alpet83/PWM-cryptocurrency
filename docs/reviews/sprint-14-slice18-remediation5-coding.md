# Sprint 14 — Slice 18 — Remediation 5 (coding)

## Что сделано

- Усилен фильтр progress-строк в file sink: строки, оканчивающиеся на carriage-return с хвостом из `\n`/пробелов/табов, больше не попадают в файл.
- Добавлен альтернативный placeholder `~UT` в шаблон `--log-file-template` с форматом `HH:MM:SS.mmm` (UTC).
- Обновлены краткие doc-заметки в `docs/pwmd.md` и `docs/LOGGING_STYLE.md`.

## Тесты

- Добавлен тест на раскрытие `~UT` в `expand_log_template_path`.
- Расширен тест file sink: проверяются варианты progress-строк с `\r`, `\r\n` и `\r` + хвостовые пробелы/перенос.
