# Slice 2 final review — divergence dump + time-align seal

**Дата:** 2026-05-09  
**Роль:** `pwm-review` (независимый гейт)  
**Тикет:** `tasks/20260509-protocol-versioning-debug-controls.json`  
**Диапазон:** `ad0bee1` (coding), `e3e0e24` (тикет), `e2c1701` (testing-артефакты и обновление тикета)

## 1. Scope recap

Замыкание Slice 2 по тикету и design-gate: **контролируемые дампы локального блока при устойчивой sync-tip divergence**, **глобальный cap записей**, **разрешение базового каталога** (`data_file` parent / явный `--debug-dump-dir` / fallback `state/blocks`), **опциональное выравнивание локального seal к середине секунды**, с **явным приоритетом deterministic seal-time**. Документирование: `docs/reviews/20260509-slice2-dump-timealign.md`, runbook-секция по same-shard sync.

## 2. Requirements fit

| Цель | Оценка |
|------|--------|
| Триггер не на первый флэп, а на «persistent» divergence | Да: для каждого `node_id` в `HandshakeState.sync_live.peers` ведётся `div_streak`; инкремент на `Ok(Some(div))`, сброс в `0` на `Ok(None)` после успешного `on_tip`. Дамп только при `div_streak >= trigger_streak.max(2)` совместно с `on_divergence`. Минимум streak `2` дублируется при разборе CLI (`trigger_streak.max(2)`). |
| Cap и путь записи | Да: `dump_count` против `max_files.max(1)` до записи → `CapReached` с предупреждением; после успеха — `fetch_add`; имя `b{height}.json`; `create_dir_all`, атомарная запись через `.json.tmp` + `rename`. |
| Идентификация дампа | Да: `source = divergence_probe`, `node_id`, `protocol_version = PWM_PROTOCOL_VERSION`, полное тело блока через serde JSON. |
| Time-align и приоритет | Да: `align_mid_on` отключает align при активном deterministic; в `lifecycle::run_with` `app.debug_align_mid` задаётся через `align_mid_on`, при конфликте флагов конфигурации — явный warning «deterministic wins»; ожидание в `spawn_seal_loop` перед `chain.seal` только если эффективный `debug_align_mid`. |
| По умолчанию OFF и обратная совместимость | Да: `DebugDumpCfg::default().on_divergence == false`, align и deterministic выключены в defaults CLI; включение дампа только флагами / truthy env; prod-путь синхронизации без изменений wire-протокола в этом слайсе. |

## 3. Style and module shape

- **Идентификаторы:** `python scripts/check_rust_fn_name_segments.py` по затронутым путям `pwmd` (включая `debug_dump.rs`, `peer_session/mod.rs`) — **нарушений нет**.
- **Модульность:** выделен `debug_dump.rs` с кратким `//!`; интеграция в транспорт и lifecycle локальная, без раздувания faсade.

## 4. Safety

- Дампы **opt-in**: риск утечки содержимого блоков ограничен явным включением оператором; путь каталога — доверенный параметр конфигурации (типичный классический footgun debug-инструментов).
- **Гонка по cap:** проверка `dump_count >= max` и последующее увеличение не атомарны как единая транзакция — при нескольких пирах теоретически возможно чуть превысить cap; для диагностического режима приемлемо как остаточный риск (не блокирует merge).
- Выравнивание секунды: при `wait > MID_WAIT_CAP_MS` ожидание сбрасывается в `0` — намеренно, чтобы не задерживать seal чрезмерно; осознанное ослабление «идеальной» midpoint-политики.
- Отдельное **наблюдение:** дамп сериализует полный `Block`; размер ограничен нормальной семантикой блока/TPS узла, не отдельным лимитом файла — при чрезмерных блоках в dev это всё же IO/место на диске оператора.

## 5. Tests

- По отчёту `docs/reviews/20260509-slice2-dump-timealign-testing.md`: модульные тесты `debug_dump` (путь, `mid_wait`, приоритет align vs det, успешная запись дампа), `dump_on_div_default_off`, смоук handshake — **pwm-testing PASS**.
- **Пробелы:** нет интеграционного сценария «две подряд divergence на одном пире перед дампом» и автотеста ветки `CapReached` при исчерпании лимита; поведение стрека покрыто статическим ревью плюс одиночным успешным дампом.

## 6. Verdict

**Approve with nits** — блокирующих дефектов для объявленного scope не выявлено; merge допустим с учётом зафиксированных пробелов в тест-пирамиде.

**Nits (не блокеры merge):**

1. Добавить по возможности узкий интеграционный или транспортный тест на `div_streak` (2 события) и/или на `DumpWrite::CapReached`, чтобы регрессии триггера и капа ловились автоматически.
2. При желании усилить гарантию cap — атомарная проверка-инкремент (CAS) или мьютекс вокруг секции дампа.

**Must-fix для merge:** нет.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/20260509-slice2-dump-timealign-final-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 16000
  confidence: medium
```

## 8. Residual risks (summary)

- Пробелы интеграционного покрытия стрека и `CapReached` (см. выше).
- Лёгкая гонка по глобальному `dump_count` при параллельных пирах.
- Оператор должен понимать, что дампы содержат полный блок и пишутся в выбранный каталог.

## 9. Merge readiness (Slice 2)

- **Результат гейта:** **PASS** (функциональные критерии выполнены, testing-артефакт **PASS**, must-fix отсутствуют).
- **Merge readiness:** **ready**.

---

_End of final review._
