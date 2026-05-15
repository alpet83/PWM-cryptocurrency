# Sprint V2-9 Slice B — wave notes (2026-05-10)

## Что добавлено

- `cluster_2of2_gate_ok` переведён на полноценный wire E2E в `crates/pwmd/src/transport/tests/production.rs`:
  - proposer `node-a` отправляет `ClusterPropose` по TCP в inbound attester `node-b` (role-gate как в production),
  - proposer локально зеркалит исходящий propose через helper `record_cluster_propose_originated(...)` (без ручной правки `cluster_attest` полей в тесте),
  - attester `node-b` поднимает отдельную inbound-сессию в proposer `node-a` и отправляет валидный `ClusterAttest`,
  - подпись строится тем же 5-строчным binding (`height/round/vote/candidate_hash/candidate_ref`), что и в проверке `route_cluster_stub`,
  - gate открывается только после реального принятия attestation по wire.
- Добавлен негатив `cluster_timeout_no_seal`:
  - proposal есть, attest отсутствует,
  - после `attest_timeout_ms` gate остаётся закрыт (`no-seal`, путь `quorum_timeout`).
- Добавлен fault-inject `cluster_bind_mismatch_no_seal`:
  - отправляется attest с несовпадающим `vote_object`,
  - attest не засчитывается, gate остаётся закрыт (`no-seal`, путь `binding_mismatch/quorum_pending`).

## Как запускать

```bash
cargo test -p pwmd cluster_2of2_gate_ok
cargo test -p pwmd cluster_timeout_no_seal
cargo test -p pwmd cluster_bind_mismatch_no_seal
cargo test -p pwmd cluster_partition
```

Полный регресс по crate:

```bash
cargo test -p pwmd
```

## Наблюдаемость причин no-seal

- Для timeout-сценария: `seal_suppressed_by_cluster reason=quorum_timeout`.
- Для отсутствия attest до таймаута: `seal_suppressed_by_cluster reason=quorum_pending`.
- Для несовпадения binding на attest: `cluster attest dropped ... reason=binding_mismatch` и последующий `no-seal`.
