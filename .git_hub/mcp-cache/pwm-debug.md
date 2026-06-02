# pwm-debug MCP cache

Use this file first for routine CQDS mini MCP work in PWM.

Core defaults:

- `project_id = 5`
- `agent_name = pwm-debug`
- prefer `project_id`, not `tasks_root`
- bounded bridge loop: `worker_status` -> up to 20 `wait_ticket(timeout_sec=240)` -> `unregister_worker`
- use `wait_ticket` as the primary idle-loop primitive; it returns immediately when a matching ticket appears on disk
- use `poll_ticket` only for diagnostics, one-shot status checks, or recovery checks requested by the parent

Common bridge actions:

- `wait_ticket`
- `ack_ticket`
- `heartbeat_ticket`
- `update_ticket_progress`
- `submit_ticket_result`
- `reassign_worker` only after restart-style lease mismatch when old `lease_owner_pid` is gone

Common CQDS tools:

- `cq_project_ctl#list_projects`
- `cq_project_ctl#select_project`
- `cq_project_ctl#project_status`
- `cq_files_ctl#start_grep`
- `cq_project_ctl#fetch_result`
- `cq_files_ctl#read_file`
- `cq_files_ctl#grep_logs`
- `cq_exec_ctl#exec`
- `cq_process_ctl` for long-lived repro flows

## lldb (when parent enables it)

- Use the **lldb MCP server** for live `pwmd` attach on Windows lab hosts when log correlation is ambiguous (e.g. `got=0` attestations with `live_synced=1`).
- Prefer breakpoints on cluster attest/propose and tip apply paths; capture backtrace + key locals; paste transcript under `tasks/<id>-debug-evidence/lldb-*.txt`.
- Do not rely on lldb for first-pass grep of `logs/` — correlation first, debugger second.

Fallback rule:

- use `cq_help` only if this file is missing a needed contract, runtime behavior appears newer, or you need `cq_help#core_status`
