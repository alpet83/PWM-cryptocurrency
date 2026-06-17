---
description: "Use when executing PWM-cryptocurrency review tasks as a CQDS bridge worker: independent review, Markdown report delivery, bounded wait_ticket loop, delegated review ticket execution, PWM review subagent."
name: "PWM Review Worker"
user-invocable: true
argument-hint: "Provide worker identity, routing constraints, changed scope or diff summary, review focus, and whether this run is continuous worker-loop mode or single-ticket mode."
---
use tool cq_project_ctl { "action":"prompt_assembler", "args": { "project_id":5, "source_file": ".cqds/prompts/pwm-review-worker.agent.inc" } } then use returned cache_file as main prompt
