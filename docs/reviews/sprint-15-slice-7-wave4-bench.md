# Sprint 15 — Slice 7 Wave 4: отчёт по бенчмаркам `snapshot_load`

Источник методики и переменных окружения: `docs/reviews/sprint-15-slice-6-bench.md`. Ниже — сопоставление сценариев Wave 4 checklist (JsonFile vs CH mock vs live CH; разложение «холодного» пути на части).

## Команды

| Сценарий | Команда |
|----------|---------|
| JsonFile | `cargo bench -p pwmd --bench snapshot_load` |
| CH (mock или live) | `cargo bench -p pwmd --bench snapshot_load --features clickhouse-snapshot` |
| Живой ClickHouse | задать `PWM_CLICKHOUSE_BENCH_URL=http://127.0.0.1:8123` перед командой с feature |

## Функции Criterion и смысл для Wave 4

| Функция | Интерпретация Wave 4 |
|---------|------------------------|
| `snap_load_jsonfile` | Полный путь загрузки файла (I/O + decode + `validate_snapshot` / replay). |
| `snap_decode_trust_state` | Нижняя граница «без replay» (только decode + wire); небезопасный контракт. |
| `snap_validate_full_replay` | Изолированный полный replay на уже декодированном снимке — прокси для стоимости «full replay baseline» без повторного чтения файла. |
| `snap_load_clickhouse` | HTTP `ch_load` + decode + validate; при mock — без Docker; при live — после `import_snapshot_file`. |

**Cold start checkpoint + tail vs full replay:** отдельного режима загрузки «только checkpoint + хвост» в коде пока нет (см. slice-6 bench § чекпоинт); разложение `snap_decode_trust_state` + `snap_validate_full_replay` даёт оценку вклада replay относительно чистого decode. Когда появится bootstrap по checkpoint, добавить отдельный bench и строку в таблицу ниже.

## Таблица результатов (заполнить на машине разработчика)

Запустить команды выше, при необходимости положить `./tmp/state-testnet/pwm-data.json` и `./tmp/genesis-custom.json` после `node-1.ps1`. Скопировать из вывода Criterion среднее время (µs или ms).

| Сценарий | JsonFile / mock CH / live CH | Среднее время | Примечание |
|----------|------------------------------|---------------|------------|
| Full load | JsonFile (`snap_load_jsonfile`) | | |
| Decode only | (`snap_decode_trust_state`) | | |
| Replay only | (`snap_validate_full_replay`) | | |
| CH load | mock (`snap_load_clickhouse`) | | |
| CH load | live (`PWM_CLICKHOUSE_BENCH_URL`) | | или «н/д» если CH недоступен |

## Write pressure (multi-node)

Рантаймовый профиль `INSERT` на блок / checkpoint в CH зависит от задержки сети и размера payload; сценарий multi-node не входит в одиночный bench `snapshot_load`. Для замеров частей/задержек на стороне ClickHouse использовать системные метрики кластера или отдельный soak-тест (pwm-testing).
