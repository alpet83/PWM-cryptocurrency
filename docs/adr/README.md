# ADR index (`docs/adr`)

Этот каталог хранит архитектурные решения PWM.  
Изначальный фокус блока был на V3 foundation: зафиксировать направления без преждевременного включения V4/V5/V7 runtime scope.  
MVP v4 policy runtime уже закрыт; текущий статус версий смотрите в `docs/CONCEPT_ROADMAP.md` и `docs/plans/mvp_v4.md`.  
Черновик **[0005-policy-deferred-activation.md](0005-policy-deferred-activation.md)** — планируемое расширение **режима активации `Deferred`** (по высоте цепи), **без** address flags и **без** delayed-transfer; реализация — отдельный V4.x тикет после принятия ADR/RFC-согласования.

## Формат ADR

Рекомендуемая структура:

1. Статус
2. Контекст
3. Решение
4. Границы реализации (что откладывается)
5. Последствия
6. Ссылки

## Индекс

| ADR | Тема | Статус |
|---|---|---|
| [0001-consensus-and-node-stack.md](0001-consensus-and-node-stack.md) | Стек консенсуса и ноды (MVP) | Принято |
| [0002-ipv4-claiming-design.md](0002-ipv4-claiming-design.md) | Архитектурная модель IPv4 Claiming | Draft (V3 foundation) |
| [0003-offchain-scaling-model.md](0003-offchain-scaling-model.md) | Offchain scaling model (centralized batch baseline) | Draft (V3 foundation) |
| [0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md](0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md) | Cleanup-chain, Bootstrap Snapshot и external anchoring | Draft (V3 foundation) |
| [0005-policy-deferred-activation.md](0005-policy-deferred-activation.md) | Третий режим активации политики `Deferred` по высоте цепи (минимальный V4.x, без flags/conservation) | Draft |

## Важно про границы V3

Документы ADR серии `0002..0004` фиксируют направление и интерфейсные границы.  
Они не означают автоматическую немедленную реализацию всего V5/V7 scope в текущем runtime; V4 трек закрыт отдельно по `docs/plans/mvp_v4.md`.
