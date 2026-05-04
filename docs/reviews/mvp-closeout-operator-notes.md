# MVP multi-sprint closeout — operator notes (2026-05-04)

Краткие положения для финального отчёта; не дублирует требования к продукту.

## Colloquium / CQDS / MCP

Стабильность **индекса кода** и **MCP Colloquium** на стороне dev-окружения **не** являются функциональными инвариантами `pwmd` / `pwm-cli` / `pwm-tui`. Сбои `rebuild_index` или сессии MCP влияют на рабочий процесс агентов, а не на консенсус ноды. В артефактах closeout указывайте эту границу, если обсуждается «готовность инфраструктуры разработки».

## См. также

- [MVP-checklist.md](../MVP-checklist.md)
- [tester-guide-cli-tui-scenarios.md](../tester-guide-cli-tui-scenarios.md) — негативы и RPC-параллели
