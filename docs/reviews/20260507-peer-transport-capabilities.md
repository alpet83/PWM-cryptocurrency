# Ревью: возможности пиринговой связи (PWMD transport + RFC 8)

**Дата:** 2026-05-07  
**Область:** `crates/pwmd/src/transport/**`, `spawn_seal_loop`, `docs/rfc/8-shard-runtime-identity-and-peering.md`, ADR 0001.

---

## Executive summary

Текущий транспорт PWMD после handshake обменивается только сообщениями уровня **identity / liveness / кросс-шард фактов / обзоров счетов / федеративного gossip**: в `PeerWireMsg` нет типов для блоков, заголовков, инвентарей или запросов синхронизации цепочки. Блоки создаёт **локальный** PoA-контур: `spawn_seal_loop` периодически вызывает `chain.seal` при готовности `InitState`; загрузка истории с диска при старте через snapshot использует `absorb_blocks_tail`, но это не сетевой путь. В `NodeHello.capabilities.services` перечислены `mempool` и `sync`, однако на проводе отсутствует протокол передачи цепочки — поле `chain_tip_height` в heartbeat/hello попадает в **federation**-таблицу наблюдаемых высот, а не инициирует докачку блоков. RFC 8 описывает целевую модель native/foreign, приоритеты и будущие gossip/sync; полный набор acceptance criteria из §11 пока не реализован как рабочий chain-sync между нодами одной шарды.

---

## Таблица возможностей

| Возможность | Да / нет / частично | Код (ориентир) | RFC / спека |
|-------------|---------------------|----------------|-------------|
| Handshake с подписанным `NodeHello` (сеть, genesis, cluster, node, capabilities) | **Да** | `handshake.rs`, `wire.rs` (`Hello` / `HelloAck`), `seed/handshake.rs`, `inbound.rs` | RFC 8 §5 (частично: структура данных; политика — см. `process_incoming_peer_hello`) |
| Классификация native/foreign по `domain_hi` | **Да** | `policy.rs`, `bridges.rs`, `transport_tick.rs` (`classify_peer_for_hs`, приоритизация) | RFC 8 §6 |
| Приоритет native при dial/backoff (очереди seed, лимиты outbound) | **Частично** | `transport_tick.rs`, `policy.rs` | RFC 8 §7 (полный gossip-weighting из §7.4 — не полный «gossip» блоков) |
| Heartbeat + `chain_tip_height` | **Да** (как наблюдаемость) | `peer_session/mod.rs` (`peer_heartbeat_wire`), `wire.rs` (`Heartbeat`) | RFC 8 §4.1 (`services`); высота не тянет блоки |
| Federation: слияние hello/hb в таблицу шардов | **Да** | `federation.rs` (`merge_remote_hello`, `merge_remote_hb`) | См. обзор шардов (не substitute chain-sync) |
| Cross-shard facts по wire | **Да** | `wire.rs` (`CrossShardFacts`), `send_*` / `merge_*` в `peer_session/mod.rs` | Вне scope «общая история блоков шарды» |
| Account views по wire | **Да** | `wire.rs` (`AccountViews`) | idem |
| Передача блоков / заголовков / inv / getheaders | **Нет** | `PeerWireMsg` исчерпывающе перечислен в `wire.rs` | — |
| Синхронизация цепочки отстающей ноды по P2P | **Нет** | Нет кода apply полученных блоков из transport (сравнить с snapshot в `lifecycle.rs`) | RFC 8 `services` + §11 |
| Общая синхронная история блоков у двух native-нод **только** за счёт P2P | **Нет** | Независимые локальные `seal`; нет репликации блоков | ADR 0001 — встроенный PoA devnet |

---

## Ответы на вопросы владельца

### 1. Две ноды одной шарды (одинаковый `domain_hi`, native peer): ведут ли общую синхронную историю блоков?

**Нет**, если иметь в виду единый консенсусный журнал, автоматически согласованный между процессами по транспорту. Каждая нода формирует блоки локально через `spawn_seal_loop` → `chain.seal` (PoA dev-консенсус, см. ADR 0001). P2P не пересылает блоки между peer’ами; совпадение истории возможно только если изначально общие genesis/снимок и одинаковый порядок внешних событий (транзакции, время), но это не механизм «общей синхронной истории» по сети в текущем коде.

### 2. Может ли отстающая нода догнать хвост блоков по transport в режиме синхронизации?

**Нет.** После handshake стабильная сессия шлёт/принимает heartbeat, cross-shard facts, account views; `chain_tip_height` в heartbeat обновляет federation-строку о наблюдаемой высоте, но **не** запускает загрузку и apply блоков с peer. Догрузка известного состояния — через **snapshot** при старте (`lifecycle.rs`, `absorb_blocks_tail`), не через peer wire.

---

## Gaps / roadmap для настоящего chain-sync

1. **Протокол wire:** новые типы сообщений (или отдельный subprotocol): запрос диапазона блоков/хедеров, ответы, инвентаризация, контроль форков — сейчас в `PeerWireMsg` отсутствуют.
2. **Apply с сети:** безопасное применение входящих блоков (валидация подписи валидатора, высота, связность) и политика выбора канонической цепи при рассогласовании — не реализовано в transport-слое.
3. **Семантика `services: sync`:** либо реализовать поведение, либо сузить объявляемые capabilities до фактических (`dial.rs` сейчас включает `"sync"` без блок-синка).
4. **RFC 8 §11:** критерии приёмки (полная identity на launch, метрики §9, gossip weighting для реальных топиков) — часть ещё в стадии «спека/частичная реализация»; для chain-sync потребуется **отдельная нормативная спецификация** или расширение RFC (fork-choice, анти-DoS на объём блоков).

---

## Verdict

**INFO** — инвентаризация зафиксирована: блок-репликации и catch-up по P2P нет; для проектирования реального chain-sync по нативным линкам логично завести/расширить RFC (**NEED-RFC** по сути следующей фазы, не как блокер текущего кода).

---

## Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260507-peer-transport-capabilities.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 8500
  confidence: low
```
