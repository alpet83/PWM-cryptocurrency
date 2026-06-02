# Audit: `tasks/queue` vs `.cqds/team-tasks/queue`

**Date:** 2026-05-31  
**Trigger:** Owner sees pile-up in legacy `tasks/queue` while bridge worker queue is empty.

## Two different directories (not a single `tasks_root` bug alone)

| Path | Role |
|------|------|
| `tasks/<id>.json`, `tasks/in_progress/` | Orchestrator **planning** copies (human/edited JSON) |
| `.cqds/team-tasks/queue/` | Bridge **worker** inbox (`wait_ticket` reads here) |
| `.cqds/team-tasks/in_progress/` | Leased / acked worker tickets |
| `.cqds/team-tasks/done/` | Submitted worker results (153 files as of audit) |

**Canonical rule (PWM):** `cq_team_bridge_ctl` with **`project_id: 5`** only — **do not pass `tasks_root`** unless explicit queue-debug.  
`bridge_status` resolves `team_tasks_root = <project>/.cqds/team-tasks`, not `tasks/queue`.

**Doc drift:** `docs/AGENT_PROMPT_orchestrator.md` still says layout `tasks/queue/` under project root; runtime bridge uses **`.cqds/team-tasks/`**. That mismatch encourages writing to `tasks/queue/` by hand.

## What `share_ticket` does

On success it **moves** the source file into `.cqds/team-tasks/queue/` and sets `shared: true`. It does **not** always delete a pre-existing duplicate if the ticket was **copied** into `tasks/queue/` separately. Many completed slices left a **ghost** `tasks/queue/<id>.json` with stale `status: queue`.

## Snapshot (2026-05-31)

- `tasks/queue/`: **15** JSON files  
- `.cqds/team-tasks/queue/`: **0**  
- `.cqds/team-tasks/in_progress/`: **1** (active)  
- `.cqds/team-tasks/done/`: **153**

### `tasks/queue` breakdown

| Verdict | Count | Meaning |
|---------|-------|---------|
| **GHOST** | 12 | Also in `.cqds/team-tasks/done` with `status: done` — work finished; safe to remove from `tasks/queue` |
| **ACTIVE ghost** | 1 | `20260607-v5-cy-gate-poll-optimization-experiments-debug` — real ticket in `.cqds/team-tasks/in_progress`; delete `tasks/queue` copy only |
| **STUCK (never bridged)** | 2 | Only in `tasks/queue`: gate-readiness + propose-timeout coding (orchestrator wrote queue without `share_ticket`) |
| **Broken JSON** | 1 | `20260607-v5-resumed-state-seal-throughput-coding.json` — parse error in `tasks/queue` copy; bridge `done` copy is OK |

### Other stale copies

| File | Issue |
|------|--------|
| `tasks/in_progress/20260610-v5-attester-sync-fork-continuity-failclosed-coding.json` | `in_progress` but `.cqds/.../done` + `status: done` |
| `tasks/in_progress/20260610-v5-cy-proposer-attest-gap-iter2-debug.json` | `done` in file body; should live only under `tasks/done/` |

## `tasks_root` incident (related, different symptom)

Without `project_id: 5`, MCP defaulted to Colloquium install `…/cqds/tasks` — worker stole foreign smoke tickets. See `issues-report.md` (2026-05-22 bridge routing).  

**Not the same** as `tasks/queue` pile-up: that is **ghost duplicates** + **direct writes to wrong folder**, not wrong global CQDS install (unless someone passed explicit `tasks_root: .../tasks` overriding team-tasks).

## Recommended hygiene

1. Stop creating `tasks/queue/<id>.json` manually; use `tasks/in_progress/<id>.json` then **`share_ticket`** (`project_id: 5`).
2. Run `scripts/reconcile_tasks_queue_ghosts.ps1` (dry-run first) to archive/remove ghosts.
3. **`share_ticket`** the two STUCK coding tickets into bridge queue.
4. Fix orchestrator doc: worker queue path = `.cqds/team-tasks/queue`, planning = `tasks/` root only.
