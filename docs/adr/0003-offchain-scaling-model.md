# ADR 0003: Offchain Scaling Model (centralized batch baseline)

## Статус

Draft (V3 foundation, direction fixed; runtime expansion deferred).

## Контекст

Roadmap V3 отмечает риск R8 (выбор между centralized offchain batch и payment channels) и требует зафиксировать направление до расширения API/runtime.

Текущее состояние:

- `docs/OFFCHAIN_STUB.md` и `docs/offchain-batch.md` описывают MVP-заглушку batch root + provider signature;
- полноценный bridge/API-пайплайн и proof-verification в runtime пока отсутствуют;
- V3 не должен внедрять production offchain API.

## Решение

1. **Базовый вектор — centralized batch processing** как первичный scaling path для V5.
2. **Клиентская проверяемость обязательна:** провайдер публикует batch commitment и данные, достаточные для верификации включения.
3. **On-chain surface минимальный:** в runtime принимаются компактные commitments вместо массового потока микроопераций.
4. **Payment channels не отвергаются навсегда,** но считаются альтернативным/поздним направлением после стабилизации batch-модели.

## Почему так

- Быстрее путь к demo-ready интеграциям для внешних сервисов.
- Операционно проще для раннего devnet и первых партнерских PoC.
- Меньше протокольной сложности на этапе foundation freeze.

## Deferred implementation boundaries (не часть V3)

В V3 **не** включаются:

- production endpoints для offchain settlement;
- финальный verifier pipeline на стороне `pwmd`;
- channel-based lifecycle (open/update/close/dispute) как обязательный runtime;
- экономические санкции/slashing за некорректные offchain batches.

## Последствия

- Понадобится отдельный RFC с canonical leaf schema, proof format и error-contract до production rollout.
- `docs/api-v1.md` должен помечать offchain/operator surface как нестабильный до отдельного freeze.

## Ссылки

- `docs/CONCEPT_ROADMAP.md`
- `docs/OFFCHAIN_STUB.md`
- `docs/offchain-batch.md`
- `docs/plans/mvp_v3.md`
