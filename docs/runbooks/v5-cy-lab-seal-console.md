# V5 CY lab seal console

Цель: одна команда для оператора и агентов, которая делает RPC manual-seal шаг и возвращает единый JSON window с логами proposer + attester.

## Когда использовать

- Для ручного debug-прохода `preflight -> lease -> propose -> gate_wait -> seal_commit`.
- Когда не хочется вручную сводить HTTP response и два лога через `curl`/`Select-String`.
- Только для lab loopback `http://127.0.0.1:3030`.

## Запуск

```powershell
python scripts/cy_lab_seal_console.py discover
python scripts/cy_lab_seal_console.py status
python scripts/cy_lab_seal_console.py control --mode manual_rpc --verbose-default
python scripts/cy_lab_seal_console.py step preflight --verbose
python scripts/cy_lab_seal_console.py step propose --verbose
python scripts/cy_lab_seal_console.py step gate_wait --timeout-ms 5000 --verbose
python scripts/cy_lab_seal_console.py step seal_commit --verbose
```

## Формат вывода

Каждый one-shot вызов печатает один JSON-объект со следующими полями:

- `ok`
- `cmd`
- `step`
- `ts_utc`
- `duration_ms`
- `rpc`
- `rpc_meta`
- `window.proposer`
- `window.attester`
- `summary`
- `warnings`

`window.*.events` содержит структурированные строки лога с `kind`, `ts_log`, `fields` и `event_id`.

## Watch mode

Для непрерывного наблюдения используйте JSONL:

```powershell
python scripts/cy_lab_seal_console.py watch --interval-ms 500 --max-ticks 120
```

`watch` печатает по одному JSON-объекту на тик, поэтому удобно направлять stdout в файл или MCP-обвязку.

## Ожидаемая семантика

- `discover` возвращает найденные proposer/attester log paths и `rpc_meta.reachable`.
- `status` и `step` берут window только с последнего сохранённого byte offset, а не читают весь лог заново.
- Первый вызов на свежем state-файле стартует с конца текущих логов, чтобы не тащить историю целиком.
- Если active log rotation происходит во время окна, offset сбрасывается на 0 для нового файла.

## Замечания

- Скрипт stdlib-only, без внешних зависимостей.
- Штатная остановка ноды теперь идёт тем же локальным path: `POST /v1/shutdown` или Ctrl+C, с операторской RU-строкой в логах.
- Для parser fixtures и regression coverage см. `scripts/_test_cy_lab_seal_console.py`.