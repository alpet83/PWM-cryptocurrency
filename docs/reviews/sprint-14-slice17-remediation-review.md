# Sprint 14 — Slice 17 remediation review

## Verdict
`request changes`

## New blocker
- Потенциальный дедлок из-за инверсии порядка lock-ов (`inner` -> `init` и `init` -> `inner`) в API/status/seal путях.

## Additional nit
- `pwm-tui` production symbol `inter_shard_cli_route_message` нарушает style rule (>\=5 слов).
