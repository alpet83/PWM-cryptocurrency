# pwm-info MCP cache

Use this file as the first help source for routine `pwm-info` or research-style worker tasks. Do not query `cq_help` unless this file is missing the needed detail, appears stale, or you need live data such as `cq_help#core_status`.

## Startup and routing

- Prefer `project_id` in CQDS and bridge calls.
- For a fresh project session: `list_projects` -> `select_project` when needed -> `project_status` if status or index freshness is unclear.

## Main MCP tools for info work

### `cq_files_ctl`

- `start_grep` plus `fetch_result` for targeted search across code, docs, or logs.
- `read_file` for exact file evidence.
- `grep_logs` for operational/log analysis.
- `get_index` and `grep_entity` for symbol-aware navigation when cheaper than raw grep.

### `cq_project_ctl`

- `project_status` for health and index state.
- `query_db` only when read-only SQL is the cheapest way to answer the question.

### `cq_exec_ctl`

- `exec` for bounded project-side inspection commands.

### `cq_team_bridge_ctl`

- If running as a bridge worker: `worker_status`, `wait_ticket`, progress/result lifecycle, and `unregister_worker` on bounded exit.

## Info defaults

- Prefer read-only inspection tools.
- Prefer one targeted search or file read over live `cq_help`.
- Use live `cq_help` only for missing action contracts or live status endpoints.

## Self-refresh rule

If `pwm-info` had to query live `cq_help` for an important missing contract, merge the missing operational detail back into this file.