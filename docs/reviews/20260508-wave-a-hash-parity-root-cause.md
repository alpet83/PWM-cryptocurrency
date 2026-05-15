# Wave A — root cause: `tip_hash` / `last_epoch_hash` после стабилизации wire/compat

**Тикет:** `tasks/20260508-wave-a-hash-parity-followup.json`  
**Связанные артефакты:** `docs/reviews/20260508-peer-compat-wire-stabilization-testing.md`, `docs/reviews/20260508-v2-slice6-tip-hash-divergence-diagnosis.md`, `tasks/20260508-v2-slice6-hotfix-tip-hash-divergence.json`

---

## 1. Scope recap

- Цель Wave A: два same-shard узла с одинаковым генезисом и совместимым wire — **байтово идентичная** фиксация цепочки на диске, в т.ч. совпадение `tip_hash` в `epochs/pwm-epochs-manifest.json` и SHA256 последнего epoch-файла.
- После коммитов wire/u128 и guard маршрутизации (`docs/reviews/20260508-peer-compat-wire-stabilization-testing.md`) **wire_decode** и логи без `u128`/protocol_error — но gate по хэшам **по-прежнему FAIL** (`tip_hash_equal=false`, `last_epoch_hash_equal=false`).

---

## 2. Requirements fit (что «ломает» gate сейчас)

Harness читает:

- `tip_hash` из манифеста (`man1["tip_hash"]` vs `man2["tip_hash"]`), что согласовано с записью в `pwmd` при обновлении epoch (**hex от `hdr_hash` последнего блока**).
- `last_epoch_hash_*` — **SHA256 всего файла** последнего `block_e*.json` на ноде.

Оба показателя зависят от **точных байт заголовков блоков** в персистентных артефактах, а не только от согласованности высоты/части полей аккаунтов.

---

## 3. Детерминированный источник расхождения (primary root cause)

**Определение identity блока на tip:** `Chain::tip_hash()` возвращает `hdr_hash(&last.hdr)` (`pwm-core`).

**Что входит в `hdr_hash`:** BLAKE3 от bincode-сериализации **полного** `BlockHdr`, включая поля **`ts`** и **`sig`** (`crates/pwm-core/src/block.rs` — функция `hdr_hash`, структура `BlockHdr`).

**Откуда берётся `ts` при seal:** `Chain::seal` вызывает `next_apply_ctx()`, которое задаёт время как **`SystemTime::now()` в секундах UNIX** (`crates/pwm-core/src/chain.rs`). Это **независимый wall-clock на каждом узле**.

**Следствие:** два узла, каждый из которых **локально** вызывает `seal` (пустые блоки, соревнование пропоузеров, разный порядок/тайминг), получают при той же высоте и том же `state_root`/`tx_root` **разные** `ts` → другой preimage для подписи → другая **`sig`** → другой **`hdr_hash`** → другой **`tip_hash`** в манифесте и другой контент последнего epoch-файла → **`last_epoch_hash_equal=false`**.

**Вывод:** после устранения проблем wire расхождение Wave A — **не** следствие некорректного декода кадров, а **ожидаемый симптом недетерминированного заголовка** (wall-clock + производная подпись) при multi-node локальном seal. Это согласуется с прежним диагнозом slice6 (`docs/reviews/20260508-v2-slice6-tip-hash-divergence-diagnosis.md`).

**Вторичные/сопутствующие факторы** (могут усиливать расхождение, но не являются «новой» причиной после wire-fix):

- Разный порядок или набор локально заземлённых блоков до полной сходимости gossip (если политика не принуждает к одному канону байт-в-байт).
- Любое расхождение в том, **какой именно** сериализованный блок оказывается в конце epoch-файла (см. выше: тот же механизм `ts`/`sig`).

Исключено как первопричина **после** стабилизации peer wire: массовые `wire_decode` / `u128` ошибки (по отчёту тестирования — их нет в логах Wave A).

---

## 4. Style / architecture (кратко)

Проблема **не** в стиле кода, а в **семантике PoA + источнике времени**. Для отчёта pwm-review: правки прод-кода не выполнялись.

---

## 5. Safety

- Перевод `ts` на детерминированную схему в общем профиле затрагивает **экономику v2** (`season_ppm(ts)` и др.) — изменения должны быть **изолированы** (testnet/harness/флаг), чтобы не менять mainnet-семантику без явного RFC.

---

## 6. Tests

- Существующие **`tip_divergence`**, **`wire_decode`**, **`peer_session`** unit-интеграции **не заменяют** полный Wave A gate: они не гарантируют идентичность байт заголовков между двумя живыми `pwmd` с локальным seal.
- Регрессия: **`scripts/wave_a_same_shard_stop.py` exit 0** с `tip_hash_equal=true` и `last_epoch_hash_equal=true` после фикса.

---

## 7. Минимальная безопасная стратегия hotfix для MVP

**Приоритет A (узкий объём, предсказуемый эффект в harness/testnet):**

1. Ввести **детерминированный источник `ts` для seal** под явным профилем (env или поле `GenCfg` / cli dev-only): например `ts = genesis_anchor_secs + height` или монотонный **сетевой** clock, согласованный через существующий протокол, **без** `SystemTime::now()` на критическом пути формирования заголовка в этом режиме.
2. Документировать, что **`season_ppm` / v2 policy** в этом режиме либо отключаются, либо используют тот же детерминированный `ts` с осознанным компромиссом (только testnet).

**Приоритет B (шире по продукту, ближе к «один канонический блок на высоту»):**

- Только узел-пропоузер на высоте **H** производит блок; остальные применяют **ровно полученные байты** (replay), без повторного `seal` с новым `ts`. Требует чёткой политики выбора proposer и запрета локального seal для «не своей» высоты.

**Приоритет C (уже сделано как диагностика):** harness **FAIL** при `tip_hash` / `last_epoch_hash` mismatch (коммит `36823f3` в slice6) — сохранять, чтобы не маскировать проблему.

**Acceptance criteria (MVP hotfix slice):**

- Wave A (тот же сценарий, что в runbook): **`tip_hash_equal=true`**, **`last_epoch_hash_equal=true`**, существующие проверки высоты/чекпоинта/аккаунтов без регрессий.
- `cargo test -p pwmd` релевантные фильтры остаются зелёными.
- Нет регрессии wire/compat ( smoke из `peer-compat` отчёта по желанию повторить).

---

## 8. Логи и метрики, подтверждающие fix

- На обоих узлах при каждом seal (или в debug-режиме): **height, ts, hex(hdr_hash), prod_idx** — для одной и той же высоты после фикса значения **должны совпасть** между нодами.
- Сравнение **SHA256 последнего epoch-файла** (уже в отчёте Wave A) — must match.
- Опционально: однократный лог «источник времени» (`wall` vs `deterministic`) при старте ноды, чтобы оператор видел режим.
- Метрики (если есть экспорт): счётчик `seal_total` + гистограмма `block_ts` по shard — после фикса распределение `ts` на двух нодах должно совпадать на общих высотах.

---

## 9. Verdict

**REQUEST_CHANGES** (продуктовый/design): для «зелёного» Wave A нужен **либо** детерминированный `ts`/режим testnet, **либо** строгая модель «один sealer — остальные replay». Текущее поведение после wire-stabilization **согласуется с кодом** и **не указывает на оставшийся дефект wire-decode** как на первопричину расхождения хэшей.

---

## 10. Implementation follow-up (MVP hotfix, 2026-05-08)

- В `pwm-core` добавлен явный режим времени seal: `SealTimeMode::{WallClock, DeterministicHeight}`.
- По умолчанию сохранён `WallClock` (текущая прод-семантика без изменений).
- В test/dev-режиме `DeterministicHeight` используется формула `ts = 1_700_000_000 + height`; для одинакового chain context и height это даёт одинаковый `BlockHdr.ts`/`hdr_hash` на независимых нодах.
- В `pwmd` добавлен test/dev toggle:
  - CLI: `--debug-deterministic-seal-time`
  - ENV: `PWM_DEBUG_DETERMINISTIC_SEAL_TIME=1`
  - resolved config: `PwmdConfig.debug_det_seal_time` (default `false`)
- В startup лог добавлено предупреждение-caveat: deterministic mode искусственно фиксирует time-context и не должен использоваться как прод-поведение для season/fee анализа.
- Wave A harness (`scripts/wave_a_same_shard_stop.py`) теперь включает режим явно через `--debug-deterministic-seal-time` на обеих нодах.

---

## 11. Residual diagnosis after deterministic seal-time (`d048bbe`, pwm-testing FAIL)

**Контекст:** коммит `d048bbecd33b74da2373cc413f15a62e03f1bd74` включает `SealTimeMode::DeterministicHeight` и Wave A harness включает `--debug-deterministic-seal-time` на **обеих** нодах. Прогон (`docs/reviews/20260508-wave-a-hash-parity-testing.md`): `tip_hash_equal=false`, `last_epoch_hash_equal=false`, высоты и выборочные поля sender/receiver в снимке **совпадают**.

### 11.1 Что уже исключено

- **Wall-clock `ts` и производная `sig` от него** как единственный источник дрейфа — в этом режиме `next_apply_ctx()` задаёт `ts = 1_700_000_000 + height` (`crates/pwm-core/src/chain.rs`).
- **Wire decode / u128** — не первопричина текущего FAIL (отдельный отчёт по peer-compat).

### 11.2 Остаточная первопричина (two-node runtime)

**Независимый локальный seal на обоих пирах при живом transport.** У каждого `pwmd` в `spawn_seal_loop` каждые ~2 с вызывается `chain.seal(pool.take(64))` (`crates/pwmd/src/lifecycle.rs`) на **своей** копии мемпула и **своём** уже накопленном tip. Пира **не обязаны** получить один и тот же набор транзакций в одном и том же тике seal: задержки RPC vs gossip, разный порядок/состав батча до 64 tx, гонка «оба успели seal на высоте H до применения чужого блока» приводят к **разным** `tx_root` / `state_root` / `sig` при том же `height` и том же детерминированном `ts`. `hdr_hash` и байт последнего epoch-файла тогда закономерно расходятся.

Синк-protocol при `lag == 0` и разном `head_hash` возвращает `TipDivergence` (`sync_live::on_tip`) — это **фиксирует** расхождение, но **не выполняет reorg** к одной канонической цепочке; узлы остаются на **разных** байтовых кончиках при совпадении высоты.

**Почему harness всё ещё «зелёный» по аккаунтам:** проверяются только `balance_pwm` / `nonce` / `initialized` у sender/receiver. Полный `state_root` включает producer-награды, `fee_pool`, прочие счета — при разной упаковке блоков подмножество совпадает, **`tip_hash` не обязан совпасть**.

### 11.3 Минимальная дополнительная правка для прохождения Wave A

**Узкий тестовый/флаг-профиль (рекомендуемый MVP-шаг):** на **одном** из двух узлов (типично «follower») **не запускать** периодический локальный seal — только применение блоков по существующему P2P sync (`apply_blk` / catch-up). Второй узел остаётся единственным источником **локально** закрытых блоков; peer догоняет и получает **байтово тот же** канон. Для продакшена тот же принцип ближе к «один sealer на высоту / остальные replay», чем к двум независимым seal-loop.

Альтернативы шире (не минимум): реорганизация при `TipDivergence`, обязательный push блока после seal до следующего тика, жёсткая глобальная очередь tx — дороже по изменениям и рискам.

### 11.4 Чеклист для `pwm-coding`

1. Ввести явный **dev/test-only** флаг конфигурации (CLI + env по аналогии с `debug_det_seal_time`): например «запретить локальный seal-loop» или «режим follower: только apply из sync».
2. В `spawn_seal_loop` (или точке регистрации) при включённом флаге **не вызывать** `chain.seal` по таймеру; убедиться, что нода всё ещё поднимает transport и проходит **полный** catch-up до tip (существующий `sync_live`).
3. Обновить **`scripts/wave_a_same_shard_stop.py`:** включить флаг только на **node2** (или согласованно на всех кроме одного sealer).
4. Прогон Wave A: `tip_hash_equal=true`, `last_epoch_hash_equal=true`, прежние проверки высоты/чекпоинта/sender/receiver без регрессий.
5. Unit/интеграция: «follower без seal-loop» доходит до того же tip_hash, что leader, в одном шард-сценарии (минимальный тест рядом с `tip_divergence` или отдельный короткий сценарий).

### 11.5 Acceptance criteria (дополнение к §7)

- При двух нодах с одним genesis и включённым детерминированным seal-time: **хэши tip и последнего epoch-файла совпадают** между нодами после Wave A.
- Поведение **без** follower-режима и **без** флагов по умолчанию **не меняется** (два активных sealer по-прежнему допустимы вне harness).
- Документация runbook/harness: явно указано, что Wave A требует **ровно один** активный sealer среди пиров (через новый флаг).

### 11.6 Verdict (итерация после hotfix)

**REQUEST_CHANGES** — детерминированный `ts` необходим, но **недостаточен**; для байтовой паритета нужно убрать **двойной независимый seal** в сценарии Wave A (или ввести reorg/canonical выбор — шире по объёму).

---

## Participation / token estimate (§11 follow-up)

- `agent`: pwm-review  
- `result`: PASS (остаточный root-cause и план; правки только `docs/` + `tasks/`)  
- `artifacts`: дополнение `docs/reviews/20260508-wave-a-hash-parity-root-cause.md` §11  
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 9500, "confidence": "medium" }`
