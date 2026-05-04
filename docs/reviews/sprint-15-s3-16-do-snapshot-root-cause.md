# S15-S3.16 — DO: `state_root` mismatch при загрузке снапшота (pwm-review)

**Связь:** `docs/reviews/sprint-15-s3-16-cycle2-relay-journal-review.md` (межшард, `ready_degraded` на DO).

## 1. Симптом

При старте ноды (сценарий DO: `node-2.ps1`, `state-testnet2/pwm-data.json`):

- `snapshot load failed (fallback to genesis state): snapshot chain mismatch: block[16] state_root does not match replayed state`
- затем `ready_degraded`, в памяти остаётся genesis/bootstrap до загрузки, снапшот **не применяется**.

**Индекс блока:** в `validate_snapshot` цикл `enumerate()` даёт **нулевой индекс** в массиве `blocks`. `block[16]` — это **17-й** блок, ожидаемая высота **17**. Число 16 само по себе не «особое»; это **первый блок**, на котором `digest(replay_state)` после переигрывания цепочки не совпал с `hdr.state_root` в файле.

## 2. Код-путь (загрузка → проверка → фаза)

**Старт:** `lifecycle::spawn_snapshot_loader` поднимает задачу: фаза `loading_snapshot`, затем `load_snapshot(path, &cfg)` с `cfg` из уже загруженного genesis текущего процесса.

**Загрузка и валидация:** `snapshot::load_snapshot` парсит JSON, собирает canonical `SnapshotData`, вызывает **`validate_snapshot(&snap, cfg)`**. Только при `Ok` снапшот считается допустимым; далее `into_runtime` лишь перекладывает поля в runtime-структуры.

**Проверка цепочки (суть симптома):** в `validate_snapshot` заводится `replay_state = cfg.state0()`. Для каждого блока по порядку: проверки высоты, `prev_hash`, `tx_root`, `prod_idx`, подписи; затем для каждой tx — `replay_state.apply_tx(tx)`; после блока — `accrue_marks(cfg.marks_coeff)`, `reward_producer(..., cfg.block_reward)`, и сравнение **`blk.hdr.state_root` с `digest(&replay_state)`**. Несовпадение даёт ровно сообщение про `state_root does not match replayed state`.

Это тот же порядок эффектов, что и при майнинге/seal в `pwm_core::chain::Chain::seal` (apply → marks → reward → `state_root`).

**Фаза после ошибки:** при ошибке `load_snapshot` или `into_runtime` в `spawn_snapshot_loader` выставляется `InitState::ready_degraded` с текстом ошибки (`lifecycle.rs`). В `state.rs` у `ReadyDegraded` **`is_ready()` всё ещё true**, то есть HTTP не обязан отрезаться только из‑за фазы; поведение межшард/E2E смешивается с «битым» persisted state (см. cycle2 review).

**Сохранение (контраст):** `save_snapshot` атомарно пишет tmp и `rename`; в один снимок попадают `blocks` и `state` из одного `Inner`. При штатном пути после export/import в `v1_tx` сначала `seal`, затем сохранение под тем же write-lock; при ошибке save делается rollback in-memory (`api.rs`).

## 3. Гипотезы: ожидаемое vs вероятный баг

### 3.1 Ожидаемое / операционное (не обязательно дефект сериализации)

1. **Расхождение genesis при чтении снапшота.** Replay всегда стартует с **`cfg.state0()` текущего `genesis.json`**, а `validate_snapshot` сравнивает с снапшотом только **`genesis_accounts` как набор (acct, pubkey, der_idx)** без начальных балансов и **без** жёсткой привязки `block_reward`, `marks_coeff`, политики наград и т.д. Если на DO подставили тот же набор аккаунтов/ключей, но **изменили балансы genesis**, **награду за блок** или **коэффициент марок**, начальное состояние и последующие шаги replay **не совпадут** с тем, как цепочка строилась при записи снапшота → типичный `state_root` mismatch на раннем или среднем блоке (в т.ч. «блок 16»).

2. **Другая версия `pwm-core` / логики `apply_tx`.** Если бинарник при загрузке меняет семантику состояния относительно того, что писал заголовки при seal, первое расхождение даст тот же класс ошибок. Исторически в репозитории уже фиксировали связанный класс проблем (см. remediation2, блокер C: self-transfer / import provenance).

3. **Повреждение или ручное редактирование `pwm-data.json`.** Любая порча tx, заголовка или несогласованность `blocks` vs `state` может пройти часть проверок и упереться в `state_root` (или раньше — в `tx_root` / подпись).

4. **«Чужой» снапшот / смешение артефактов.** Копирование файла от другой среды без идентичного genesis (в широком смысле — включая параметры, влияющие на `state0` и reward path) даёт тот же эффект.

5. **Гонка autosnapshot.** В `lifecycle` autosnapshot делается в seal-loop с write-lock на `inner`; запись атомарна. **Два процесса `pwmd`, пишущих один и тот же файл**, теоретически дают «последний rename выиграл» — файл останется внутренне согласованным для одного из процессов, но это скорее сценарий **потери блоков**, а не характерный «ровный» mismatch на фиксированной высоте без смены кода/genesis. Низкий приоритет без доказательства двойного writer.

### 3.2 Возможный баг в продукте (если genesis и версия бинарника идентичны записи)

1. **Неполный контракт снапшота:** отсутствие в persisted формате **дайджеста genesis/chain params**, из-за чего «тихо» принимается снапшот при **дрейфе** параметров, не отражённом в `genesis_accounts`. Это граница между **ожидаемым** (оператор ошибся конфигом) и **дефектом продукта** (слабая валидация).

2. **Расхождение между тем, что делает runtime при seal, и тем, что делает чистый replay из JSON** (дополнительные поля `State`, порядок ключей в `digest`, скрытые поля) — потребовалось бы сравнить `digest` и сериализацию `State`; в документации `pwmd.md` контракт явно описывает self-verification replay; при идентичном коде путь seal и validate должен совпадать.

На основании только кода **наиболее проверяемая** гипотеза для devnet с двумя нодами: **DO читает `pwm-data.json`, собранный при одном наборе chain/genesis параметров, а стартует с другим `genesis.json` (или другой сборкой), при том что строки `genesis_accounts` совпали.**

## 4. Рекомендации для pwm-coding

1. **Диагностика на месте:** при `state_root` mismatch логировать (или отдавать в `/v1/status`) **дайджест `state0()`** / хэш genesis payload и версию `SNAPSHOT_VERSION`, чтобы сразу отличить дрейф конфига от порчи файла.

2. **Ужесточение контракта снапшота:** сохранять в JSON **хэш или каноническое представление** параметров, влияющих на replay (`block_reward`, `marks_coeff`, начальные балансы или целый `digest(GenCfg)`), и отклонять загрузку с понятным сообщением, а не только на шаге replay.

3. **E2E / межшард:** для чистого прогона при уже известном `ready_degraded` — отдельный прогон с **удалением `pwm-data.json` на DO** или с гарантированно тем же genesis, что при записи; иначе нельзя отделить сбой релея от невалидного persisted state (как в cycle2 review).

4. **Регресс-тест:** снапшот, записанный при конфиге A, не должен проходить валидацию при конфиге B с тем же `genesis_accounts`, но другими `block_reward` / начальными балансами (если продукт это хочет запретить).

5. **Документация для оператора:** явно указать, что **`genesis_accounts` в файле не задаёт полный genesis** для replay.

---

## 5. Краткий вердикт для оркестратора

**Verdict:** анализ цепочки загрузки и причин класса `state_root` mismatch зафиксирован; высокий приоритет проверки **идентичности genesis (включая параметры вне `genesis_accounts`) и версии бинарника** между записью и загрузкой DO; усиление контракта снапшота снимает класс «тихих» расхождений.

```yaml
participation:
  agent: pwm-review
  result: PASS
  artifacts: docs/reviews/sprint-15-s3-16-do-snapshot-root-cause.md
  note: "Код-путь и гипотезы выверены по snapshot.rs/lifecycle.rs/chain.rs/state.rs; индекс block[16] интерпретирован как height 17. Оригинальный субагент не записал файл (Ask mode); оркестратор сохранил тело ответа."
  token_usage:
    source: estimate
    input: null
    output: null
    total: 7500
    confidence: low
```

**Однострочный verdict для цитирования:** `state_root mismatch на block[i] — первый блок, где replay от текущего cfg.state0() расходится с заголовком; типичные причины: дрейф genesis-параметров при совпадении genesis_accounts, смена логики apply_tx между сборками, порча JSON; гонка autosnapshot маловероятна; для pwm-coding — bind chain params в снапшот и диагностический digest при load.`
