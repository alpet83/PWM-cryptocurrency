# Slice 2: divergence dump + time-align seal

Дата: 2026-05-09  
Тикет: `tasks/20260509-protocol-versioning-debug-controls.json`

## Что внедрено

1. **Controlled divergence dump** (default OFF):
   - `--debug-dump-on-divergence` / `PWM_DEBUG_DUMP_ON_DIVERGENCE=1`
   - `--debug-dump-dir <DIR>` / `PWM_DEBUG_DUMP_DIR`
   - `--debug-dump-cap <N>` / `PWM_DEBUG_DUMP_CAP` (default `16`)
   - `--debug-dump-trigger-streak <N>` / `PWM_DEBUG_DUMP_TRIGGER_STREAK` (default `2`, минимум `2`)
2. **Time-align seal (mid-second)** (default OFF):
   - `--debug-align-seal-mid-second` / `PWM_DEBUG_ALIGN_SEAL_MID_SECOND=1`
3. **Precedence rule**:
   - если включены одновременно `debug-deterministic-seal-time` и `debug-align-seal-mid-second`, детерминированный режим имеет приоритет, а mid-second align игнорируется с явным warning.

## Trigger semantics (divergence dump)

Dump запускается не на первом разовом mismatch, а при **устойчивой дивергенции**:

- источник сигнала: существующий путь `SyncTipDivergence` в `transport/peer_session` (ветка обработки `SyncTipAnnounce`);
- condition: для конкретного `node_id` накоплен `div_streak >= trigger_streak` (по умолчанию `2`) последовательных divergence-событий;
- reset: streak сбрасывается в `0`, когда `SyncTipAnnounce` проходит без divergence;
- при срабатывании пишется локальный блок по высоте/хэшу divergence с `source = "divergence_probe"`.

## Файлы и формат dump

- Путь по умолчанию: `<data_file_parent>/blocks/b{height}.json`.
- Если `data_file` отсутствует: fallback `state/blocks/b{height}.json`.
- Кастомный путь: через `--debug-dump-dir`.
- Формат JSON содержит:
  - `height`
  - `hash`
  - `source`
  - `node_id`
  - `protocol_version`
  - `block` (canonical serde `pwm_core::block::Block`)

## Ограничение записи (bounded writes)

- Дампы выключены по умолчанию.
- Глобальный cap на процесс: `debug-dump-cap` (default `16`, минимум `1`).
- При достижении cap новые файлы не пишутся, в лог идёт предупреждение `reason=cap_reached`.

## Time-align details

- Mid-second align вставлен в `seal_loop` перед seal-операцией.
- Задержка вычисляется как bounded wait к отметке `~500ms` текущей/следующей секунды.
- Максимальный wait ограничен `750ms`; если расчёт выходит за предел, sleep пропускается.
- Режим предназначен только для debug/dev снижения wall-clock drift и не заменяет deterministic parity режим.

## Проверки

- `cargo test -p pwmd dump_path_uses_b_height`
- `cargo test -p pwmd div_dump_writes_block_file`
- `cargo test -p pwmd mid_wait_stays_bounded`
- `cargo test -p pwmd align_det_wins_over_mid`
- `cargo check -p pwmd`
