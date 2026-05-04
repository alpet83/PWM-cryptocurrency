# Sprint 15 kickoff note

Sprint 15 стартует как consistency/hardening спринт с ограниченным scope: сначала прозрачный и детерминированный cross-shard path, затем guardrails genesis/hash, после этого optional DB snapshot prototype без риска для JSON baseline.

Ключевой порядок исполнения:
1. `S15-S0-ARCH-FREEZE` -> зафиксировать семантику балансов и readiness контракт.
2. `S15-S1..S15-S3` -> закрыть критичные инварианты протокола/операторской диагностики.
3. `S15-S4..S15-S6` -> добавить backend abstraction, ClickHouse prototype и parity e2e между backend path.
4. `S15-S7-CLOSEOUT` -> финальные гейты и решение ready/carry-over/blocked.

Правило принятия: любой block-level finding по readiness/genesis/replay переводит спринт в `carry-over` до явного remediation.

Handoff: `S15-S0-ARCH-FREEZE` завершён; governing contract зафиксирован в `docs/reviews/sprint-15-s0-architecture-freeze.md`. Стартовый рабочий слайс: `S15-S1-XSHARD-HARDEN`.
