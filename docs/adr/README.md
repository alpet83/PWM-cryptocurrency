# ADR index (`docs/adr`)

Этот каталог хранит архитектурные решения PWM.  
Фокус V3: зафиксировать foundation-направления без преждевременного включения V4/V5/V7 runtime scope.

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

## Важно про границы V3

Документы ADR серии `0002..0004` фиксируют направление и интерфейсные границы.  
Они не означают немедленную реализацию V4/V5/V7-функций в текущем V3 runtime.
