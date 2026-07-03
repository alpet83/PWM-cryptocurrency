# pwm-coding MCP cache

**Policy lives in `docs/AGENT_PROMPT_coding.md` only.** This file is a **syntax cheat sheet** for CQDS / bridge calls — not a second coding prompt.

Use this file first for routine MCP payload shapes. Bridge workers: read **`AGENT_PROMPT_coding.md`** before using these tools.

Core defaults:

- `project_id = 5`
- `agent_name = pwm-coding`
- prefer `project_id`, not `tasks_root`
- bounded bridge loop: `worker_status` -> up to 20 `wait_ticket(timeout_sec=240)` -> `unregister_worker`
- use `wait_ticket` as the primary idle-loop primitive; it returns immediately when a matching ticket appears on disk
- use `poll_ticket` only for diagnostics, one-shot status checks, or recovery checks requested by the parent
- report `llm_model`, `llm_provider`, `llm_model_source` in `worker_status` when reliably known

Common bridge actions:

- `wait_ticket`
- `ack_ticket`
- `heartbeat_ticket`
- `update_ticket_progress`
- `submit_ticket_result`
- `reassign_worker` only after restart-style lease mismatch when old `lease_owner_pid` is gone
- `bridge_status`

Common CQDS tools:

- `cq_project_ctl#list_projects`
- `cq_project_ctl#select_project`
- `cq_project_ctl#project_status`
- `cq_files_ctl#start_grep` then `cq_project_ctl#fetch_result`
- `cq_files_ctl#read_file`
- `cq_files_ctl#replace` — server-side Colloquium tree only; not default for local `$PROJECT_ROOT` edits when `user-gitbash` is up
- `cq_exec_ctl#exec`
- `cq_exec_ctl#spawn_script`

### File edits — local checkout (`user-gitbash`)

**Default for content changes under `P:/opt/docker/pwm-protocol`** (not IDE `Write` / `StrReplace`):

1. **`git_mcp_script`** — `recipe_id` + `inputs`: `editor_single_file`, `editor_write_lint_commit`, `editor_write_lint_undo`.
2. **`git_write_file`** + **`git_write_undo`** — lone write; keep `write_id` for rollback.

Paths: forward slashes in JSON. Inline script: single-quoted strings; `OKResult`/`FailedResult` returns. Normative: `.cqds/prompts/15-file-editing-gitbash.md`, `.cqds/prompts/65-mcp-script.md`.

Fallback rule:

- use `cq_help` only if this file is missing a needed contract, runtime behavior appears newer, or you need `cq_help#core_status`

## Cached payload templates (used in V5-4 slice flow)

Use these as copy/paste base to reduce `cq_help` calls.

### 1) Register worker snapshot

Tool: `cq_team_bridge_ctl`

```json
{
	"action": "worker_status",
	"args": {
		"project_id": 5,
		"agent_name": "pwm-coding",
		"worker_class": "coding",
		"status": "idle",
		"llm_model": "GPT-5.3-Codex",
		"llm_provider": "OpenAI",
		"rehydrate_supported": true
	}
}
```

### 2) Long-poll and claim ticket

Tool: `cq_team_bridge_ctl`

```json
{
	"action": "wait_ticket",
	"args": {
		"project_id": 5,
		"agent_name": "pwm-coding",
		"worker_class": "coding",
		"timeout_sec": 180
	}
}
```

### 3) Ack claimed ticket

Tool: `cq_team_bridge_ctl`

```json
{
	"action": "ack_ticket",
	"args": {
		"project_id": 5,
		"agent_name": "pwm-coding",
		"ticket_id": "<ticket_id>"
	}
}
```

### 4) Heartbeat during execution

Tool: `cq_team_bridge_ctl`

```json
{
	"action": "heartbeat_ticket",
	"args": {
		"project_id": 5,
		"agent_name": "pwm-coding",
		"ticket_id": "<ticket_id>"
	}
}
```

### 5) Progress updates

Tool: `cq_team_bridge_ctl`

```json
{
	"action": "update_ticket_progress",
	"args": {
		"project_id": 5,
		"agent_name": "pwm-coding",
		"ticket_id": "<ticket_id>",
		"progress": 65,
		"progress_message": "milestone message"
	}
}
```

### 6) Submit result

Tool: `cq_team_bridge_ctl`

```json
{
	"action": "submit_ticket_result",
	"args": {
		"project_id": 5,
		"agent_name": "pwm-coding",
		"ticket_id": "<ticket_id>",
		"outcome": "done",
		"result_summary": "PASS short summary",
		"result_payload": {
			"ticket_id": "<ticket_id>",
			"result": "PASS",
			"files_touched": ["path/a.rs", "path/b.rs"],
			"commands_run": [
				"cargo fmt --check",
				"python scripts/check_entity_name_segments.py <paths>",
				"cargo check --workspace"
			]
		}
	}
}
```

### 7) Unregister worker when loop ends

Tool: `cq_team_bridge_ctl`

```json
{
	"action": "unregister_worker",
	"args": {
		"project_id": 5,
		"agent_name": "pwm-coding"
	}
}
```

### 8) Companion bridge/ticket diagnostics (`get_status`)

Tool: `cq_companion_ctl`

**RPC-first:** MCP вызывает `POST /control/get_status` на companion (`companion_api_url`). Ответ живого инстанса: `instance`, `workers[].availability` (`free`|`busy`|`blocked`), `dialog_phase`, `subagents`, `bridge_status`, `queue_tickets[]`.

Для **нескольких companion** (например Windows + WSL2) указывайте `companion_api_url` явно.

```json
{
	"action": "get_status",
	"args": {
		"project_id": 5,
		"worker_class": "coding",
		"companion_api_url": "http://127.0.0.1:8099"
	}
}
```

При недоступном RPC — fallback `source: filesystem_fallback` (только дерево `team-tasks/`, без runtime диалогов).

Опционально: `worker_id`, `include_workers: false`.

### 9) Delegate edit to local `pwm_editor` (companion subagent)

Tool: `cq_companion_ctl`

Prerequisites: companion running for project 5; `[worker.pwm_editor]` bootstrapped (`bootstrap_probe` ok).

Policy:

- For edits under `crates/**`, this path is mandatory.
- Do not use Codex ad-hoc `spawnAgent` / `wait` flow for crate mutations.
- Pass `edit_plan` + `allowlist`; if `pwm_editor` is unavailable, mark `BLOCKED` (no standalone fallback spawn).

```json
{
	"action": "subagent_call",
	"args": {
		"project_id": 5,
		"worker_id": "pwm_editor",
		"worker_class": "subagent",
		"task": "Edit P:/opt/docker/pwm-protocol/crates/cqds-delegation-smoke/src/lib.rs only. Change delegation_ping() to return \"ok-v3\" and delegation_version() to 3. Update unit tests. Reply DONE after save.",
		"edit_plan": [
			{"path": "crates/cqds-delegation-smoke/src/lib.rs", "action": "replace_span", "reason": "ticket change"}
		],
		"allowlist": ["crates/cqds-delegation-smoke/src/lib.rs"],
		"timeout_sec": 300
	}
}
```

Responses:

- `status: done` — use `result`; verify file on disk anyway.
- `status: pending` + `call_id` — poll (§9).
- `status: busy` — retry after ~30s or fail ticket.
- `status: worker_not_found` | `companion_unavailable` — companion / worker config issue (mark ticket `BLOCKED` for crate edits).

Do **not** use `cq_files_ctl#replace` on the same slice when ticket requires editor delegation.

### 10) Poll pending `subagent_call`

Tool: `cq_companion_ctl`

```json
{
	"action": "subagent_poll",
	"args": {
		"project_id": 5,
		"call_id": "<call_id from subagent_call>"
	}
}
```

Poll until `done` or `failed`. Local Ollama edits often need several polls (30–120s total).

### 11) Rebuild CQDS code index (background)

Tool: `cq_files_ctl`

```json
{
	"action": "rebuild_index",
	"args": {
		"project_id": 5,
		"background": true
	}
}
```

Notes:

- For ticket-heavy sessions, prefer bridge calls above directly; avoid repeated `cq_help` round-trips.
- Use `cq_help` only for new/changed actions, uncertain payload fields, or explicit diagnostics (`cq_help#core_status`).
