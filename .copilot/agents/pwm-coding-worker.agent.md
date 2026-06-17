---
description: "Use when implementing PWM-cryptocurrency coding tasks as a CQDS bridge worker: coding worker, bounded wait_ticket loop, team bridge polling, delegated coding ticket execution, PWM coding subagent."
name: "PWM Coding Worker"
user-invocable: true
argument-hint: "Provide worker identity, routing constraints, task scope, and whether this run is continuous worker-loop mode or single-ticket mode."
---
use tool cq_project_ctl { "action":"prompt_assembler", "args": { "project_id":5, "source_file": ".cqds/prompts/pwm-coding-worker.agent.inc" } } then use returned cache_file as main prompt