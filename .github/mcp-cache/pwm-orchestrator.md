# pwm-orchestrator MCP cache

Use this file as the first help source for routine `pwm-orchestrator` work. Do not query `cq_help` unless this file is missing the needed detail, appears stale, or you need live data such as `cq_help#core_status`.

## Startup and routing

- Prefer `project_id` as the base routing key for bridge actions.
- Resolve `project_id` via `cq_project_ctl#list_projects` if the parent context did not provide it.
- Use `tasks_root` only as an advanced override.
- Omit `worker_lane` unless lane-specific routing is intentionally required.

## Main MCP tools for orchestration

### `cq_team_bridge_ctl`

- `bridge_status`: inspect queue counts, free workers by class, free workers by model, active waiters by class, active waiters by model.
- `create_ticket`: create a queued ticket under bridge control.
- `share_ticket`: move an existing task file under bridge control.
- `requeue_orphans`: recover orphaned in-progress tickets when needed.
- `rehydrate_prompt`: manage rehydrate-required state.
- `reassign_worker`: worker-side recovery path after restart; orchestrator should understand it but not use it as normal task stealing.

### `cq_project_ctl`

- `list_projects`, `select_project`, `project_status`, `fetch_result`.

### `cq_files_ctl`

- `start_grep` plus `fetch_result` for task preparation, evidence gathering, and targeted code/doc lookup.

## Orchestrator defaults

- Before spawning a new worker, inspect `bridge_status`.
- Prefer reusing available workers when `bridge_status` shows matching capacity.
- Use `free_workers_by_model` and `active_waiters_by_model` when selecting workers by reported LLM.
- If a ticket is created manually outside bridge queue flow, follow with `share_ticket`.

## Self-refresh rule

If `pwm-orchestrator` had to query live `cq_help` for an important missing contract, merge the missing operational detail back into this file.