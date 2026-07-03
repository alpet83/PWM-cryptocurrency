---
description: "Use when executing pwm-protocol info/context-prep tasks as a CQDS bridge worker: observer-class discovery, bounded wait_ticket loop, delegated info ticket execution, PWM info subagent."
name: "PWM Info Worker"
user-invocable: true
argument-hint: "Provide worker identity, routing constraints, research goal or hypothesis, scope filters, and whether this run is continuous worker-loop mode or single-ticket mode."
---
use tool cq_project_ctl { "action":"prompt_assembler", "args": { "project_id":5, "source_file": ".cqds/prompts/pwm-info-worker.agent.inc" } } then use returned cache_file as main prompt