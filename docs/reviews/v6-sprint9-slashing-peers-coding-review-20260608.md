# Review: V6-9 slashing evidence stubs + peer sync scoring

**Slice:** `20260608-v6-sprint9-slashing-peers-coding`  
**Branch:** `v6/20260608-v6-sprint9-slashing-peers-coding`  
**Coding commit:** `7086434` (`feat(v6-9): evidence stubs + operator-local peer sync scoring`)  
**Base:** `5f61316` (main)  
**Reviewer:** pwm-review  
**Date:** 2026-06-08

## 1. Scope recap

Слайс закрывает **MVP v6 Sprint V6-9** ([`docs/plans/mvp_v6.md`](../plans/mvp_v6.md)): append-only evidence stubs в consensus state (ADR 0010) и operator-local peer sync scoring для bias выбора sync/backfill пиров (v6-rfc15). Без consensus `peer_score_table`, без seizure/ejection.

**Заявленный diff (14 файлов, +310/−11):**

| Область | Изменения |
|---------|-----------|
| `pwm-core` | `State::evidence_record_id`, `append_evidence`, `Chain::append_unavailable_proposer_evidence`; тесты `evidence_duplicate_reject`, `evidence_append_no_seizure` |
| `pwmd` | `transport/score.rs` (`PeerSyncScoreCache`, дельты RFC15), поле `HandshakeState::peer_scores`, хуки в sync/seed/hello, `prioritize_peer_candidates_scored` + `score_sort` в backfill/transport_tick/dev_peers |

**Норматив:** ADR 0010, v6-rfc15, acceptance criteria в `tasks/20260608-v6-sprint9-slashing-peers-coding.json`.

## 2. Requirements fit

| Критерий | Статус | Комментарий |
|----------|--------|-------------|
| Детерминированный `record_id`, duplicate → `TxError::EvidenceDuplicate` / `E_EVIDENCE_DUPLICATE` | **PASS** | `evidence_record_id` (blake3 + domain tag + bincode type); linear dedup; wire mapping уже в `reject_wire.rs` |
| Evidence не меняет balances / stake / active set | **PASS** | `evidence_append_no_seizure` снимает снимки до/после |
| ≥1 путь evidence + тесты | **PASS** | Прямой `State::append_evidence` (в т.ч. `UnavailableProposer` в duplicate-тесте); `Chain::append_unavailable_proposer_evidence` — тонкая обёртка без вызова из `seal` (см. nits) |
| Operator-local score, дельты RFC15 | **PASS** | +1/+1/−2/−5/−10 в `PeerScoreEvent::delta` |
| Selection biased by score, детерминированный tie-break | **PASS** | `prioritize_peer_candidates_scored` (rank → score desc → last_seen → node_id); `score_sort` (score desc → lex peer_id) |
| Unit tests `evidence_*`, `peer_score_*` | **PASS** | 5 тестов; worker PASS по конвейеру |
| RPC `/v1/peers/scores` | **Deferred (OK)** | Тикет допускает operator-only; scores в `HandshakeState`, не экспонированы в API — соответствует scope «если не раздувает API freeze» |
| Consensus `GenCfg.peer_score_table` | **Out of scope** | Явно не добавлено (RFC §3 deferred) |

**Частичные пробелы (не блокеры):** seal-internal hook для `UnavailableProposer` не подключён к `Chain::seal`; публичный `EvidenceTx` не включён — ADR 0010 допускает для V6-9 только internal hook *или* disabled wire.

## 3. Style and module shape

- **Модульность:** новая логика scoring вынесена в `transport/score.rs` с `//!` banner; интеграция точечная в существующие transport hooks — хороший срез.
- **Именование:** `python scripts/check_entity_name_segments.py` по всем 14 touched paths — **violations: []** (prod ≤4, test ≤5).
- **Символы:** `append_unavailable_proposer_evidence` (4 сегмента), `prioritize_peer_candidates_scored` (4), `evidence_record_id` (3) — в политике.
- **`prioritize_peer_candidates`:** оставлен как thin shim с `#[allow(dead_code)]` → делегирует в scored-вариант с пустым cache; приемлемо для обратной совместимости тестов.
- **Комментарии:** English `//!` на `score.rs`; остальные правки без раздувания `main.rs`/façade.
- **Protocol version:** `PWM_PROTOCOL_VERSION` не менялся; peer wire payloads не расширялись — **no wire compatibility impact**.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). Изменения касаются in-memory operator cache и consensus `State::append_evidence`; `PeerSyncScore` использует `Serialize` только локально, не в `PeerWireMsg`. Существующие snapshot `u128` поля (conservation) не тронуты.

## 4. Safety

- **Evidence:** append-only `Vec`, без экономических side-effects — соответствует ADR non-effects. Duplicate guard — O(n) scan; для V6 stub приемлемо.
- **`expect` в `evidence_record_id`:** bincode сериализация `EvidenceType` — closed enum; паника маловероятна, не hot-path сеть.
- **Peer scores:** in-memory `HashMap` по `node_id` — тот же класс риска, что и `hs.peers`; без gossip/consensus не влияет на BFT safety.
- **Score saturation:** `i64::saturating_add` — нет wrap-around сюрпризов.
- **Trust boundary:** scoring на observables оператора (timeout, fork mismatch, bridge refusal) — не consensus input.

## 5. Tests

**Покрыто:**

- `evidence_duplicate_reject` — повторный `record_id` → `EvidenceDuplicate`, log len=1.
- `evidence_append_no_seizure` — balances, stakes, `active_validator_indices` неизменны после `InvalidAttestation` + reporter.
- `peer_score_deterministic_deltas` — полная цепочка дельт RFC15 и счётчики событий.
- `peer_score_tie_order` — `score_sort` при равных score → lex `peer_id`.
- `peer_score_select_order` — native peers с разным score → `native-b` (score 2) перед `native-a` (1) перед `native-c` (0).

**Пробелы (низкий приоритет):**

- Нет теста вызова `Chain::append_unavailable_proposer_evidence` (обёртка тривиальна).
- Нет интеграционного теста «sync event → score bump → reorder» end-to-end (unit-level достаточно для slice).
- Локальный `cargo test` в среде ревьюера упал на `dlltool.exe` (Windows toolchain); опираемся на worker PASS (`evidence_*`, `peer_score_*`, `check --workspace`).

## 6. Nits (prioritized)

1. **Medium — bridge commitment mismatch без score penalty:** в `incoming_hello.rs` `PeerScoreEvent::BridgeTrustRefusal` применяется только при `bridge_commitment: None`, но **не** при `got != expected` (вторая ветка refusal). RFC15 таблица одна на «Bridge trust refusal contributor −10» — стоит симметрично вызывать `apply` в обеих ветках.
2. **Low — seal hook не подключён:** `append_unavailable_proposer_evidence` экспортирован, но `Chain::seal` не вызывает; тикет notes ссылается на V6-4b failover — ок для V6-9, но стоит завести follow-up или однострочный комментарий в ADR/ticket при merge.
3. **Low — RFC vs impl drift:** v6-rfc15 описывает `score: i32` и `last_updated_unix`; реализация `i64` без timestamp. Для operator-local cache не критично; при следующем касании RFC — выровнять текст или поля.
4. **Low — двойной `handshake_read_traced` в `handlers_backfill`:** блок score_sort и блок genesis_hash читают HS отдельно — микро-неэффективность, не корректность.

## 7. Verdict

**Approve with nits (PASS_WITH_NITS).** Ядро acceptance criteria выполнено: evidence append/dedup/non-seizure, operator-local scoring с RFC15 дельтами, score-biased peer order с детерминированным tie-break. Блокеров по контракту, naming policy и wire/u128 нет. Единственный содержательный nit — неполное покрытие bridge-trust refusal в scoring (mismatch path).

## 8. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/v6-sprint9-slashing-peers-coding-review-20260608.md
token_usage:
  source: estimate
  input: 18000
  output: 3500
  total: 21500
  confidence: medium
```
