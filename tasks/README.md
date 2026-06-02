# Task tickets (orchestrator)

JSON-файлы здесь — **версионируемые задания** для субагентов: вводные, статус, ссылки на коммиты и артефакты (ревью в `docs/reviews/`). Оркестратор создаёт/обновляет файл **в начале** задачи и **после** ключевых шагов (делегирование, коммит, ревью).

## Мини-отчёты по субагентам (каждый шаг)

После вызова **`pwm-coding`**, **`pwm-testing`**, **`pwm-review`**, **`pwm-info`**, **`pwm-debug`** (или другого исполнителя) оркестратор кратко фиксирует в чате и обязательно в **`delegations[]`** тикета:

- **что** делегировали (1–2 предложения);
- **результат**: ок / расхождение с критериями / что не покрыто;
- **token_usage**: точные данные, если системный/tooling слой их дал; иначе приблизительная оценка с `source="estimate"` и `confidence`;
- **настройка промптов**: если субагент систематически ошибается в одном и том же — правка соответствующего `docs/AGENT_PROMPT_*.md`.

**Для `pwm-debug` дополнительно** (см. шаблон `_template.task.json` → блок `pwm-debug`):

- **`verbosity_focus`** — kebab-case область (`area` или `area:sub`), под которую поднимали детализацию логов; **обязательно** в каждом handoff к `pwm-debug`.
- **`instrumentation.reverted`** — `yes|no`; если `no`, заполнить **`instrumentation.receiver_if_kept`** (обычно `pwm-coding` или ссылка на follow-up тикет), и инструментация должна быть под `#[cfg(debug_assertions)]` или фиче-флагом `debug-<area>`.
- **`repro`** — `deterministic` (yes/no), `flake_rate` при флаках, конкретная команда воспроизведения.
- Длинные логи/бэктрейсы хранить в `tasks/<id>-debug-*` или `docs/debug/<id>-*` и ссылаться путём, не вставлять полные дампы в тикет.

Так проще держать слаженность команды и **не раздувать** контекст оркестратора повтором деталей, которые уже в ответе субагента.

## Делегирование (конвейер и логи)

- **`pwm-coding` / `pwm-testing` / `pwm-review`** в **линейном конвейере** по умолчанию запускать **синхронно** (`Task` / субагент без `run_in_background: true`), чтобы не терять цепочку и не полагаться на уведомления позже.
- Для **`pwm-review`** на операторских артефактах (логи, файлы в репозитории) в тикет или промпт включать **абсолютные пути** и ссылку на дефолты: `logs/{UTC-date}/pwmd-peer-{node_id}-*.log` при `cwd` = корень репо (см. `PWM_LOG_DIR`, `crates/pwmd/src/config.rs`, `logging.rs`). Требовать **чтение существующих файлов** (`Glob`/`Read`) до выводов «логов нет».

## Именование

Базовый формат: `YYYYMMDD-<slug>.json`, где `YYYYMMDD` — **дата создания тикета**, не плановая дата релиза/волны.

- Плановую дату хранить отдельно в поле `planned_for` (формат `YYYY-MM-DD`).
- `id` и имя файла должны совпадать.
- Если требуется исключение (осознанная future-дата в id), использовать override `PWM_ALLOW_FUTURE_TICKET_DATE=1` и зафиксировать причину в `notes`.

Локальный guard перед делегированием в bridge:

```bash
python scripts/_orchestrator_ticket_id_guard.py <ticket_id>
```

Guard встроен и в fallback share-скрипт `scripts/_orchestrator_share_ticket_to_bridge.py`.

**Выжимки контекста от субагента `pwm-info`:** см. **`docs/AGENT_PROMPT_info.md`** — отдельные файлы **`…-info.json`** (не путать с полноценным тикетом с `delegations[]`, если только оркестратор не связал их в `notes`).

## Поля (минимум)

| Поле | Тип | Назначение |
|------|-----|------------|
| `schema_version` | number | Сейчас `1` |
| `id` | string | Короткий id (дублирует имя файла или UUID) |
| `title` | string | Заголовок для человека |
| `status` | string | `open` \| `in_progress` \| `blocked` \| `done` \| `cancelled` |
| `mvp_checklist` | string[] | Строки или §-ссылки из `docs/MVP-checklist.md` |
| `brief` | string | Цель и критерии готовности (markdown ок) |
| `delegations` | array | Элементы: `{ "agent": "pwm-coding", "prompt_summary": "…", "result": "PASS", "artifacts": [], "tokens": { "source": "estimate", "input": null, "output": null, "total": 12000, "confidence": "low" }, "done_at": null }` |
| `commits` | string[] | Хэши локальных коммитов по этой задаче |
| `artifacts` | object | Например `{ "review_md": "docs/reviews/foo-20260418.md" }` |
| `notes` | string | Свободные заметки оркестратора |

Дополнительные поля — по необходимости; совместимость сохраняйте через `schema_version`.

## Token telemetry

- Каждый subagent должен вернуть `Participation / token estimate`.
- Оркестратор переносит эти данные в `delegations[]`.
- Если точных usage counters нет, используйте приблизительную оценку; это лучше, чем отсутствие данных.
- Эти поля нужны для последующей агрегации расходов по слайсам/ролям.

## Шаблон

См. [`_template.task.json`](_template.task.json).

## CQDS: `cq_help`, не исходники

Инструкции по **всем** MCP-инструментам CQDS в первую очередь смотреть через **`cq_help`** (актуальные схемы, `tool_ref`, батчи `requests[]`). **Не стоит** «перемалывать» исходники `mcp-tools/` или агента CQDS ради синтаксиса вызова — при сомнениях: `cq_help` → при ошибке или несоответствии ожиданию **эскалировать владельцу** (конфиг MCP, доступность Colloquium, сеть).

**Анти-зависание IDE:** не искать по workspace `**/tools/*.json`. Для статических имён действий обёртки MCP читать **`docs/mcp_index.json`**, затем **один** файл дескриптора по указанному абсолютному пути.

### Имена серверов в Cursor

В **глобальном** `mcp.json` серверы могут называться с префиксом **`user-`** (например **`user-cqds_mcp_mini`** вместо `cqds_mcp_mini`). Вызовы MCP в агенте должны использовать **фактическое** имя из конфигурации пользователя.

## Индекс кода в CQDS (после закоммиченного среза)

Если проект PWM зарегистрирован в Colloquium и MCP CQDS подключён (см. выше про префикс `user-`), после **завершённого** набора правок имеет смысл **синхронно** обновить индекс, чтобы поиск/символы совпадали с коммитом:

1. **`cq_project_ctl`** — `action`: `list_projects` (узнать `project_id`), при необходимости `select_project`.
2. **`cq_files_ctl`** — `action`: `rebuild_index`, `args`: `{ "project_id": <id>, "background": false, "timeout": 120 }` — ждёт готовность индекса в одном вызове.

Параллельно можно вызвать **`cq_project_ctl`** с `project_status` для проверки скана/кэша.

**Проблемы с CQDS** (сервер не найден, 401, таймаут индекса, пустой `list_projects`) — **эскалировать владельцу**; правки глобального MCP и окружения оркестратор не подменяет без явного запроса.

## Team bridge (VS Code worker, `cq_team_bridge_ctl`)

Делегирование coding-слайса во **внешний** long-lived worker (Copilot / VS Code). Оркестратор и воркер используют **один ключ маршрутизации**:

- **`project_id: 5`** на **каждом** вызове `cq_team_bridge_ctl` (`bridge_status`, `share_ticket`, `create_ticket`, …).
- **`tasks_root` не передавать** — MCP резолвит `<project_root>/tasks` сам; явный `tasks_root` только advanced override.
- Предпочтительно **`share_ticket`** на существующий `tasks/<slice-id>.json` после заполнения `brief`/критериев.
- Не обходить вручную `P:/opt/docker/cqds/tasks` — это не PWM-очередь.

Подробно: **`docs/AGENT_PROMPT_orchestrator.md` § Team bridge**, воркер: **`.github/agents/pwm-coding-worker.agent.md`**.

## Человеко-читаемый индекс кодовой базы

- Базовый артефакт: `docs/reviews/pwm-codebase-index-*.md`.
- Постоянный парсер: `scripts/cqds_index_digest.py` (вход: payload `cq_files_ctl#get_index`, выход: компактный Markdown).
- Полный refresh индекса делать **не регулярно**, а при росте структуры (новые модули/крэйты, крупные перестройки).
- Для обычных задач ревьюер может править существующий индекс **точечно** после сверки кода и тикета.
