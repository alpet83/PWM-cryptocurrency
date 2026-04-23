# Ревью: SealAbort и возврат tx в мемпул (2026-04-19)

## 1. Scope recap

Заявленный набор правок закрывает пункты **MVP-checklist §3** (тесты сценария **SealAbort + prepend_block**) и **§4** (при ошибке `seal` не терять изъятые tx: `SealAbort`, `prepend_block` в `pwmd`). В коде: публичный тип `SealAbort`, `Chain::seal` → `Result<(), SealAbort>`, `Mpool::prepend_block`, цикл в `pwmd`, юнит-тесты в `pwm-core`.

## 2. Requirements fit

Цель «не терять транзакции при неуспешном seal» достигнута: при любом `Err` из `seal` возвращается исходный `Vec<SignedTx>`, нода снова кладёт их в пул через `prepend_block`. Порядок блока восстанавливается за счёт обхода `rev` + `push_front`.

Пробелы вне этого диффа: ранняя валидация `POST /v1/tx`, лимит тела JSON, CORS, персист — по-прежнему открытые строки чеклиста §4.

## 3. Style

Короткие имена (`SealAbort`, `prepend_block`), комментарии на английском в затронутых местах; `lib.rs` реэкспортирует `SealAbort`.

## 4. Safety

- При ошибке `apply_tx` откат за счёт клонирования `st` до коммита в `self` — корректно.
- `prepend_block` не проверяет `cap`; при текущем вызове из `pwmd` после `take` длина восстанавливается, переполнения нет. **Нит:** при будущих вызовах из других мест стоит документировать инвариант или проверять cap.
- Одна и та же «плохая» tx снова попадает в блок и снова фейлит seal — ожидаемо для devnet без отбраковки.

## 5. Tests

- `seal_returns_txs_on_apply_error` — возврат tx, сообщение, высота не растёт.
- `prepend_block_restores_fifo_order` — порядок после take + prepend.
- `seal_fail_then_prepend_keeps_len` — seal fail + возврат в пул.

Не покрыто: интеграционный HTTP smoke для `pwmd` (отдельный пункт чеклиста).

## 6. Verdict

**Approve with nits** (см. §4 про контракт `prepend_block` и cap).
