# Sprint 15 S3.12 live validation (CY<->DO)

## Verdict
`FAIL`

## Scope and constraints
- Requested environment mismatch: `genesis.yaml` and passphrase `1234` were requested, but `node-1.ps1`/`node-2.ps1` are hardcoded to `--genesis-file ./tmp/genesis-custom.json --genesis-passphrase 12345`.
- No repository code changes were made.
- Validation executed with the existing scripts exactly as present in repo.

## Commands (operator evidence)
1. Start live nodes:
   - `.\node-1.ps1`
   - `.\node-2.ps1`
2. Connectivity snapshot:
   - `Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"`
   - `Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/status"`
3. CY -> DO foreign account checks:
   - `Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/account/32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5"`
   - `Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/account/8b156ec0000ab8efd52949577c1a965d495b9cc7b767c85f771a2c2b5a674dab"`
4. CLI semantics check (mirror TUI expectations):
   - `cargo run -p pwm-cli -- --rpc http://127.0.0.1:3030 tx-import --master 1111111111111111111111111111111111111111111111111111111111111111 --domain 0x2C --to 32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5 --amount 1 --export-id 0000000000000000000000000000000000000000000000000000000000000001`
   - `cargo run -p pwm-cli -- --rpc http://127.0.0.1:3030 tx-import --master 1111111111111111111111111111111111111111111111111111111111111111 --domain 0x2C --to 8b156ec0000ab8efd52949577c1a965d495b9cc7b767c85f771a2c2b5a674dab --amount 1 --export-id 0000000000000000000000000000000000000000000000000000000000000001`

## Observed results

### 1) Reconnect churn (goal: fixed/reduced)
**Not satisfied.**

Terminal logs still show continuous hello churn near ~2s cadence, for example:
- `[11:30:39.168] #INFO: peer hello accepted ...`
- `[11:30:41.208] #INFO: peer hello accepted ...`
- `[11:30:43.246] #INFO: peer hello accepted ...`
- `[11:30:45.283] #INFO: peer hello accepted ...`

This pattern persisted over the multi-minute observation window.

### 2) Protocol connectivity CY<->DO (goal: restored)
**Satisfied.**

Status snapshot showed protocol path up on both nodes:
- CY: `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health="ok"`, `peer_listen="127.0.0.1:3130"`
- DO: `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health="ok"`, `peer_listen="127.0.0.1:3131"`

### 3) Foreign balance visibility from CY for DO addresses
**Not satisfied for known-value expectation.**

For DO account `32ecaa...c1c5` queried from CY:
- `home_lookup_status="not_found"`
- `authoritative_home_balance=null`
- `balance_pwm="0"` (legacy clamp)
- `local_state_balance="1000000"`

The result is explicit, but authoritative foreign value was not surfaced on CY.

### 4) Explicit unavailable/known semantics (CLI vs TUI contract intent)
**Satisfied for explicitness.**

From CY (`pwm-cli tx-import`) preflight messages are explicit and non-ambiguous:
- For `home_lookup_status=not_found`:
  - `recipient home-shard init state is not authoritative (home_lookup_status=not_found); verify trusted peer connectivity before submit`
- For unavailable peer-path case:
  - `recipient home-shard init state is unavailable via protocol peer path; verify trusted peer connectivity before submit`

This matches explicit unknown/unavailable semantics (no fake "initialized/uninitialized" collapse).

## Blockers (precise)
1. **Environment mismatch blocker**: requested `genesis.yaml` + passphrase `1234` is not representable by current `node-1.ps1`/`node-2.ps1` (hardcoded `genesis-custom.json` + `12345`).
2. **Reconnect churn blocker**: hello traffic still loops at ~2s cadence during live soak.
3. **Foreign visibility blocker**: CY query for known DO address did not return authoritative home balance (`home_lookup_status=not_found`, `authoritative_home_balance=null`).

## Minimal repro
1. In `P:/opt/docker/pwm-protocol`, run:
   - `.\node-1.ps1`
   - `.\node-2.ps1`
2. Wait 2-3 minutes, inspect node logs:
   - observe repeated `peer hello accepted` roughly every ~2 seconds.
3. Query status:
   - `GET http://127.0.0.1:3030/v1/status`
   - `GET http://127.0.0.1:3031/v1/status`
   - both can report healthy connectivity (`live_peer_count=1`, `peer_relay_health=ok`).
4. Query known DO account from CY:
   - `GET http://127.0.0.1:3030/v1/account/32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5`
   - observe `home_lookup_status=not_found` with `authoritative_home_balance=null`.

