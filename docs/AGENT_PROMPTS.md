# Промпты для агентов (PWM)

Краткий указатель. Полные инструкции копируйте в системный промпт или в начало чата соответствующего агента.

**Субагенты Cursor (типы Task):** `pwm-coding` → промпт coding; `pwm-testing` → testing; `pwm-review` → review. **Оркестратор** ведёт план, коммиты, мини-отчёты по субагентам и файлы в [`tasks/`](../tasks/README.md). Доп. исполнители (рефакторинг, отладка) — по запросу владельца, с отдельными промптами в `docs/`.

**CQDS:** справка по MCP — **`cq_help`**; id сервера в Cursor может быть с префиксом **`user-`** (например `user-cqds_mcp_mini`). Проблемы MCP/CQDS — эскалация владельцу.

| Роль | Файл |
|------|------|
| Оркестратор (план, делегирование, `tasks/*.json`, коммиты) | [AGENT_PROMPT_orchestrator.md](AGENT_PROMPT_orchestrator.md) |
| Написание кода (`pwm-coding`) | [AGENT_PROMPT_coding.md](AGENT_PROMPT_coding.md) |
| Тесты и чеклист §3–§6 (`pwm-testing`) | [AGENT_PROMPT_testing.md](AGENT_PROMPT_testing.md) |
| Ревью — только отчёт (`pwm-review`) | [AGENT_PROMPT_review.md](AGENT_PROMPT_review.md) |
| Colloquium / CQDS, параллельные LLM | [AGENT_PROMPT_colloquium_parallel.md](AGENT_PROMPT_colloquium_parallel.md) |
