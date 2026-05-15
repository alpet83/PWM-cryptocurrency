# SYSTEM BOUNDARIES
- ORCHESTRATOR: Only allowed to modify contents in scripts/, docs/ and tasks/. Other code creating/editing in crates/ must be delegated to sub-agents (see .cursor/agents/Orchestrator.mdc & docs/AGENT_PROMPT_orchestrator.md).
- SUB-AGENTS: Located in `.cursor/agents/`. Session-less actors for code/logs/task processing. 
