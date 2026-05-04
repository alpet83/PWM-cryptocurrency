# Review: underfunded TRANSFER in mempool → infinite `seal skip` (2026-05-04)

Статический разбор (pwm-review). Продуктовые правки не включены в этот файл.

## Суть

- `validate_tx_shape` **не** проверяет баланс; `Mpool::push` только лимит по размеру.
- Недостаточно средств выявляется в `Chain::seal` → `apply_tx` → при ошибке весь батч возвращается через `prepend_block` → каждые ~2 с тот же WARN.

## Рекомендации (приоритет)

1. **Admission в `handlers_tx::v1_tx`:** перед `pool.push` — dry-run `apply_tx` на клоне tip-state; при `Insufficient` / жёстких ошибках — **409/400**, не класть в пул.
2. Опционально: **`State::precheck_tip`** в `pwm-core`.
3. **Defense in depth в `spawn_seal_loop`:** при ошибке seal отбрасывать первый «плохой» tx из батча, а не prepend всего вектора слепо.
4. UX: TUI/CLI preflight баланса (не заменяет сервер).
5. Тесты: HTTP underfunded → не 204; после исправления — нет бесконечных warn.

## Вердикт

**Request changes** на стороне `pwmd`/`pwm-core` (п.1 и желательно п.3).
