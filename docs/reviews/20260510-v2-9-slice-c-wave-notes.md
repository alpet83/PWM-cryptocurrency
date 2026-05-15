# Sprint V2-9 Slice C — wave notes (2026-05-10)

## Что добавлено в Slice C (TCP gate, wire-only)

- В `crates/pwmd/src/transport/tests/production.rs` добавлен позитив `cluster_2of3_gate_wire`:
  - proposer `node-a`, attesters `node-b` и `node-c`, `n=3`, `k=2`;
  - `ClusterPropose` уходит по TCP на attester inbound;
  - proposer локально зеркалит propose через `record_cluster_propose_originated(...)`;
  - два независимых inbound-сеанса (`node-b -> node-a`, `node-c -> node-a`) отправляют валидные `ClusterAttest`;
  - `run_cluster_gate(&app_a)` возвращает `true` только после 2 валидных ACK.
- Добавлен негатив `cluster_2of3_one_ack_stuck`:
  - тот же состав кластера (`n=3`, `k=2`);
  - приходит только один валидный attest;
  - `run_cluster_gate(&app_a)` остаётся `false` (без silent bypass).

## Follower / convergence

- Ловушка **ложного** `sync_tip_divergence` при **peer-behind** (follower на genesis, source уже выше) устранена в `sync_live::on_tip` — см. root cause и fix ниже. Регрессия: `tip_behind_no_divergence`.
- **Двухузловой** multi-app TCP soak (cluster-enabled source + cluster-off follower в одном тесте) — **landed**: `same_shard_follower_tcp_tip` в `crates/pwmd/src/tests/transport_peer.rs`. Отдельный сценарий «три узла кластер 2-of-3 + четвёртый follower в одном бинарном тесте» по-прежнему опционален (длительность/флейки) и не требуется для текущего чеклиста V2-9 Slice C.
- Root cause (устарело как открытый дефект, оставлено как история):
  в `sync_live::on_tip` lag считался как `head_h.saturating_sub(local_h)`, и кейс `head_h < local_h`
  попадал в ветку `lag == 0`, где сравнивались tip-хэши разных высот.
  Это давало ложный `TipDivergence` и disconnect вместо benign peer-behind состояния.
- Исправление: после обновления peer sync state в `on_tip` добавлен ранний выход при `head_h < local_h`
  (`Ok(None)`), а `lag` теперь вычисляется только для `head_h >= local_h`.
  Для equal/ahead путей (`lag == 0` и `lag > 0`) поведение не менялось.
- Ожидаемый эффект: steady bidirectional сессии, где follower остаётся на genesis,
  а source уже анонсирует продвинутый tip, больше не должны давать false-divergence disconnect.
- Multi-node TCP soak для same-shard follower теперь **landed**: `same_shard_follower_tcp_tip` (cluster-enabled source vs non-cluster follower, bounded converge по `tip_h` + `tip_hash`).
- В рамках этого слайса усилен детерминированный same-shard sync путь в
  `crates/pwmd/src/transport/peer_session/mod.rs`:
  - `blk_fetch_apply_ok` теперь явно фиксирует non-cluster контур (`cluster_cfg.enabled = false`);
  - после sync-apply проверяется не только `tip_h`, но и точное совпадение `tip hash` с удалённым блоком.
- Запуск текущего follower-coverage (partial):

```bash
cargo test -p pwmd transport::peer_session::tests::blk_fetch_apply_ok
cargo test -p pwmd transport::peer_session::tests::tip_behind_no_divergence
```

## Как запускать Slice C tests

```bash
cargo test -p pwmd cluster_2of3_gate_wire
cargo test -p pwmd cluster_2of3_one_ack_stuck
cargo test -p pwmd same_shard_follower_tcp_tip --lib
```
