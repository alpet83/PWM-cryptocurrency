# ADR index (`docs/adr`)

Этот каталог хранит архитектурные решения PWM.  
Изначальный фокус блока был на V3 foundation: зафиксировать направления без преждевременного включения V4/V5/V7 runtime scope.  
MVP v4 policy runtime уже закрыт; текущий статус версий смотрите в `docs/CONCEPT_ROADMAP.md` и `docs/plans/mvp_v4.md`.  
**[0005-policy-deferred-activation.md](0005-policy-deferred-activation.md)** принят как V5 contract для режима **`Deferred { activate_at_height }`** по высоте цепи, **без** address flags и **без** delayed-transfer. ADR 0006/0007 фиксируют V5 spec-only границы для address flags и domain lease governance.

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
| [0005-policy-deferred-activation.md](0005-policy-deferred-activation.md) | Третий режим активации политики `Deferred` по высоте цепи, без flags/conservation | Accepted |
| [0006-address-flags-and-nondisableable-profiles.md](0006-address-flags-and-nondisableable-profiles.md) | Address flags и non-disableable profiles; runtime enforcement → [ADR 0009](0009-address-flags-runtime-enforcement.md) | Accepted |
| [0007-domain-lease-parameter-governance.md](0007-domain-lease-parameter-governance.md) | Governance параметров аренды `domain_lo > 0`, No Burn Principle | Accepted (spec-only) |
| [0008-snapshot-genesis-anchor-light.md](0008-snapshot-genesis-anchor-light.md) | Genesis anchor в Epoch Snapshot: лёгкие проверки, одна подпись, block@1 preflight, миграция | Accepted (impl pending) |
| [0009-address-flags-runtime-enforcement.md](0009-address-flags-runtime-enforcement.md) | Runtime enforcement ADR 0006: cosign non-disableable, conservation queue | Accepted (V6) |
| [0010-slashing-evidence-stubs.md](0010-slashing-evidence-stubs.md) | Slashing evidence append-only stubs; no seizure | Accepted (V6) |
| [0011-policy-activation-target.md](0011-policy-activation-target.md) | `ActivatePolicy.activation_target`, fee-free activation, emergency evac | Accepted (V6) |
| [0012-emergency-stake-evacuation.md](0012-emergency-stake-evacuation.md) | Emergency activation evacuates `staked_pwm_raw` to rescue (extends 0011) | Accepted (V7 impl) |

## V6 RFC addenda

Нормативные расширения RFC для MVP v6 (spec-freeze V6-1): каталог [docs/rfc/addenda/](../rfc/addenda/) (`v6-rfc4-*` … `v6-rfc16-*`, `v6-rfc9-*`, `v6-rfc6-*`, `v6-rfc10-*`, `v6-rfc15-*`).

## Важно про границы V3

Документы ADR серии `0002..0004` фиксируют направление и интерфейсные границы.  
Они не означают автоматическую немедленную реализацию всего V5/V7 scope в текущем runtime; V4 трек закрыт отдельно по `docs/plans/mvp_v4.md`, а V5 scope ведётся в `docs/plans/mvp_v5.md`.
