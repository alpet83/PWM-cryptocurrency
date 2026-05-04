# Sprint 15 S3.11 Coding

Implemented contract realization from S3.10:

- Added trusted peer account-view exchange in stateful peer wire protocol (`AccountViews`).
- Added peer-backed cache in runtime state for foreign authoritative fields.
- Extended `AcctOut` with:
  - `authoritative_home_initialized`
  - `home_lookup_status` (`local|ok|not_found|unavailable`)
- Wired `/v1/accounts` and `/v1/account/:id` to return foreign authoritative values only from trusted peer path.
- Kept legacy `balance_pwm` behavior for compatibility; new clients can use lookup status + authoritative fields.
- Updated `pwm-tui` polling/render/preflight:
  - foreign unknown balance/init renders as `???`,
  - unknown foreign init blocks send with explicit message,
  - no silent fallback to false `0` semantics.
