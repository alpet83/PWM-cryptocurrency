# Pre-test architecture gate: P0 proposer rule and P2 scope (2026-05-08)

## Executive summary (owner-readable)

Зафиксированный в RFC 0015 (§7.1) контракт для цепочки: на каждую высоту **ровно один ожидаемый пропозер**, получаемый **детерминированно из фиксированного набора валидаторов и высоты** — без слотового голосования и без локальных эвристик по времени (§7.2–7.3). Предложение владельца **«UTC timestamp modulo среднее число пиров; при конкуренции — кто первым вошёл в чат»** **не согласуется** с этим контрактом как описание **канонического блок-пропозера**: среднее число пиров и порядок «first seen» **различаются между узлами**, время — манипулируемо и подвержено смещению часов. Для **MVP-тестов волн**, где проверяется синхронизация и разрешение веток, **нельзя** считать такое правило достаточным источником консенсусного прогресса без либо **явного отказа** от §7.1 в новой ревизии RFC, либо **уточнения**, что речь не о блок-пропозере, а о вторичном механизме (например, только тестовый orchestrator / не нормативный для цепи).

По **P2**: в RFC 15 уже есть операторская модель **сегмента** (`local_segment_id`, §13) и различие внутреннего сегмента vs внешних рёбер — этого достаточно для **нормативного «network zone» на уровне конфигурации и политики релея**, без обязательного нового обязательного поля в каждом wire-сообщении для ближайших волн. **Полная реализация** UDP broadcast-слушателя на LAN и автоматической классификации `local_broadcast` может быть **отложена** отдельным сетевым апгрейдом при сохранении уже описанной семантики §13.

**Итоговая рекомендация:** перед автоматическими wave-тестами **заблокировать в спецификации** единое правило ожидаемого блок-пропозера (согласованное с §7.1 или явная правка RFC), усилить определение «finalized_height» при разрешении веток (см. nits-register), и считать **net-zone** для MVP **конфигурационным квалификатором сегмента** (как в §13), а не полным дизайном полного mesh внутри кластера. Патч RFC — **желателен до старта волн**, если владелец настаивает на формуле P0 для продукта.

---

## Scope recap

- **Тикет:** `tasks/20260508-pretest-architecture-gate-p0-p2.json`.
- **Входы:** `docs/reviews/20260508-v2-8-nits-register.md`, `docs/rfc/15-same-shard-sync-v1.md`, `docs/plans/mvp_v2.md` (фрагмент Sprint V2-8).
- **Задача отчёта:** оценить P0 (правило пропозера), P2 (декларация vs реализация, сетевые зоны), выдать минимальный набор до тестов и формулировки для правок RFC 15 / примечания к плану.

---

## P0 sufficiency verdict

**CONDITIONAL** — **условно недостаточно для MVP**, если P0 претендует на роль **нормативного выбора блок-пропозера** для канонической цепи при текущем тексте RFC 15 §7.1.

**Обоснование в одном абзаце:** детерминизм и одинаковость результата на всех честных узлах при фиксированном генезисе и высоте нарушаются зависимостью от локальной топологии (`avg_peer_count`), от локального порядка доставки («first in chat»), и от глобально несогласованных часов. Это ухудшает предсказуемость ожидаемого подписанта блока, упрощает споры о ветках и создаёт поверхность для игры временем и порядком сообщений там, где RFC требует детерминированного разрешения среди валидных кандидатов (§7.2–7.3 поверх уже определённой модели пропозера).

**PASS** только при явном ограничении области: P0 описывает **не консенсусный** блок-пропозер, а например **эфемерный выбор координатора тестовой среды** или внутреннюю эвристику вне нормативного контракта цепи — с зафиксированным в RFC boundary.

---

## P0 risk matrix

| Риск | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Расхождение ожидаемого пропозера между узлами из-за разных `avg_peer_count` и представлений о пирах | High | High (спорные ветки, отклонение валидных блоков, лишний sync churn) | Зафиксировать пропозера только от `(validator_set_id, height)` и порядка валидаторов; среднее по пирам не использовать в консенсусе |
| Недетерминированный tie-break «first seen» / порядок входа в «чат» | High | High (разный fork-choice при одинаковых данных заголовков у разных узлов, нарушение §7.2 «без локальных временных эвристик») | Tie-break только из общих полей блока/цепи (как §7.3 для веток); не использовать локальный порядок сети |
| Манипуляция `utc_timestamp` и эксплуатация skew | Medium | Medium–High | Не опирать ожидаемого пропозера на wall-clock; NTP как операционный контроль, не как источник консенсуса |
| Операционная неясность: операторы не знают, кто «должен» производить блок | Medium | Medium | Документировать ожидаемого пропозера на height из спеки; метрики/алерты на unexpected proposer |
| Путаница «пропозер блока» vs «координатор gossip» | Medium | Medium | Явные термины в RFC: block proposer vs gossip coordinator |

---

## P2 decision matrix (declare vs implement now)

| Тема | Declare normatively now | Implement fully now | Рекомендация |
|------|-------------------------|---------------------|--------------|
| Сегмент / «зона» как операторский квалификатор (`local_segment_id`, классы релея, §13) | Yes — уже в RFC 15 §13 | Partial — достаточно флагов и метрик для wave-профиля | **Declare + минимальная реализация политики там, где уже есть код слайсов**; полный LAN UDP — отдельно |
| Обязательное поле net-zone на wire для всех сообщений | Optional extension | No для MVP волн | **Не требовать** нового обязательного поля до профиля межзоновых атак; опираться на handshake/конфиг оператора |
| UDP broadcast listener на LAN, full mesh внутри кластера | Informal / rollout note (§4.3, §13) | Defer | **Defer implementation** как отдельный network upgrade; нормативно достаточно ingress kind и suppression semantics |
| Кластер без full-mesh interior | Yes (егресс-first, anti-entropy) | Align tests with §13 knobs | **Зафиксировать в плане спринта**: тесты не предполагают all-to-all внутри сегмента по умолчанию |

---

## Recommended pre-test baseline (lock before Wave tests)

### «Go now» — минимальный набор правил

1. **Блок-пропозер для канона:** один источник истины, согласованный с RFC 15 §7.1 (фиксированный набор + высота + явный порядок валидаторов), без `avg_peer_count` и без «first in chat» для **ожидаемого** пропозера.
2. **Разрешение конфликтующих кандидатов на одной высоте:** строго §7.3 после валидности; без локальных временных эвристик (§7.2).
3. **finalized_height:** зафиксировать операторски-детерминированное правило источника и монотонности (закрывает P0 nit из nits-register).
4. **Сегменты для тестов с плотным ядром:** если включается storm guard — одинаковый `local_segment_id` у когорты, задокументированные значения `T_suppress` и сценарии anti-entropy (§13.5).

### «Must-not-skip» — контроли

- Нормативное разделение **legacy_observe** vs **full_v1** источников sync (RFC §5–6).
- Анти-DoS потолки на сообщения и окна catch-up (§8).
- Метрики отказов и причин reject для интерпретации волн (§10).
- Явная трассируемость в тикете/плане: что считается **in-scope** для волны (без скрытого расширения на UDP LAN до отдельного тикета).

### «Defer to next upgrade»

- Полноценный UDP multicast/broadcast listener и авто-discovery на всех LAN-интерфейсах.
- Обязательный wire-level «net_zone» во всех сообщениях.
- Любая смена §7.1 на не-height-детерминированную модель без нового RFC и версии протокола.

---

## RFC patch recommendations (exact text intent — bullets)

Ниже — **намерение формулировок** для правки RFC 15 (и при необходимости однострочной отсылки в `mvp_v2.md` у Sprint V2-8), без вставки готового патча в код.

- **§7.1 уточнение:** добавить явное определение: *Expected block proposer at height H SHALL be derived only from the ordered validator set committed in chain state (or genesis) and H; implementations MUST NOT use moving peer-count aggregates, wall-clock modulo, or first-seen peer ordering to determine the expected proposer.*
- **§7.2 перекрёстная ссылка:** *Ordering among simultaneous candidates remains §7.3 only; “who spoke first on gossip” MUST NOT substitute for fork-choice or expected-proposer derivation.*
- **Раздел «Терминология» или §7:** ввести отдельный термин **Gossip coordinator** (не нормативный для канона), если тестам нужен координатор — с пометкой *non-consensus test harness only*.
- **§13 / net-zone:** одна фраза: *Network zone for v1 SHALL be represented operationally by `local_segment_id` and peer policy classes; no mandatory net-zone field is required on the Section 6 wire envelope for v1 interoperability.*
- **Связка с nits-register:** подпараграф к §7 или §10: *`finalized_height` used in §7.3 comparisons SHALL be sourced from `<explicit rule: e.g., last finalized checkpoint advertised by PoA oracle / sealed in header>` and MUST be monotonic along a valid chain view; nodes MUST document the source when operating in testnet.*

---

## Participation / token estimate (orchestrator)

```json
{
  "agent": "pwm-review",
  "result": "PARTIAL",
  "artifacts": "docs/reviews/20260508-pretest-architecture-gate-p0-p2.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 5500,
    "confidence": "low"
  }
}
```

---

## Final verdict (gate)

**NEED_RFC_PATCH** — перед запуском автоматических wave-тестов как **нормативной** проверки поведения сети необходимо **устранить противоречие** между предложенным P0 и RFC 15 §7.1 **или** официально сузить P0 до неконсенсусного слоя сboundary в RFC. Параллельно рекомендуется **короткая правка** про `finalized_height` и (опционально) явное утверждение, что net-zone в v1 — **`local_segment_id` / политика**, без нового обязательного wire-поля.

**Условие перехода к READY_FOR_TESTS:** принятые владельцем правки RFC (или письменный waiver с версией протокола) + зафиксированный в тикете wave-scope без неявного UDP-кластера.

---

_Report: pwm-review, architecture gate only (no production code)._
