# Промпты для агентов (PWM)

Краткий указатель. Полные инструкции копируйте в системный промпт или в начало чата соответствующего агента.

**Субагенты Cursor (типы Task):** `pwm-coding` → промпт coding; `pwm-testing` → testing; `pwm-review` → review; **`pwm-info`** → [AGENT_PROMPT_info.md](AGENT_PROMPT_info.md) (контекст/грепы → `tasks/*-info.json`). **Имена символов:** см. §Style в `AGENT_PROMPT_coding.md` и §Naming в `AGENT_PROMPT_testing.md` (≤5 слов в сегментах `snake_case`, короткие токены, docstrings). **Модули:** держать микро-модульную раскладку (slice **O**), не раздувать `main`/`lib`/фасады; короткие англ. **`//!`** в затронутых `*.rs` — см. **`AGENT_PROMPT_coding.md`** §Micro-modular layout. **Ревью:** при необходимости — скрипты **`scripts/_review_*.{py,ps1}`** — см. **`AGENT_PROMPT_review.md`**. **Оркестратор** ведёт план, коммиты, мини-отчёты по субагентам, примерный/точный `token_usage` в `tasks/*.json` и файлы в [`tasks/`](../tasks/README.md). По умолчанию шаги конвейера запускать **синхронно** (без фона), чтобы не обрывать цепочку; фон — только для **параллельных** ног (в т.ч. разумно для их **первых** запусков одновременно), затем синхронное ожидание и следующий шаг; линейный конвейер — без фона (см. `AGENT_PROMPT_orchestrator.md` §4.1). Доп. исполнители (рефакторинг, отладка) — по запросу владельца, с отдельными промптами в `docs/`.

**CQDS:** справка по MCP — **`cq_help`**; id сервера в Cursor может быть с префиксом **`user-`** (например `user-cqds_mcp_mini`). Перед CQDS-вызовами использовать skill **`colloquium-cqds-mcp`**. Исходники агента (`mcp-tools/`) и произвольный обход `mcp.json` — не для синтаксиса. Чтобы не зависать на поиске `tools/*.json`, статические имена действий смотреть через **`docs/mcp_index.json`** → точечное **`Read`** одного JSON из указанного каталога. Проблемы MCP/CQDS — эскалация владельцу.

| Роль | Файл |
|------|------|
| Оркестратор (план, делегирование, `tasks/*.json`, коммиты) | [AGENT_PROMPT_orchestrator.md](AGENT_PROMPT_orchestrator.md) |
| Написание кода (`pwm-coding`) | [AGENT_PROMPT_coding.md](AGENT_PROMPT_coding.md) |
| Тесты и чеклист §3–§6 (`pwm-testing`) | [AGENT_PROMPT_testing.md](AGENT_PROMPT_testing.md) |
| Ревью — независимый аудит; коммиты **только** отчёт `docs/reviews/*` + при необходимости `tasks/*.json` (`pwm-review`) | [AGENT_PROMPT_review.md](AGENT_PROMPT_review.md) |
| Оптимизационный аудит (копипаста, >500 LOC, зависимости) (`pwm-optimus`) | Использует встроенный промпт в `.cursor/agents/pwm-optimus.md` |
| Контекст для исследования (`pwm-info`, **Kimi-K2.5**) | [AGENT_PROMPT_info.md](AGENT_PROMPT_info.md); артефакт **`tasks/<date>-…-<slug>-info.json`**; поиск через **`cq_files_ctl`/`start_grep`**, **`project_id: 5`**, или **`rg`** |
| Colloquium / CQDS, параллельные LLM | [AGENT_PROMPT_colloquium_parallel.md](AGENT_PROMPT_colloquium_parallel.md) |
