# S2 lease/fencing failover: final review (pwm-review)

Дата: 2026-05-09  
Тикет: `tasks/20260509-single-sealer-failover-profiles.json`  
Оцениваемые коммиты: `2e597d7` (реализация), `cfda7af` (трассируемость тикета после кодирования), `ef9f57f` (pwm-testing отчёт и артефакты тикета)

## 1. Scope recap

Срез **S2** добавляет process-local lease/fencing gate для `single_sealer`: ключ аренды — `validator_identity_hash`, владелец — `node_instance_id`, TTL и окно takeover настраиваются, перенос повышает `term` и `fence`, есть защита от takeover на отстающем tip (`max_tip_lag`). Seal-loop вызывает `run_lease_gate` до `chain.seal`. Сигналы аренды проброшены в `NodeHelloCapabilities`, `PeerWireMsg::Heartbeat` и `/v1/status`. Документация: отчёт кодирования, дополнения runbook. Связь с **docs/MVP-checklist.md** §4 / §12: реализован **локальный** предохранитель от параллельного seal для той же validator identity **в рамках одного процесса pwmd** и описанных таймингов; распределённый консенсус (S3) не затрагивается.

## 2. Requirements fit

**Корректность state machine (в пределах MVP):** `step_lease` в `lease.rs` согласован с заявленной семантикой: первичный acquire при пустой записи; renew того же `owner_id`; удержание peer-ом до `expires_at_ms`; фаза ожидания takeover до `expires + takeover_ms`; отказ takeover при «stale tip»; успешный takeover инкрементирует `term` и `fence`. Тесты `lease_renew_ok_same_owner`, `lease_takeover_after_timeout`, `old_active_blocked_without_lease` покрывают базовую траекторию A→B и блокировку старого active после потери аренды — **достаточно для unit-уровня координатора**.

**Split-brain для пары узлов с тем же ключом (адекватность):** карта аренды — **статический in-memory `HashMap` на процесс** (`LEASE_MAP`). Два независимых процесса `pwmd` на разных хостах (или на одном) **не разделяют** это состояние: каждый может выполнить локальный acquire для одного и того же `validator_identity_hash` и одновременно считать себя активным с точки зрения локального gate, пока S1 handshake не отсечёт active/active по ролям. **Итог по цели ревью «адекватность против split-brain same-key»:** **частичная** — защита от «двойного seal» работает, когда координация сводится к **одному** рантайму или к сценариям, где операторами обеспечена эксклюзивность процесса; для настоящей HA без внешнего источника истины (файловый lock, KV, сервис аренды) **недостаточно**. Это явно признано в отчёте кодировки и runbook — **ожидание по продукту согласовано с документацией**, но не с «строгой» интерпретацией same-key HA.

**Handshake / heartbeat / status:** исходящий `NodeHello` и периодический heartbeat читают снимок `lease_runtime` (mutex). `run_lease_gate` обновляет тот же `lease_runtime` в seal-loop (интервал 2s). **Согласованность:** в общем случае hello/heartbeat отражают последнее состояние после последнего `step_lease` на данном узле, а не «живую» реконструкцию по локальным часам между тиками — для наблюдаемости приемлемо. **Нит:** при `debug_disable_seal_loop` gate не вызывается — в статусе остаётся `lease_not_acquired` / `seal_gate_allowed=false`, но в hello по-прежнему сериализуются поля аренды из начальной структуры (`owner_id` локального инстанса, нулевые сроки до первого acquire); это может путать оператора, сравнивающего hello и `/v1/status`, хотя gate и так запрещает seal.

**Обратная совместимость wire:** новые поля в capabilities и heartbeat — опциональные с `serde(default)`; существующие пиры без полей продолжают декодировать кадры. **`PWM_PROTOCOL_VERSION` не поднимали** (`0.1.0`) — приемлемо при позиции «minor optional fields, mixed-version decoding»; явной строки «no bump» в отчёте кодирования мало, но риск ниже, чем у обязательных полей.

**Операционные риски:** чувствительность к **синхронизации часов** между standby и ex-active (takeover окно по wall-clock); чувствительность к выбору TTL/takeover при сетевых задержках и длительности seal-tick; удалённые lease-поля **не** используются как источник истины для локального gate (только локальный `step_lease`) — это снижает сложность и атаки на ложный lease с wire, но оставляет ответственность на **раздельных процессах**.

## 3. Style and module shape

- **Имена:** `python scripts/check_rust_fn_name_segments.py` по путям `lease.rs`, `lifecycle.rs`, `handshake.rs`, `handlers_status.rs`, `transport/dial.rs`, `transport/peer_session/mod.rs`, `transport/peer_session/wire.rs` — **violations пусто**.
- Новая подсистема вынесена в `lease.rs` с кратким `//!` баннером; seal-loop интеграция локализована — ок для среза.
- Многократные подряд `lease_runtime.lock()` в `build_local_node_hello` — стиль/эффективность нит, не безопасность.

## 4. Safety

- **Граница доверия:** решение о разрешении seal основывается на **локальном** store, не на подписанных lease-сообщениях от пира — уменьшает поверхность «ложной аренды» по сети, но **не** даёт криптографического fencing между процессами.
- **Паники / poisoning:** при poison mutex lease map/runtime — переход в `FencedStandby` / подавление seal и логирование; ок.
- **DoS:** отдельных новых векторов по сравнению с существующим seal-loop не выделено; глобальный mutex на карту аренды — узкое место при гипотетически очень большом числе разных validator keys в одном процессе (для MVP нерелевантно).

## 5. Tests

- Покрыты: чистая логика `step_lease` (три теста), прогон `pwm-testing` с транспортными smoke-тестами heartbeat, `status_exposes_identity_signals` (identity без assert lease-полей).
- **Пробелы:** нет интеграционного теста «два app в одном процессе / две задачи» на race; нет asserts JSON status для счётчиков lease после takeover (отмечено в testing report как некритично для «проводка компилируется»).

## 6. Verdict

**approve with nits** — для заявленного **MVP process-local** среза блокирующих дефектов нет; ключевые ограничения (нет межпроцессного fencing) **документированы**. Ниты: наблюдаемость hello при отключённом seal-loop; отсутствие интеграционных тестов на конкурирующие инстансы в одном бинарнике; операторская зависимость от clock/TTL.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PARTIAL
artifacts: docs/reviews/20260509-s2-lease-fencing-failover-final-review.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 7500, "confidence": "low" }
```

## 8. Merge readiness (итоговые флаги S2)

- **PASS / PARTIAL / FAIL:** **PARTIAL** — логика gate и тесты соответствуют **документированному** scope; «полная» адекватность против split-brain для **двух независимых процессов** с тем же ключом **не достигнута** (и не заявлена как достигнутая в продуктовом отчёте).
- **Готовность к merge:** **да, с операционными ограничениями** — merge уместен, если релизная позиция фиксирует «single-process / операторская эксклюзивность / следующий шаг — внешний lease backend»; не позиционировать как готовую кросс-процессную HA без доп. мер.

---

**Вердикт одной строкой для оркестратора:** `PARTIAL — merge готов при явных MVP-ограничениях process-local lease; для cross-process same-key HA нужен внешний координатор.`
