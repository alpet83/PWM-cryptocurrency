# Agent prompt: Colloquium / CQDS parallel LLM track (PWM)

Скопируйте блок для документирования **параллельной** ветки разработки через чаты Colloquium-DevSpace и внешних агентов. Это **задел на оркестрируемую разработку**, не обязательный процесс для каждой задачи.

---

You may use **Colloquium-DevSpace (CQDS)** as a **parallel** channel to Cursor agents:

- CQDS exposes MCP tools for projects, files, exec in the Linux project environment, chats, etc. (see workspace rules: `cq_list_projects`, `cq_select_project`, `cq_project_ctl`, `cq_exec`, …).
- Chats inside CQDS can host **other LLM-backed “agents”** with roles shaped by prompt files.

## Role prompts on disk (CQDS / docs tree)

Base and role-specific system prompts for Colloquium-side models are expected under:

`P:\opt\docker\docs\`

Naming pattern (as used in CQDS deployment docs): **`llm_pre_prompt.md`** (base) and **`llm_pre_prompt-*.md`** (actors / roles). If your tree uses a slight spelling variant, align with the files actually present in `docs/`.

When orchestrating:

1. Keep **one authoritative task description** (issue or `docs/MVP-checklist.md` pointer) so Cursor agents and CQDS chats do not diverge.
2. Prefer CQDS for **server-side** inspection of the registered project; prefer local Cursor agents for **this git working copy** when paths differ.
3. Merge outcomes manually: CQDS chat output is not automatically the same branch/commit as local PWM unless you sync.

## Relation to repo prompts

- **Coding agent** (`docs/AGENT_PROMPT_coding.md`) — may call `cq_project_ctl` when MCP is available.
- **Review agent** (`docs/AGENT_PROMPT_review.md`) — независимый аудит без правок **продуктового** кода; может коммитить отчёт в `docs/reviews/` и поля тикета. Может читать заметки CQDS, если их вложили в запрос на ревью.

---

_End of Colloquium parallel track prompt._
