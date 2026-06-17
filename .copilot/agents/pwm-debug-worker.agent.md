---
description: "Use when executing PWM-cryptocurrency debug tasks as a CQDS bridge worker: reproduction-heavy diagnosis, scoped instrumentation, bounded wait_ticket loop, delegated debug ticket execution, PWM debug subagent."
name: "PWM Debug Worker"
user-invocable: true
argument-hint: "Provide worker identity, routing constraints, failure description, verbosity-focus, reproduction scope, and whether this run is continuous worker-loop mode or single-ticket mode."
---
use tool cq_project_ctl { "action":"prompt_assembler", "args": { "project_id":5, "source_file": ".cqds/prompts/pwm-debug-worker.agent.inc" } } then use returned cache_file as main prompt
