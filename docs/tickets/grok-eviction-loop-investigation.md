---
ticket: grok-eviction-loop-investigation
priority: high
sprint: V7-S3
assignee: Grok
created: 2026-06-27
---

# Grok: Расследование eviction-шторма в lifecycle.rs

## Контекст

Бенчмарк `cy_cluster_transfer_ramp_soak.py` останавливается на level≈40 из-за
DDoS ноды. Кластер (proposer + attester, filesystem state) перестаёт производить
блоки — все токены sealer'а уходят на бесконечный цикл eviction.

## Уже установлено (не надо исследовать повторно)

**Root cause скрипта (уже исправлен):** `pick_senders()` всегда итерировал
`sender_accounts` с индекса 0 — одни и те же N сендеров в каждом блоке.
При повторном использовании сендера до подтверждения его предыдущей tx
в пул попадали две tx от одного сендера с одним nonce. Исправлено: добавлен
`sender_cursor` с round-robin ротацией.

## Проблема на стороне ноды (надо расследовать)

**Файл:** `crates/pwmd/src/lifecycle.rs`, строки ~1958-1993

После eviction одной tx с bad nonce, `g.pool.prepend_block(replay)` возвращает
оставшиеся N tx в пул и loop продолжается (`continue`). Из логов (2026-06-27):

```
[12:51:28.653] seal skip: evicting unapplicable tx at index 33 (bad nonce), requeueing 34 others
[12:51:29.279] seal skip: evicting unapplicable tx at index 30 (bad nonce), requeueing 63 others
[12:51:29.946] seal skip: evicting unapplicable tx at index 38 (bad nonce), requeueing 63 others
[12:51:30.597] seal skip: evicting unapplicable tx at index 38 (bad nonce), requeueing 63 others
... (ещё ~30 раз с интервалом ~650ms)
```

Пул раздулся до 64 tx (34 requeued + 40 новых из следующего батча скрипта),
а затем застрял: evict одну, requeue 63, evict другую, requeue 63...

**Почему пул не уменьшается:** когда sealer вызывает `seal()` с 64 tx, находит
bad-nonce tx, evicts её и возвращает 63 обратно через `prepend_block`.
Новые `validated_rx` tx тем временем тоже попадают в пул. В итоге pool ≈ 64
бесконечно.

**Цикл в логах — один и тот же tx_id появляется дважды:**
```
[12:51:27.594] accepted: queued via worker | tx_id=a747cf49... h=299406
[12:51:28.069] accepted: queued via worker | tx_id=a747cf49... h=299406
```
Это означает что та же tx прошла через воркеры дважды — или из `tx_ingress`
повторно, или есть другой путь re-submission.

## Задачи для Grok

### Задача 1: Найти источник дублей в eviction requeue

В `lifecycle.rs` строки 1977-1992:
```rust
let mut kept = Vec::with_capacity(txs.len().saturating_sub(1));
kept.extend(txs[..i].iter().cloned());
kept.extend(txs[i + 1..].iter().cloned());
// ...
g.pool.prepend_block(replay);
```

`txs` — это `Vec<SealEntry>`. Проверить: включает ли `SealEntry::Raw` tx,
которые после `prepend_block` снова идут через воркеры (→ двойная validated)?
Или только `SealEntry::PreValidated` — и тогда они минуют воркеры?

Если `prepend_block` переводит PreValidated → Raw, это объясняет re-validation
и двойное появление в логах.

### Задача 2: Почему `requeueing 63` повторяется ~30 раз без уменьшения

Ожидаемое поведение: каждый `seal skip` удаляет ровно одну плохую tx,
пул уменьшается. Фактически пул остаётся ~64.

Гипотеза: `validated_rx` дренируется в пул на каждой итерации цикла seal
(строки перед `pool.take(64)`). Когда eviction возвращает 63 в пул, следующий
цикл снова дренирует `validated_rx` — там могут быть те же tx повторно,
поступившие через воркеры после первого requeue.

Проверить: в `lifecycle.rs` перед `pool.take(64)` — сколько tx дренируется из
`validated_rx`? Есть ли дедупликация по tx_id в `pool`?

### Задача 3: Предложить fix для прекращения eviction cascade

**Вариант A (консервативный):** После `prepend_block(replay)` добавить
`tokio::time::sleep(Duration::from_millis(seal_interval_ms))`. Это даст
интервал для поступления новых аттестаций и предотвратит busy loop.

**Вариант B (правильный):** Если eviction нашёл bad-nonce tx, не возвращать
оставшиеся в пул а сразу попытаться seal с ними (одна итерация):
```rust
// Вместо prepend_block + continue:
if !replay.is_empty() {
    match g.chain.seal(replay.clone()) {
        Ok(h) => { /* success */ }
        Err((e, txs)) => { g.pool.prepend_block(txs); }
    }
}
```

**Вариант C (ограничение попыток):** Счётчик `eviction_retries`. После K
eviction без seal — сбросить пул (drop все pending) и логировать WARN.

Grok: рекомендуй наилучший вариант с учётом:
- Минимального изменения кода
- Отсутствия риска потери valid tx
- Совместимости с cluster gate (attester timeout)

### Задача 4: Анализ логов `logs/2026-06-27/`

Файлы: `pwmd-cy-proposer-123813.log`, `pwmd-cy-attester-123817.log`.

1. Найти точный момент, когда attester перестал аттестовать во время шторма.
   Лог показывает `seal_suppressed_by_cluster reason=quorum_pending` —
   когда это началось?

2. Через сколько блоков узел восстановился после шторма?

3. Определить: хватало ли воркеров (8 workers) или они тоже перегружались
   повторной валидацией requeued tx?

## Логи для анализа

```
logs/2026-06-27/pwmd-cy-proposer-123813.log
logs/2026-06-27/pwmd-cy-attester-123817.log
```

Ключевые временные метки:
- 12:51:20 — начало первого шторма (36 tx, level=36, height≈299402)
- 12:51:26 — второй эпизод (height 299405)
- 12:51:28 — третий и главный шторм (level=40, height 299406, pool=64)
- 12:51:50 — последний tx_included в видимом окне

Ключевой код: `crates/pwmd/src/lifecycle.rs` строки 1958–1993.

## Ожидаемый результат

- Объяснение механизма дублей (duplicate tx_id в accepted)
- Подтверждение/опровержение гипотез 1 и 2
- Рекомендованный fix (A/B/C) с кодом или псевдокодом
- Оценка: может ли шторм повториться при правильной ротации сендеров?
