# Sprint V2-1 — Slice E: testing gate report (docs-only)

**Дата:** 2026-05-05  
**Агент:** `pwm-testing`  
**Входы:** [sprint-v2-1-slice-e-implementation-handoff.md](./sprint-v2-1-slice-e-implementation-handoff.md), [tasks/20260505-v2-s1-slice-e-implementation-handoff.json](../tasks/20260505-v2-s1-slice-e-implementation-handoff.json)

---

## Verdict: **PASS**

**Обоснование:** Slice E как **implementation handoff (docs-only)** агрегирует lock-пакет A–D (§1), **file-level map** для первой код-волны (§2), пофазовый rollout **E-1/E-2/E-3** с явными **гейтами** (§3) и **минимальный test-plan** для `pwm-testing` (§5) — шесть блоков (tx/schema, state/maturity, free-day, anchor/reorg, API parity, compatibility). Этого достаточно, чтобы после появления кода в `crates/*` спроектировать табличные и интеграционные кейсы без расширения экономического scope V2-1. Done criteria §6 явно требуют последующий **исполняемый** test-gate по tx/state/policy/api/reorg — текущий тикет этот прогон **не закрывает**.

**Ограничение:** исполняемые тесты **не запускались** и **не добавлялись** (**docs-only**, без правок `crates/*`); настоящий gate оценивает **тестопригодность handoff** и согласованность фаз/критериев, а не CI.

---

## 1. Test-readiness по кодовым слайсам E-1 / E-2 / E-3

| Слайс | Оценка | Комментарий |
|-------|--------|-------------|
| **E-1 (foundation, consensus-first)** | **READY** | Цель и точки входа заданы (`pwm-core` tx/state/policy, replay/chain). Гейт «deterministic apply/replay + быстрые smoke/unit» напрямую мапится на §5.1–§5.4 (без pwmd wire). После кодирования — приоритет: `purpose`/ClaimTx shape, maturity continuity reset, `floor`, C-ANC/C-MAT/C-FRE, reorg replay. |
| **E-2 (API and preflight parity)** | **READY** | Зависит от E-1 + freeze Slice D: `E_* → response_class`, reject JSON (`phase`, `tx_kind`, `response_class`, `error{code,message,trace_id}`), симметрия preflight/mempool/apply на одном snapshot. Гейт формулируется проверяемо (совпадение класса/кода между фазами). |
| **E-3 (client adaptation)** | **PARTIAL** | `pwm-cli`: non-interactive тесты (args/serde/reject-field reads) пригодны по политике тест-агента. `pwm-tui`: визуал и экранный вывод без machine-readable канала — регрессии reject-полей остаются **operator/manual** или требуют явного продукта (test hook); legacy BurnMark v1 adapter и «документация миграции» не зафиксированы здесь как отдельные frozen артефакты — понадобятся ссылки/update после кодирования. |

---

## 2. Сопоставление с §5 test-plan handoff (coverage target до появления кода)

| Блок §5 | Статус на сейчас | Замечание |
|---------|------------------|-----------|
| Tx/schema (`purpose`, ClaimTx mode/fee) | **Спецификация есть, автотестов нет** | Граничные случаи перечислены; после реализации — table-driven в `pwm-core`. |
| State/maturity (continuity reset, `floor`, no over-claim) | **Спецификация есть** | Риск §4.2/§4.3: единый path округления и replay — критично для негативов. |
| Free-day (одна free/день, paid fallback) | **Спецификация есть** | Нужен тестовый контроль **chain `utc_day`** (не wall-clock узлы); см. §4 п.6 handoff. |
| Anchor/reorg (C-ANC-A..D, orphaned effects) | **Спецификация есть** | Интеграция с replay/rollback из §2.1 map. |
| API parity (preflight/apply, обязательные поля reject) | **Спецификация есть** | Опирается на Slice D; golden/fixture после wire в `pwmd`. |
| Compatibility (v1 adapter, клиенты v2 по умолчанию) | **PARTIAL norm** | v1 adapter path заявлен в §1 п.10; детали адаптера и CLI default — только в коде/доп. доке. |

---

## 3. Gaps и риски (до исполняемого PASS V2-1)

1. **`crates/*`:** ни одного нового автотеста в рамках Slice E-тикета — ожидаемо до coding leg **E-1**.
2. **Миграция `marks_quota → marks`:** handoff §4 перечисляет скрытые ссылки и double-accounting; тест-план должен включить регрессию «нет утечки legacy quota в сериализации/API» после рефакторинга.
3. **Детерминизм времени:** free-day и `utc_day` требуют фикстур chain time; негативные кейсы с локальным временем ноды вне спецификации — сознательно исключить из harness.
4. **`error.message` / `trace_id`:** как в Slice D — разумно фиксировать `code` + `response_class` + обязательные поля; жёсткий формат `trace_id` не расширен в E-handoff.
5. **E-3 TUI:** без договорённого machine-readable вывода — нет стандартного автоматического gate на «показ reject-полей»; документировать manual check или hook в отдельном соглашении.
6. **Межслайсовая зависимость:** исполняемый PASS по API parity (E-2) блокируется завершением consensus path (E-1) и стабильным error registry в `pwmd`.

---

## 4. Рекомендации `pwm-testing` на coding legs

- Начинать автоматизацию с **E-1** строго по §2.1 + гейт §3 (unit/replay smoke), затем **E-2** — негативы от таблицы Slice D + симметрия фаз, затем **E-3** — CLI serde/отображение кодов; TUI — минимальные pure helpers или manual.
- Закладывать **единый snapshot** для сравнения preflight vs apply (как в D-CON) и отдельные кейсы drift при смене anchor.
- После каждого код-слайса: `cargo fmt`, `cargo test` по затронутым crates, preflight `target/debug` по `docs/AGENT_PROMPT_testing.md`.

---

## Participation / token estimate (`pwm-testing`)

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/sprint-v2-1-slice-e-test-report.md
  - tasks/20260505-v2-s1-slice-e-implementation-handoff.json
commands: []
cleanup: n/a (no spawned processes)
preflight_target_debug: n/a (no cargo build/test)
snapshot_benches: n/a (slice not pwmd snapshot)
token_usage:
  source: estimate
  input: null
  output: null
  total: 4200
  confidence: low
```

_Оценка по объёму handoff (lock + фазы + test-plan §5) без провайдера токенов._
