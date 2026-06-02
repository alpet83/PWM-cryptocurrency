# PWM MCP cache

Locality rule:

- each project keeps its own MCP cache under `{project_root}\\.git_hub\\mcp-cache`
- each worker agent reads one cache file named after its agent
- each worker updates only its own cache file when live MCP help reveals important missing semantics

Current PWM agent cache files:

- `pwm-coding.md`
- `pwm-debug.md`
- `pwm-review.md`
- `pwm-testing.md`

Usage order:

1. Read `.git_hub\\mcp-cache\\<agent_name>.md`
2. Use it as the default help source for routine CQDS mini MCP work
3. Call `cq_help` only for missing details, suspected drift, or live endpoints such as `cq_help#core_status`
4. Merge important new semantics back into the same agent cache file