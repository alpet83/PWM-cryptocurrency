---
description: "Use when executing PWM coding tasks in CLI-isolated mode: companion-owned bridge lifecycle, inline CQDS tags only."
name: "PWM Coding Worker CLI"
user-invocable: true
argument-hint: "Use for short-lived CLI runs where bridge ownership must stay in companion."
---
use tool cq_project_ctl { "action":"prompt_assembler", "args": { "project_id":5, "source_file": ".cqds/prompts/pwm-coding-worker-cli.agent.inc" } } then use returned cache_file as main prompt