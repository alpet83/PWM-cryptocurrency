# Review: Sprint V2-3 Slice 1 — policy-gated emission runtime

**Дата:** 2026-05-06  
**Тикет:** `tasks/20260506-v2-sprint3-emission-whales.json`  
**Скоуп кодовый:** `7d9b6cb` (`feat(v2-3): add policy-gated emission runtime`)  
**Freeze-якорь:** `docs/reviews/sprint-v2-3-slice0-design-freeze.md` (контракт Slice 1)  
**Статус pwm-testing:** PASS (post-`7d9b6cb` gate из тикета)

## 1. Scope recap

Slice 1 по фризу: ветвление по `policy_ver`, при `policy_ver == 1` полное сохранение legacy-пути наград и marks; при нелегаси-ветке — пороги `pwm_stake_min` / `marks_stake_min` и сезонный множитель через `season_enabled` + `season_coeff_ppm` на детерминированном пути; синхронизация той же семантики во всех путях replay/validation в `pwmd`, сохранение replay-детерминизма относительно сохранённых блоков.

Проверялись: `crates/pwm-core/src/{genesis.rs,state.rs,chain.rs}`, `crates/pwmd/src/snapshot/{io.rs,ch_http.rs,repair.rs,mod.rs,store.rs}`, `crates/pwmd/src/lib.rs`.

## 2. Requirements fit

**В целом соответствие контракту Slice 1 — хорошее.**

- **`policy_ver`:** `GenCfg::is_legacy_policy()` эквивалентно `policy_ver == LEGACY_POLICY_VER` (1); legacy-ветка вызывает прежние `accrue_marks` / `reward_producer` без порогов и без ppm-масштабирования награды и marks.
- **Пороги:** `accrue_marks_v2` пропускает аккаунты с `staked < marks_stake_min`; `reward_producer_v2` не начисляет PWM продюсеру при `staked < pwm_stake_min`. Это совпадает с заявленной семантикой «киты» через минимальный стейк.
- **Сезонность (ppm):** при `season_enabled` множитель берётся из `season_coeff_ppm`, иначе — `DEF_SEASON_COEFF_PPM` (1e6). Значение ppm **сейчас не зависит от `block_ts`**: в `GenCfg::season_ppm` параметр заголовка принят в сигнатуру и подавлен через `let _ = block_ts`, фактически используется только конфиг. Зато **транзакции и claim-матерность** при реплее идут с `blk.hdr.ts` через `apply_tx_with_ctx`, что соответствует freeze про опору на timestamp заголовка на шаге состояния. Календарная сезонность «от времени суток/даты» в формуле ppm **не реализована** — узкий первый шаг; для полного буквального «ppm от `header.ts`» нужен последующий спек/слайс.
- **`pwmd` replay parity:** идентичное ветвление legacy vs v2 в `snapshot/io.rs` (сверка `state_root` при полном реплее), `snapshot/repair.rs` (`replay_to`), и `snapshot/ch_http.rs` (`replay_state_at` под `clickhouse-snapshot`). Это снижает риск дрейфа «узел уплотнил по одной формуле — снимок валидируется по другой».

**Нюансы:**

- Дублирование одного и того же блока «после txs: marks+reward» в четырёх местах (`Chain::seal` + три пути `pwmd`) — риск рассинхрона при следующей правке формулы (сопровождение, не функциональный дефект в текущем коммите).
- **Порядок при ошибках:** проверка наличия счёта продюсера в `Chain::seal` выполняется до ветки policy; при нарушенном genesis-инварианте marks/reward не выполняются — для корректного genesis недостижимо.

## 3. Style and module shape

- Прогон `python scripts/check_rust_fn_name_segments.py` по путям из `7d9b6cb` — **нарушений нет** (prod ≤ 4 сегментов, тесты отдельно не регрессировали в этом чек-листе).
- В рамках **только** `7d9b6cb` в `pwmd` заметны согласованные переименования без смены семантики: публичный реэкспорт `snap_ch_db_net` вместо `snap_ch_db_from_net_id` в `lib.rs`; в `store.rs` — `ch_save_seal_fallback` / `ch_save_tip_fallback` и вызов `io::save_epochs_sum_tip` вместо прежнего имени summary-хелпера; в связке с `io.rs` это уменьшает шанс расхождения fallback-пути ClickHouse ↔ JsonFile по имени API.

## 4. Safety

- Новые пути используют `saturating_*` и деление на `PPM_DENOM` — переполнения и panics по этой арифметике маловероятны.
- Явной новой криптографии в диффе нет.
- **`Chain::seal`** по-прежнему берёт время следующего блока из `SystemTime::now()` в `next_apply_ctx` — это путь **живого** уплотнения, не реплея. Freeze запрещает wall-clock как вход в **консенсусный расчёт там, где он может отличаться от реплея**; для новой ppm-ветки самой формулы начисления часы ОС не читаются — реплей использует сохранённый `hdr.ts`.

## 5. Tests

- По тикету зафиксирован **PASS**: `cargo test -p pwm-core`, `cargo test -p pwmd snapshot` (включая сценарии контекста блока для реплея), `genes_`, `cargo fmt`, bench `--no-run`.
- В `pwm-core`: `legacy_keeps_reward_path`, `policy_v2_gates_with_season` в `chain.rs`; в `state.rs` — изолированные тесты порогов и ppm для marks/PWM.
- **Пробел (низкий приоритет):** отдельная фикстура genesis/snapshot с `policy_ver > 1` end-to-end в `pwmd` усилила бы регрессию проводки конфига; не обязательна для закрытия Slice 1 при имеющемся покрытии ядра и snapshot-replay тестов.

## 6. Verdict

**Approve with nits**

Приоритеты:

1. (Низкий) Когда понадобится календарная сезонность, использовать `block_ts` внутри `season_ppm` или убрать параметр до появления реальной зависимости — сейчас сигнатура слегка вводит в заблуждение.
2. (Низкий) При росте формулы — вынести общий «шаг награды после применения txs» в один хелпер, разделяемый `pwm-core` и `pwmd`, чтобы не плодить четыре копии ветвления.

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS
artifacts: docs/reviews/sprint-v2-3-slice1-review-20260506.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 15000, "confidence": "low" }
```

---

**Краткий вердикт для оркестратора:** **PASS (approve with nits)** — `7d9b6cb` выполняет контракт Slice 1 по ветвлению, порогам и ppm из конфига; реплей в `pwmd` согласован с ядром по `hdr.ts` для применения txs; расчёт ppm пока не зависит от timestamp заголовка; остаётся нит про сигнатуру `season_ppm` и дублирование ветвления.
