# Sprint V2-3 Slice 2: snapshot/replay compatibility closeout

**Дата:** 2026-05-06  
**Тикет:** `tasks/20260506-v2-sprint3-emission-whales.json`  
**База изменений:** Slice 1 runtime wiring (`7d9b6cb`) + review PASS (`docs/reviews/sprint-v2-3-slice1-review-20260506.md`)

## 1) Что именно зафиксировано по replay/snapshot paths

В рамках V2-3 policy в Slice 1 уже проведено выравнивание post-tx начислений (marks + reward) между `Chain::seal` и всеми критичными replay путями `pwmd`.

- **Ядро (`pwm-core`)**:
  - `crates/pwm-core/src/chain.rs`: в `Chain::seal` добавлено ветвление:
    - legacy (`policy_ver == 1`): `accrue_marks` + `reward_producer`;
    - policy v2 (`policy_ver > 1`): `accrue_marks_v2` + `reward_producer_v2` с `season_ppm(ts)`.
- **Snapshot validation / full replay**:
  - `crates/pwmd/src/snapshot/io.rs` (`validate_snapshot`): после `apply_tx_with_ctx(..., blk.hdr.ts)` применяется тот же policy branch и тот же `blk.hdr.ts` для `season_ppm`.
- **ClickHouse replay**:
  - `crates/pwmd/src/snapshot/ch_http.rs` (`replay_state_at`): зеркальная логика reward/marks после tx с тем же `blk.hdr.ts`.
- **Offline repair replay**:
  - `crates/pwmd/src/snapshot/repair.rs` (`replay_to`): зеркальная логика reward/marks после tx с тем же `blk.hdr.ts`.

Итог Slice 2: replay wiring для V2-3 policy не оставляет «скрытых» отдельных формул в `io/ch_http/repair` относительно `Chain::seal`.

## 2) Инварианты совместимости (must hold)

1. **Deterministic replay root**  
   Один и тот же `GenCfg` + один и тот же набор блоков (`hdr + txs`) => один и тот же `state_root` при replay.
2. **Schema loader continuity (v4/v5)**  
   Загрузчик genesis в `pwmd` принимает schema v4 и v5; v5 не ломает обратную совместимость по чтению.
3. **Legacy policy unchanged**  
   При `policy_ver == 1` поведение legacy-начислений (без stake gates и без v2 scaling) сохраняется.
4. **Policy v2 uses deterministic block context**  
   Для replay path используется контекст блока (`blk.hdr.height`, `blk.hdr.ts`) через `apply_tx_with_ctx`, а v2 marks/reward масштабируются через `season_ppm` на этом же `blk.hdr.ts`.

## 3) Acceptance / test matrix для pwm-testing

- [x] **A1. Core deterministic parity:** `cargo test -p pwm-core` PASS; кейсы `legacy_keeps_reward_path`, `policy_v2_gates_with_season`, stake-gate/season tests в `state.rs` не регрессируют.
- [x] **A2. Snapshot replay parity (JsonFile):** `cargo test -p pwmd snapshot` PASS; включены `snap_replay_uses_blk_ctx` и `repair_replay_uses_block_ctx`.
- [x] **A3. Genesis loader continuity:** `cargo test -p pwmd genes_` PASS; подтверждается прием schema v4/v5 без регресса старого формата.
- [x] **A4. Seal vs replay alignment check:** сценарии `snap_replay_uses_blk_ctx` / `repair_replay_uses_block_ctx` проходят replay через `validate_snapshot` с проверкой согласованности `state_root` относительно блоков, собранных через `Chain::seal`.
- [x] **A5. Legacy guardrail:** подтверждено целевыми `pwm-core` кейсами (`legacy_keeps_reward_path` и связанные seal/reward детерминизм проверки) без отдельного CLI-smoke.
- [x] **A6. Tooling sanity:** `cargo fmt --check` PASS, `cargo check -p pwmd` PASS.

## 4) Open risks (не блокируют closeout, но должны быть отслежены)

1. **Future calendar season semantics**  
   Сейчас `season_ppm` параметризован `block_ts`, но фактическая формула в текущем шаге конфиг-центрична; календарная логика (месяц/сезон) потребует отдельного RFC/слайса.
2. **Duplicated policy branch in 4 местах**  
   Схожий блок post-tx reward/marks находится в `Chain::seal`, `snapshot/io.rs`, `snapshot/ch_http.rs`, `snapshot/repair.rs`; риск drift при будущих правках формулы.
3. **Old snapshots + different GenCfg**  
   Старые snapshot-данные, проигрываемые с отличающимся `GenCfg`, по контракту должны детектить mismatch (state root / genesis rows), но это остается операционным риском при неправильном rollout.

## 5) Closeout verdict

Slice 2 (`snapshot/replay compatibility closeout`) считается зафиксированным документально: replay wiring для V2-3 policy согласован с `Chain::seal`, инварианты сформулированы, acceptance matrix для `pwm-testing` задана, открытые риски явно перечислены.
