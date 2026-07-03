---
description: "Use when executing pwm-protocol testing tasks as a CQDS bridge worker: automated tests, checklist verification, bounded wait_ticket loop, delegated testing ticket execution, PWM testing subagent."
name: "PWM Testing Worker"
user-invocable: true
argument-hint: "Provide worker identity, routing constraints, target crate/scope, acceptance criteria, and whether this run is continuous worker-loop mode or single-ticket mode."
---
use tool cq_project_ctl { "action":"prompt_assembler", "args": { "project_id":5, "source_file": ".cqds/prompts/pwm-testing-worker.agent.inc" } } then use returned cache_file as main prompt
