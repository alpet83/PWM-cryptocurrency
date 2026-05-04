# Sprint 15 S3.12.2 remediation live re-test (CY<->DO)

## Verdict
`FAIL`

## Scope and constraints
- No code changes; runtime-only re-test after remediation.
- Used node scripts from repo as requested:
  - `node-1.ps1` (CY, `127.0.0.1:3030`, peer `3130`)
  - `node-2.ps1` (DO, `127.0.0.1:3031`, peer `3131`)

## Commands executed
1. Start nodes:
   - `powershell -ExecutionPolicy Bypass -File .\node-1.ps1`
   - `powershell -ExecutionPolicy Bypass -File .\node-2.ps1`
2. Check steady behavior and peer health:
   - log sampling from both live terminals (`peer hello accepted`)
   - `Invoke-RestMethod http://127.0.0.1:3030/v1/status`
   - `Invoke-RestMethod http://127.0.0.1:3031/v1/status`
3. Validate CY->DO lookup contract:
   - `Invoke-RestMethod http://127.0.0.1:3030/v1/accounts`
   - `Invoke-RestMethod http://127.0.0.1:3030/v1/account/32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5`
   - repeated Node sampling (10x/1s) of same CY lookup.
4. Validate explicit semantics in CLI:
   - `cargo run -p pwm-cli -- --rpc http://127.0.0.1:3030 tx-import --master 1111.. --domain 0x2C --to 32ecaa... --amount 1 --export-id 0000..0001`
   - `cargo run -p pwm-cli -- --rpc http://127.0.0.1:3030 tx-import --master 1111.. --domain 0x2C --to 8b156e... --amount 1 --export-id 0000..0001`
   - `cargo run -p pwm-cli -- --rpc http://127.0.0.1:3030 tx-import --master 1111.. --domain 0x2C --to 32ffff... --amount 1 --export-id 0000..0001`
5. Degrade peer path:
   - `taskkill /PID 995620 /T /F` (node-2 tree)
   - `node -e "<check http://127.0.0.1:3031/v1/status>"` -> `DO_DOWN ECONNREFUSED`
   - re-check CY lookup + CLI preflight.

## Goal-by-goal validation

### 1) No steady ~2s re-hello churn in normal operation
**FAIL**

Steady re-hello loop is still present on DO side at ~2s cadence:
- `[11:58:46.059] #INFO: peer hello accepted node_id=local-node-DO ...`
- `[11:58:48.096] #INFO: peer hello accepted node_id=local-node-DO ...`
- `[11:58:50.139] #INFO: peer hello accepted node_id=local-node-DO ...`
- `[11:58:52.186] #INFO: peer hello accepted node_id=local-node-DO ...`
- `[11:58:54.229] #INFO: peer hello accepted node_id=local-node-DO ...`

This pattern continues further (`...56.271`, `...58.317`, `...00.356`, etc.).

### 2) CY->DO foreign lookup reaches authoritative (`home_lookup_status=ok`) when trusted path is healthy
**FAIL**

Trusted path health reports as live/ok:
- CY `/v1/status`: `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health="ok"`
- DO `/v1/status`: `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health="ok"`

But CY lookup for DO account does not become authoritative:
- `GET /v1/account/32ecaa...` on CY -> `home_lookup_status: "unavailable"`, `local_view_only: true`, `authoritative_home_balance: null`
- 10 repeated samples (1s interval) all remained `status=unavailable` (no `ok` observed).

### 3) Degraded/unavailable peer-path keeps explicit semantics (`unavailable`/`unknown`)
**PARTIAL PASS**

Explicit `unavailable` semantics are preserved both before and after degradation:
- API (`CY /v1/account/32ecaa...`): `home_lookup_status: "unavailable"`
- CLI preflight:
  - `tx-import: recipient home-shard init state is unavailable via protocol peer path; verify trusted peer connectivity before submit`
- After DO stop: `DO_DOWN ECONNREFUSED`, CY still returns explicit `unavailable`.

For `unknown` branch in this run:
- CLI unknown-address probe (`--to 32ffff...`) returned explicit local-not-found message:
  - `tx-import: recipient account not found on current RPC; recipient must run tx-init on the target shard first`
- A direct `home_lookup_status="unknown"` sample was not emitted in this environment/run.

## Key evidence snippets
- Churn (normal operation): repeated `peer hello accepted` every ~2s on DO.
- Healthy-path status: both nodes report trusted path healthy (`peer_relay_health="ok"`).
- CY foreign lookup sample:
  - `2026-04-30T12:01:23.480Z ... status=unavailable ... auth=null`
  - `2026-04-30T12:01:32.586Z ... status=unavailable ... auth=null`
- Degraded path check:
  - `DO_DOWN ECONNREFUSED`
  - CY lookup still explicit `home_lookup_status="unavailable"`.

## Blockers
1. Persistent ~2s re-hello churn still active in normal operation.
2. Authoritative CY->DO transition to `home_lookup_status=ok` not reproducible despite trusted path health=`ok`.
3. `unknown` branch could not be observed as `home_lookup_status="unknown"` in this live setup (only explicit `unavailable` and explicit local-not-found path observed).

## Cleanup
- Test processes cleaned: yes.
- Stopped node/test binaries (`pwmd.exe`, `pwm-tui.exe`); no `pwmd`/`pwm-tui` processes remained.
