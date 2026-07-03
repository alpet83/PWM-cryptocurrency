---
name: MVP v7 Plan
overview: "План MVP v7: внешний контур (production offchain batch API, svcpool.io интеграция), devnet на базе V6; operator UX (TUI pending conservation); pwm-cli addr-bruteforce v2; emergency stake evacuation (ADR 0012); ADR-gate миграции BFT (без кода BFT). Throughput gate >=50 tx/s достигнут через flamegraph-оптимизацию (V7-S1, 2026-06-29): ~76 tx/s sustained; остаточный bottleneck — P2P wire JSON, запланирован в Фазе 4."
todos:
  - id: v7-1-perf-pipeline
    content: "V7-1: Throughput gate >=50 tx/s — достигнут через flamegraph+perf quick-wins (~76 tx/s, 2026-06-29); SEDA deferred (bottleneck P2P wire, не CPU)"
    status: done
  - id: v7-2-bruteforce
    content: "V7-2: addr-bruteforce v2 (occupied-skip + CPU MT) — корректный brute при пересекающихся профилях флагов"
    status: done
  - id: v7-3-tui-conservation
    content: "V7-3: Operator TUI — отображение pending conservation transfers; AcctOut.pending_conservation + fee_pwm; TUI compact row + unit tests"
    status: done
  - id: v7-4-stake-evac
    content: "V7-4: Emergency stake evacuation (ADR 0012) — atomic evac staked_pwm_raw + balance_pwm на rescue в том же apply_tx"
    status: done
  - id: v7-5-offchain-batch
    content: "V7-5: Production offchain batch burn API (Merkle root + proof) — /v1/offchain/*; anchor surrogate (consensus-visible anchor — Phase 4)"
    status: done
  - id: v7-6-devnet-launch
    content: "V7-6: Devnet genesis + operator onboarding — 21B genesis, validator onboarding поверх V6 PoS, документация + throughput gate"
    status: in_progress
  - id: v7-7-bft-adr
    content: "V7-7: ADR-gate по BFT migration (CometBFT / custom / Option A продолжение); runtime V7 без замены Chain::seal"
    status: pending
  - id: v7-closeout
    content: "V7 closeout: MVP-checklist 0v7, CONCEPT_ROADMAP, GLOSSARY, CHANGELOG, pre-pub soak"
    status: pending
isProject: false
---

# MVP v7 — Внешний контур + Devnet

## Контекст и роль оркестратора

- **Предусловие:** MVP v6 полностью закрыт (owner sign-off 2026-06-17); публикация mirror pending. См. [plans/mvp_v6.md](mvp_v6.md), [releases/v6.0.0.md](../releases/v6.0.0.md), [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md) §MVP V6.
- **Якоря:** [docs/AGENT_PROMPT_orchestrator.md](../AGENT_PROMPT_orchestrator.md), [docs/plans/mvp_v6.md](mvp_v6.md), [docs/CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md) §MVP V7, [DRAFT_WHITEPAPER-ru.md](../../DRAFT_WHITEPAPER-ru.md).
- **Делегирование:** Продолжается модель V6 (worktree + bridge для prolonged slices в `crates/`; docs/tasks/scripts — синхронно оркестратором). Весь конвейер review/testing в worktree. Вести [docs/ORCHESTRATOR-NOTES.md](../ORCHESTRATOR-NOTES.md) после каждого слайса.
- **Принцип:** V7 — **внешний контур** поверх стабилизированного V6 (PoS admission + address flags + Mode B). BFT замена — **только ADR в V7**, код в Фазе 4. Devnet + интеграции (svcpool.io, offchain).

**Текущий статус (на момент плана):** V7-1/2/3 частично инициированы (тикеты в tasks/), но детальный план `mvp_v7.md` отсутствовал — настоящим документом фиксируем.

---

## Цель и demo-ready результат

**Цель:** Сделать PWM готовым к реальным интеграциям (offchain batch для email/AI/КИИ), запустить devnet с 21B genesis и onboarding валидаторов поверх V6 stake admission. Закрыть ключевые operator UX gaps из owner soak V6.

**Главный demo-ready результат (devnet + интегратор):**

1. Оператор в TUI видит **pending conservation** (сумма, получатель, высоты enqueue/execute, countdown) для адресов с флагом `CONSERVATION` (bit 1) после отправки.
2. `pwm addr-bruteforce` корректно находит хиты **ниже** высокого premine индекса при смене профилей флагов (occupied-skip + MT).
3. При emergency `ActivatePolicy` на скомпрометированном адресе **весь** баланс (liquid + staked) эвакуируется на rescue (ADR 0012).
4. Production-ready **offchain batch burn API** (Merkle root + on-chain anchor) — не stub; клиенты (svcpool.io и др.) могут верифицировать.
5. Devnet: genesis с реалистичным распределением 21B, stake admission работает, операторы могут присоединяться.
6. **Throughput gate**: одна нода уверенно переваривает ≥ 50 tx/s sustained (с mix политик, conservation, cross-shard) под ramp-нагрузкой (используя существующий harness `cy_cluster_transfer_ramp_soak.py`). Текущий baseline ~3 tx/s должен быть радикально улучшен за счёт параллельной pre-processing.

---

## Scope / Out of scope

| In scope (V7)                          | Out of scope (defer) |
|----------------------------------------|----------------------|
| TUI read-only conservation pending UX | Полноценный history modal для pending / отмена |
| addr-bruteforce v2 (CPU MT + skip)    | GPU/OpenCL; автоматический перебор всех масок |
| Emergency stake evac (ADR 0012)       | Cross-shard activation_target; marks evac |
| Production offchain batch API (Merkle + on-chain) | Payment channels (Lightning-style) |
| X-PWM email header reference impl (минимальная) | Полноценный MTA plugin для продакшена |
| AI API gateway stub (mark gate)       | Полноценный billing / rate-limit сервис |
| Devnet + 21B onboarding       | Production mainnet |
| ADR BFT migration (gate, не реализация) | Код замены `Chain::seal` (Фаза 4) |
| /v2/* API surface design (при необходимости) | Ломка /v1/* без веской причины |
| Обновление runbooks, docs, GLOSSARY   | Полная реорганизация operator tooling |
| **Tx pipeline parallelism prep (V7-S1)** | Полноценная замена seal engine / sharded execution (если понадобится — отдельный ADR) |

---

## Принятые решения V7 (черновик)

- **TUI conservation (V7-1):** Read-only. Данные из существующей `PendingConservationTransfer` очереди (V6-8). Минимальное расширение API (`/v1/account` или dedicated `pending_conservation` slice) — **additive**. Нет новых tx в TUI. Паритет UX с V5-6 marks saturation (строка + detail).
- **Bruteforce v2 (V7-2):** Resume = skip всех `derivation_index` из wallet v3 (не `max+1`). MT (rayon/parallelism) с детерминированным "первый минимальный hit". Без изменения wire/CLI контракта флагов. OpenCL — explicit out-of-scope.
- **Stake evac (V7-3):** Atomic в том же `apply_tx`, что и V6 balance evac. Reuse логики `Unstake` (validator set side effects). Нет нового типа tx. `staked_pwm_raw` → `activation_target.balance_pwm`. Marks не эвакуируются.
- **Offchain (V7-4):** Продолжение централизованной batch модели (ADR 0003 / R8) — Merkle root + on-chain anchor. Клиентская верификация. Не state channels.
- **BFT:** V7 — только **ADR-gate** (выбор пути, границы `Chain::seal`, RFC16 compat, rollback). Код — только в Фазе 4 после Accepted. Runtime V7 продолжает incremental PoS Option A.
- **Devnet:** 21B genesis (дизайн из V5) + staking поверх V6 active set. Onboarding runbook. Обязательный throughput soak как gate.
- **Tx pipeline & parallelism (ключевой технический трек V7):**
  - **Проблема:** В текущем дизайне (V6) практически вся работа (`validate_tx_shape`, `evaluate_policy`, `apply_tx`, conservation checks, drain) происходит **последовательно внутри `Chain::seal`** (или непосредственно перед ним в proposer). Это приводит к наблюдаемому лимиту ~3 tx/s.
  - **Предпочитаемый подход:** Конвейеры данных с **неблокирующими очередями** (non-blocking / lock-free где возможно) + **события/семафоры** для сигнализации. Избегать разделяемых мьютексов на горячем пути, чтобы минимизировать дедлоки и contention.
  - **Архитектура ноды в целом (растянуть на сервер):**
    - **Главный поток (orchestrator)**: в основном перемещает данные между очередями, координирует, выполняет `seal` (лёгкий атомарный шаг). Не занимается тяжёлым I/O или предварительной обработкой. Решение "отдельный OS-тред или tokio-задача" откладывается до прояснения архитектуры.
    - **Фоновые потоки / пайплайны**:
      - Обработка сокетов / транспорта (peer + RPC ingress).
      - **Диспетчеризация по источнику и типу** (см. детальную модель ниже).
      - Пул воркеров: воркеры получают во владение всё необходимое для задачи (в т.ч. сокеты для ответов клиентам). Оркестратор забирает задания по временному порядку и передаёт соответствующему воркеру.
      - Мягкая защита от DoS: на каждую очередь ограничено число одновременно выделяемых воркеров. Часть воркеров может быть affinity-привязана к конкретной очереди (например, проверка транзакций на консенсус) и конкурировать внутри неё без дополнительной оркестрации.
    - Отдельные очереди по классам сообщений (комфортнее для DoS: "очередь заполнена — приходите завтра").
    - Данные текут через очереди: raw tx → verified+policy-checked → prepared-for-seal → orchestrator забирает батч → seal.
  - **Принцип для V7:** Тяжёлую/параллелизуемую работу **выносим до seal** через staged pipeline. `seal` остаётся минимальным, атомарным, детерминированным шагом коммита подготовленного батча.
  - **Decoupling:** Подготовка батча может идти параллельно с предыдущим seal. Главный поток только оркестрирует и запечатывает.
  - **Диспетчеризация сети (конкретная модель):**
      - 1. Транзакции клиентов → идут в фоновую обработку консенсуса. Сокет соответствующего соединения помечается связанным с внутренним "заданием" (job). Задание попадает в соответствующую очередь, воркер получает его во владение вместе с сокетом.
      - 2. Транзакции, пред-проверенные в кластере → приходят уже локально готовыми к запечатыванию. Сразу маршрутизируются в очередь "на мемпул" (готовы к seal). Могут обслуживаться affinity-воркерами, привязанными к этой очереди.
      - 3. Запросы на подгрузку истории / backfill → направляются в отдельную очередь обслуживания data-broadcast (полностью изолированный путь).
      - Read-запросы:
        - Ультра-лёгкие (микросекунды): текущий номер блока, head и т.п. — можно отвечать синхронно рано (лучше всего на приёме из сокета), не занимая воркера.
        - Требующие копания в блокчейн (доступ к состояниям аккаунтов, историческим данным и т.п.): диспетчер-оркестратор обязан занять свободного воркера. Промахи кэша состояний аккаунтов пока оставляем за пределами MVP.
      - Оркестратор выбирает задания из очередей по времени поступления и передаёт свободному (или affinity) воркеру. На каждую очередь — лимит одновременно активных воркеров для мягкой защиты от DoS.
      - **Правило занятия воркера**: Любой запрос/задание, требующее "копнуть блокчейн" (доступ к состояниям аккаунтов, применение политик, история и т.п.), **обязательно** проходит через диспетчер-оркестратор для занятия свободного воркера. Ультра-быстрые операции (текущая высота блока и т.п.) могут оставаться синхронными на этапе приёма из сокета.
  - **Связь с BFT:** При выборе CometBFT / custom BFT в ADR (V7-6) одним из обязательных критериев оценки будет «насколько хорошо поддерживает pipeline + non-blocking dispatch и не блокирует proposal path».
  - **Simplicity constraint:** Параллелизм и очереди только там, где легко доказать детерминизм порядка и результата. Чёткие границы ownership данных. Backpressure через bounded очереди + семафоры.
  - **Измеримость:** Используем и развиваем существующий harness (`scripts/cy_cluster_transfer_ramp_soak.py` + `_analyze_transfer_ramp.py` + block_timing) как основной инструмент.

**Simplicity gate (повторяем из V6 + усиление):** reuse существующих очередей/state paths, additive wire, один путь evac. Параллелизм — только pre-seal и только с доказуемой детерминированностью результата.

---

## Спринты V7

### Таблица спринтов V7

| Спринт | Суть | Статус |
|--------|------|--------|
| **V7-1** | Tx pipeline perf (flamegraph + Arc<SignedTx>, ~76 tx/s) | ✅ Done |
| **V7-2** | addr-bruteforce v2 (occupied-skip + CPU MT) | ✅ Done |
| **V7-3** | TUI pending conservation (AcctOut + fee_pwm + unit tests) | ✅ Done |
| **V7-4** | Emergency stake evacuation — atomic staked+liquid evac (ADR 0012) | ✅ Done |
| **V7-5** | Production offchain batch API (Merkle root + proof, /v1/offchain/*) | ✅ Done |
| **V7-6** | Devnet genesis + operator onboarding — 21B genesis + validator onboarding + throughput gate | 🔄 In progress |
| **V7-7** | BFT ADR-gate (выбор пути, без кода замены Chain::seal) | ⏳ Pending |

---

> **Принцип оформления детализации:**  
> Головной план определяет цели, границы скоупа, ключевые архитектурные решения, порядок работ и критерии приёмки.  
> Подробные схемы (Mermaid), модели очередей, владения задачами, dispatch flows и пути реализации разрабатываются **в рамках соответствующего спринта**.  
> Черновики планов спринтов размещаются рядом (например `mvp_v7s1.md`) и туда выносятся детальные схемы и реализации.

### Sprint V7-1: Tx pipeline perf

**Черновик плана спринта:** [mvp_v7s1.md](mvp_v7s1.md)

Сюда выносятся все детальные схемы (Mermaid), модели очередей, пул воркеров, правила диспетчеризации, владения задачами и реализационные детали. Головной план держит только цели, скоуп и порядок работ.

**Цель:** Сломать текущий потолок ~3 tx/s. Подготовить архитектуру, при которой тяжёлая работа не блокирует основной поток seal/proposal. Это критично для credibility публичного devnet.

**Контекст:**
- Бенчмарки (owner lab + harness `cy_cluster_transfer_ramp_soak.py`) показывают, что одна нода с трудом тянет >3 tx/s.
- Вся критичная логика сейчас последовательно в `pwm-core::chain::seal` → `apply_tx_with_ctx` (validate + evaluate_policy + мутации) + `drain_conservation`.
- Ранее копать не стали «потому что скоро CometBFT». V7 — это как раз момент, когда мы **принимаем решение** по консенсусу, поэтому должны готовить runtime независимо.

**Scope (высокий уровень):**
- Диагностика текущих bottleneck'ов.
- Переход к pipeline-архитектуре на базе отдельных очередей, пула воркеров и оркестратора-диспетчера.
- Разделение лёгких операций (могут обрабатываться рано) и операций с доступом к состоянию (требуют занятия воркера).
- Выделение pre-processing до seal + подготовка prepared batch.
- Минимальный decoupling пути seal.
- Усиление критериев BFT-ADR с учётом pipeline-подхода.
- Целевой гейт: sustained ≥50 tx/s.

**Детали и схемы** — см. [mvp_v7s1.md](mvp_v7s1.md).

**Acceptance:**
- Есть воспроизводимые цифры "до" и отчёт по профилированию.
- Приняты ключевые решения (пул воркеров, отдельные очереди, диспетчер-оркестратор, лимиты на очередь, разделение по глубине доступа к state).
- Sustained ≥ 50 tx/s за ≥ 60 секунд под ramp-нагрузкой при нулевых seal-детерминизм-ошибках — полный критерий в `mvp_v7s1.md` § Критерий приёмки спринта.
- Seal путь стал легче.
- Детальные схемы и модель оформлены в спринтовом плане (mvp_v7s1.md).

**Декомпозиция (ориентир):**
- Диагностика + классификация операций.
- Модель очередей + диспетчеризация (3 пути).
- Пул воркеров, affinity, лимиты, backpressure.
- Интеграция с оркестратором и seal.
- Тесты, соаки и документация (Mermaid).

Подробности — в [mvp_v7s1.md](mvp_v7s1.md).

**Тикет-якорь:** Рекомендуется создать/расширить `tasks/2026...-v7-perf-tx-pipeline.json`.

Детальная проработка (модели, схемы Mermaid, владение задачами и т.д.) ведётся в [mvp_v7s1.md](mvp_v7s1.md) и тикете спринта.

Головной план фиксирует только цели, границы и ключевые решения.

**Эскалация и оптимизация:** Если после Slice 0 выясняется, что узкое место I/O-bound, или Slice 4 не даёт ≥50 tx/s — см. [`docs/plans/perf-optimization-spectrum.md`](perf-optimization-spectrum.md): тир 2 (ClickHouse как high-perf backend + `ShardStateCert`) и тир 3 (BFT). Документ также закрывает открытый вопрос о подписи состояния (canonical binary, не JSON).

**Делегирование:** worktree_bridge (pwm-core + pwmd). Обязательно с сильным pwm-review + тестированием на determinism, отсутствие дедлоков и корректную работу ограничений воркеров.

В handoff явно указывать желаемую модель и ссылаться на [mvp_v7s1.md](mvp_v7s1.md). Начинать консервативно с учётом опыта команды.

---

### Sprint V7-3: TUI pending conservation transfers

**Цель:** Оператор видит, что исходящий transfer с флагом `CONSERVATION` (bit 1) ушёл в очередь и когда исполнится.

**Предусловие:** V6-8 (chain queue + drain) + V6-10 soak.

**Scope:**
- TUI: расширить отображение аккаунта (Owner/Receivers панель или detail) — строка "conservation pending: N tx, next at H+Δ".
- Детальная панель / модал / F-клавиша: список `PendingConservationTransfer` (sender, amount, to, enqueue_height, execute_at_height, remaining).
- Минимальный RPC: если `/v1/account` не отдаёт pending — добавить поле или узкий `/v1/account/:id/pending-conservation` (additive, документировать в api-v1.md).
- Обновление по poll head height.
- Документация: pwm-tui.md + runbook оператора (v6-owner-stability-soak или новый).

**Acceptance:**
- После `tx-send` с адреса `flags=...2` (CONSERVATION) в TUI видно pending-запись.
- При продвижении height — countdown уменьшается, после drain — запись исчезает.
- Нет новых путей отправки tx из TUI (read-only).
- `cargo test -p pwm-tui` + manual smoke на CY с flags=2.

**Декомпозиция (ориентир):**
- Слайс 1: RPC extension + types (если нужно).
- Слайс 2: TUI UI + poll + rendering.
- Umbrella: `tasks/YYYYMMDD-v7-s1-tui-conservation.json`.

**Делегирование:** worktree_bridge (TUI + возможный pwmd slice).

---

### Sprint V7-2: addr-bruteforce v2 (occupied-skip + CPU MT) ✅

**Цель:** Корректный brute при пересекающихся профилях флагов и premine на высоких индексах. Ускорение через MT.

**Контекст (из owner lab + CONCEPT_ROADMAP):**
- Текущий resume = `max(derivation_index) + 1` — отрезает низкие индексы при разных masks.
- Нужно: Occupied set из wallet v3; skip; MT с первым минимальным хитом-победителем.

**Scope (pwm-cli только):**
- `crates/pwm-cli/src/bruteforce.rs` + helpers.
- Алгоритм: scan от 0, skip `i ∈ occupied`, match flags+domain.
- `--threads N` (default = available_parallelism()).
- Детерминизм: минимальный `i` выигрывает независимо от chunking.
- Regression: wallet с high premine + разные masks.
- Обновить `pwm-cli.md`, runbook addr-bruteforce, семантику resume.

**Acceptance (из тикета 20260616-v7-bruteforce-occupied-skip-mt.json):**
- occupied skip работает, находит hits ниже premine.
- MT + determinism.
- Нет breaking contract флагов.
- Docs обновлены.

**Тикет-якорь:** `tasks/20260616-v7-bruteforce-occupied-skip-mt.json` (статус open).

**Делегирование:** worktree + bridge (pwm-cli).

---

### Sprint V7-4: Emergency stake evacuation (ADR 0012) ✅

**Цель:** При активации emergency весь экономический остаток (liquid + staked) попадает на rescue.

**Предусловие:** ADR 0012 accepted (уже выполнено — статус «Accepted as V7 normative contract», см. `docs/adr/0012-emergency-stake-evacuation.md`); V6-7 emergency activation.

**Scope:**
- `pwm-core state::apply_tx` (Policy arm): после V6 evac balance — если `staked > 0`, atomic:
  - `staked_pwm_raw = 0`
  - `activation_target.balance_pwm += staked_amount`
  - side effects как у успешного `Unstake` (validator set, epoch admission).
- Нет wire изменений (`ActivatePolicy` остаётся тем же).
- Unit: stake>0 + stake=0 regression.
- CY e2e: victim stakes → activate → rescue получает всё.
- Обновить runbook v6-owner-stability-soak-50k.md (step 8 oracle: теперь включает stake).

**Acceptance (из тикета + ADR):**
- `staked_pwm_raw` жертвы = 0 после активации.
- rescue баланс = прежний + stake жертвы.
- Validator set обновляется (epoch boundary видит снижение).
- finalized victim не может Unstake отдельно.
- V6 behaviour (stake=0) не сломан.

**Тикет:** `tasks/20260617-v7-emergency-stake-evacuation-impl.json` (backlog).

**Simplicity:** один путь в apply_tx; reuse unstake helpers.

---

### Sprints V7-5, V7-6, V7-7

**V7-5 Offchain batch (production) ✅:**
- Заменить stub (`docs/OFFCHAIN_STUB.md`) на полноценный API (Merkle tree + anchor tx).
- Endpoint(ы) для submit batch + verify.
- On-chain anchor (специальная tx или reuse burn с purpose).
- Клиентские примеры + верификация (svcpool.io path).
- См. R8 в roadmap.

**V7-6 Devnet genesis + operator onboarding 🔄:**
- Genesis 21B (дизайн genesis-21b-design.md + V5).
- Onboarding валидаторов (stake + active set V6).
- Документация: quickstart, operator guide, "how to join devnet".
- Связь с outreach (матрица в CONCEPT_ROADMAP).
- **Обязательный throughput gate:** ramp-тест (через существующий harness) показывает стабильные ≥50 tx/s (или согласованный с владельцем минимум) без критичной деградации seal_slip. Результаты Sprint 1 (V7-S1) должны быть применены.

**V7-7 BFT ADR-gate:**
- ADR (новый или в adr/): выбор пути (CometBFT / custom Rust BFT / продолжение incremental PoS).
- Границы: что меняет `Chain::seal`, как сохраняется RFC16 cluster, rollback.
- **Код BFT не входит в V7.** Только decision + migration plan.
- Runtime V7 = V6 seal path + PoS admission.
- **Обязательный критерий оценки кандидатов (добавить в ADR):** насколько выбранный подход **поддерживает / не мешает** параллельной pre-processing транзакций и не создаёт узкое место в proposal/seal пути. Результаты Sprint 1 (V7-S1) должны учитываться при выборе.

**Черновик плана непосредственной имплементации CometBFT** (для Фазы 4 после принятия ADR): [mvp_v7_cometbft.md](mvp_v7_cometbft.md). Начинается с ТЗ, обоснования и вариантов реализации.

**Дальнейшая декомпозиция:** будет в umbrella тикетах + обновлениях этого плана.

---

### Closeout V7

- Обновить [docs/MVP-checklist.md](../MVP-checklist.md) блок 0v7.
- [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md) §V7 пометить закрытым.
- GLOSSARY, CHANGELOG, releases/v7.0.0.md.
- Pre-publication: stability soak, code audit (по аналогии V6), docs.
- Owner sign-off + mirror.

---

## Обязательный ритуал оркестратора (V7)

- Тикеты `tasks/<id>.json`: in_progress, `planned_for: "V7-N"`, acceptance, delegations.
- `scripts/_orchestrator_ticket_id_guard.py <ticket_id>`.
- Prolonged → worktree; review+testing **в worktree**.
- После слайса: запись в [ORCHESTRATOR-NOTES.md](../ORCHESTRATOR-NOTES.md).
- PASS_WITH_NITS mechanical — auto-close.

---

## Межспринтовые гейты

- **Simplicity Gate:** reuse queue/state paths; никакой новый async/VM; один путь evac. Параллелизм разрешён только на pre-seal стадии и только при гарантированной детерминированности агрегации результатов.
- **Additivity Gate:** wire additive; snapshot v4+ (или v5 если нужно) с миграцией.
- **Layer gate:** L7 (offchain) не меняет L1–L4 контракты.
- **Devnet gate:** 21B genesis + staking + **throughput ≥ целевого уровня** (см. V7-5 и Sprint 1). Результаты ramp-теста должны быть частью closeout.
- **Determinism gate:** любые параллельные куски (sig, policy, dry-run) обязаны давать идентичный итоговый батч независимо от количества потоков.

---

## Риски и контрмеры (V7)

| Риск | Контрмера |
|------|-----------|
| Conservation pending UX complexity (много записей) | Cap per-account + compact view; только read |
| Bruteforce MT non-determinism | Гарантировать "min index wins"; тесты |
| Stake evac в mixed V6/V7 сети | Height-gated rollout или feature в GenCfg; docs |
| Offchain batch доверие (централизация) | Merkle + клиентская verify; anchor on-chain |
| Scope creep (полноценный /v2 + все use cases) | Строгий MVP: batch + TUI + 3 спринта; остальное в V7.x / post |
| BFT ADR затянется | Таймбокс + explicit "Accepted / Defer" с owner note |
| **Throughput: параллелизм сильно усложнит код и нарушит simplicity principle** | Начинать с профилирования. Параллелизм только pre-seal + чистые функции. Любое усложнение — с явным обоснованием и review. |
| Детерминизм сломается при параллельной обработке | Жёсткие тесты на ordering + property-based тесты на "результат батча одинаков при 1 и N потоках". |
| ~3 TPS останется и после Sprint 1 (корневая причина глубже) | Sprint 1 начинается с точной диагностики (не предполагать). Если нужно радикальное изменение модели (например, sharded apply или другая структура state) — выносится в отдельный ADR и, возможно, за пределы V7. |
| Offchain batch поможет, но core path останется узким | Offchain (V7-4) + parallelism (Sprint 1) работают вместе. Batch даёт "эффективный" throughput, parallelism — сырой capacity. |

---

## Артефакт после согласования

- Утверждённый текст сохранён в **[docs/plans/mvp_v7.md](mvp_v7.md)** (frontmatter todos).
- Обновлён [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md) (ссылки на план).
- Добавлен блок в [MVP-checklist.md](../MVP-checklist.md) 0v7.
- Тикеты V7-1…V7-3 привязаны к спринтам.
- ORCHESTRATOR-NOTES: bootstrap V7 при необходимости.

**Сводка для владельца:**

MVP v7 фокусируется на **product-ready внешнем контуре** (offchain API + devnet) + полировке UX из V6 soak + **критическом треке производительности** (Sprint 1 / V7-S1).

Ключевой новый элемент: подготовка многопоточной обработки транзакций (decoupling pre-processing от `seal`), чтобы преодолеть текущий лимит ~3 tx/s. Это делается **независимо** от решения по BFT (которое тоже в V7, но только как ADR).

Существующий ramp harness становится основным инструментом измерения. Цель для devnet — минимум ~50 tx/s sustained.

Три ранних спринта (TUI, bruteforce, stake evac) + новый Perf-трек + интеграции + BFT-ADR. Процесс наследует V6 (worktree, NOTES, simplicity + усиленный determinism gate).

---

**Статус на 2026-06:** 
- V7-1/2/3 частично пронумерованы тикетами.
- Существует harness для throughput (20260617-cy-cluster-transfer-ramp-throughput + runbook).
- Детальный план V7 обновлён с явным Perf-треком.

Следующий шаг (рекомендация): 
1. Открыть/расширить тикет Sprint 1 (профилирование текущего bottleneck'а).
2. Обсудить целевую цифру TPS для devnet gate.
3. Привязать результаты Perf к критериям V7-6 (BFT ADR).

План будет итеративно шлифоваться по мере owner feedback, результатов профилирования и слайсов (как mvp_v6.md).