# Sprint V2-3 Slice 2 — независимое ревью (snapshot/replay closeout)

**Дата:** 2026-05-06  
**Тикет:** `tasks/20260506-v2-sprint3-emission-whales.json`  
**Артефакт closeout:** `docs/reviews/sprint-v2-3-slice2-snapshot-replay-closeout.md`  
**Кодовая база для сверки утверждений:** `7d9b6cb` (Slice 1 wiring), без новых изменений в `crates/**` в рамках Slice 2 (документальный closeout).

---

## 1. Scope recap

Slice 2 формально фиксирует **выравнивание post-tx начислений (marks + producer reward)** между `Chain::seal` и путями `pwmd`: полная валидация снапшота (`snapshot/io.rs`), ClickHouse replay (`snapshot/ch_http.rs`), offline repair (`snapshot/repair.rs`). Заявлены инварианты детерминизма, совместимости схемы genesis v4/v5, неизменность legacy policy и использование контекста блока (`height`, `ts`) для v2 с `season_ppm(ts)`. Матрица приёмки A1–A6 отдана `pwm-testing`; открытые риски помечены как не блокирующие закрытие.

---

## 2. Requirements fit

**Согласованность с кодом (spot-check).** В `chain.rs` после `apply_tx_with_ctx` используется `GenCfg::is_legacy_policy()` (эквивалентно `policy_ver == 1` при `LEGACY_POLICY_VER == 1`) с ветками `accrue_marks` / `reward_producer` против `accrue_marks_v2` / `reward_producer_v2` и `season_ppm` от `ts` в `Chain::seal`. В `io.rs` после реплея транзакций с `apply_tx_with_ctx(..., blk.hdr.height, blk.hdr.ts)` та же ветка legacy vs v2 и тот же `cfg.season_ppm(blk.hdr.ts)`. Аналогичная структура видна в `ch_http.rs` и `repair.rs`. Утверждение closeout о том, что отдельных «скрытых» формул в этих трёх путях относительно seal нет, **подтверждается**.

**Матрица A1–A6.** По записи делегирования `pwm-testing` (PASS): ядро, снапшоты (включая `snap_replay_uses_blk_ctx`, `repair_replay_uses_block_ctx`), genesis-тесты `genes_`, preflight, `fmt`, `check -p pwmd` — пройдены. **A4 (seal vs replay):** не как отдельный именованный интеграционный тест в closeout, но `snap_replay_uses_blk_ctx` строит блоки через `Chain::seal`, собирает `SnapshotData` и прогоняет `validate_snapshot`, где сравнивается `blk.hdr.state_root` с digest после реплея — это прямая проверка согласованности seal и JsonFile replay для данного сценария. **A5:** по заметке testing опирается на pwm-core тесты legacy/v2; отдельного CLI-smoke нет — приемлемо для заявленного скоупа, если оркестратор не требовал внешнего ранна.

**Зазоры:** в `verif` у testing упомянуто имя теста `snapshot_roundtrip_blocks_and_state` — в текущем дереве под `snapshot` оно не найдено; фактическое покрытие задаётся существующими тестами (в т.ч. `snap_replay_uses_blk_ctx`). Это вопрос **точности трейсабилити**, не функциональный блокер.

---

## 3. Style and module shape

Изменений production Rust в Slice 2 нет; критерии имён/модульности из `AGENT_PROMPT_coding.md` к самому слайсу **не применимы**. Текст closeout везде говорит `policy_ver == 1`, код использует `is_legacy_policy()` — семантически согласовано при текущем определении helper.

---

## 4. Safety

Новых доверенных границ или крипто-путей в Slice 2 нет. Операционные риски (старые снапшоты + другой `GenCfg`, дрейф при правках формулы в четырёх местах) в closeout перечислены корректно.

---

## 5. Tests

Покрытие заявлено делегированием `pwm-testing` PASS по A1–A6. Для формального «закрытия» документа **nit:** в `sprint-v2-3-slice2-snapshot-replay-closeout.md` чекбоксы секции 3 остаются пустыми (`[ ]`) при фактическом PASS — имеет смысл отметить выполненным в будущем doc-pass, чтобы артефакт не расходился с тикетом.

---

## 6. Verdict

**Approve with nits.**

Основные nits / follow-ups (не blockers):

1. Синхронизировать чеклист в closeout с фактическим PASS testing (галочки).
2. В тикете/testing-заметках уточнить ссылки на имена тестов для A4 (убрать или заменить несуществующее имя `snapshot_roundtrip_blocks_and_state`).
3. Уже зафиксированные в closeout: дублирование ветки policy в четырёх местах, будущая календарная семантика `season_ppm`, операционный риск v1 снапшотов — **корректно отнесены к отслеживаемым рискам**, а не к блокерам закрытия Slice 2.

Итог: **replay/snapshot compatibility для V2-3 policy в заявленном смысле покрыт** документацией и имеющимися тестами; закрытие Slice 2 как документального контура **приемлемо**.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-v2-3-slice2-review-20260506.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: low
```
