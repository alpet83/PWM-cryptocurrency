# Sprint 15 S3.9 Review

## Verdict
`block`

## Blocker
Snapshot-failure rollback не откатывает новый `cross_shard` ledger.

`CommitBak` откатывает chain/state/roaming/flow, но не ledger. Если export/import fact записан до snapshot save, а save падает, chain/state откатываются, но `/v1/status` может показывать phantom export/import fact.

## High Risk
Peer-synced facts смешиваются с local committed facts в одном `cross_shard_summary` без явной provenance/semantics границы.

Для MVP допустимо хранить trusted peer observations, но status/log должны явно различать:
- локально зафиксированные факты;
- факты, полученные от trusted peers.

## Required Remediation
1. Добавить `cross_shard` в rollback backup/restore paths.
2. Добавить тест: snapshot-save failure не оставляет phantom export/import facts.
3. Разделить или явно маркировать local vs trusted peer facts в status/log summary.
4. Добавить тест на untrusted ignored + trusted peer fact visible with proper semantics.

## Remediation Result
`approve with nits`

Закрыто:
- `cross_shard` включён в rollback backup/restore.
- Добавлен regression test на export snapshot-failure rollback без phantom ledger facts.
- `/v1/status` и log summary явно маркируют scope как `local_plus_trusted_peer_observations`.
- Добавлен `trusted_peer_observed_count` и тест на trusted peer observation labeling.

Остаточный nit:
- full live two-node peer e2e лучше прогнать перед Sprint 15 closeout, когда лимиты `pwm-testing` снова доступны.
