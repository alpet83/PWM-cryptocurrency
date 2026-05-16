# ADR 0004: Cleanup-chain, Bootstrap Snapshot и external anchoring

## Статус

Draft (V3 foundation boundary; prerequisite before pruning-era implementation).

## Контекст

Roadmap V3 и риски R10-R12 требуют заранее развести термины и границы:

- текущий runtime использует `Epoch Snapshot` как операционный checkpoint/load слой;
- будущий `Bootstrap Snapshot` нужен для архивного bootstrap, pruning и continuity;
- external anchoring рассматривается как дополнительная trust/minimization опция, не как обязательная часть V3.

Опорные документы:

- `docs/guide-node-storage-and-snapshot.md`
- `docs/runbook-store-snapshots.md`
- `docs/rfc/5-genesis-and-bootstrap.md`
- `docs/CONCEPT_ROADMAP.md`

## Решение

1. **Термины разделяются жестко:**
   - `Epoch Snapshot` — текущий runtime persistence/checkpoint механизм.
   - `Bootstrap Snapshot` — будущий архивный, кворумно подписанный снимок для bootstrap/pruning continuity.
2. **Cleanup-chain вводится как линейная цепочка архивных commitments** (каждый новый commitment ссылается на предыдущий).
3. **External anchoring допускается точечно** для high-value archival commitments и не становится обязательным runtime требованием V3.
4. **Policy-origin requirement:** будущий Bootstrap Snapshot должен сохранять минимально необходимое доказуемое происхождение активного policy-state (а не только итоговые агрегаты состояния).

## Почему так

- Снимает архитектурную путаницу между текущей snapshot-операционкой и будущим pruning-дизайном.
- Позволяет развивать runtime V3 без ложного обещания "cleanup-chain уже реализован".
- Готовит совместимую основу для будущего audit/forensics и long-range trust сценариев.

## Deferred implementation boundaries (не часть V3)

В V3 **не** реализуются:

- runtime snapshot pruning semantics;
- финальный формат `ArchiveCommitment` и полный wire протокол cleanup-chain;
- обязательный внешний anchor workflow для всех узлов;
- полная migration цепочка `Epoch Snapshot -> Bootstrap Snapshot`.

## Последствия

- Все изменения в текущих `Epoch Snapshot` docs/runtime должны явно помечаться как operational layer, не как pruning-final architecture.
- До начала pruning-работ нужен отдельный implementation RFC/ADR с форматом, подписями и правилами восстановления.

## Ссылки

- `docs/CONCEPT_ROADMAP.md`
- `docs/guide-node-storage-and-snapshot.md`
- `docs/runbook-store-snapshots.md`
- `docs/rfc/5-genesis-and-bootstrap.md`
- `docs/plans/mvp_v3.md`
