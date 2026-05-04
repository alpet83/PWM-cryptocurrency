# Sprint 15 S3.12.1 remediation live testing (CY<->DO)

## Verdict
`FAIL`

## Scope and constraints
- No code changes were made.
- Runtime validation used repository scripts exactly as-is:
  - `node-1.ps1` -> `--genesis-file ./tmp/genesis-custom.json --genesis-passphrase 12345`
  - `node-2.ps1` -> `--genesis-file ./tmp/genesis-custom.json --genesis-passphrase 12345`
- Genesis/passphrase mismatch note: current scripts are pinned to `genesis-custom.json` + `12345`; if operator expectation is different (for example `genesis.yaml` + `1234` from earlier cycle), this run does not match that expectation.

## Commands executed
1. Start nodes:
   - `powershell -NoProfile -ExecutionPolicy Bypass -File "./node-1.ps1"`
   - `powershell -NoProfile -ExecutionPolicy Bypass -File "./node-2.ps1"`
2. Check runtime status:
   - `Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status" | ConvertTo-Json -Depth 6`
   - `Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/status" | ConvertTo-Json -Depth 6`
3. Check foreign account resolution from CY:
   - `Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/account/32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5" | ConvertTo-Json -Depth 8`
   - `Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/account/8b156ec0000ab8efd52949577c1a965d495b9cc7b767c85f771a2c2b5a674dab" | ConvertTo-Json -Depth 8`
4. Validate unknown/unavailable CLI semantics:
   - `cargo run -p pwm-cli -- --rpc http://127.0.0.1:3030 tx-import --master 1111...1111 --domain 0x2C --to 32ecaa...c1c5 --amount 1 --export-id 0000...0001`
   - `cargo run -p pwm-cli -- --rpc http://127.0.0.1:3030 tx-import --master 1111...1111 --domain 0x2C --to 8b156e...4dab --amount 1 --export-id 0000...0001`
5. Simulate unavailable peer path:
   - `taskkill /PID <node-2-powershell-pid> /T /F`
   - verify `3031` is down; re-run CY checks.

## Goal-by-goal results

### 1) Reconnect/hello churn fixed or reduced
**FAIL**

Live logs still show near-constant re-hello around ~2 seconds:
- `[11:42:36.033] #INFO: peer hello accepted node_id=local-node-DO ...`
- `[11:42:38.071] #INFO: peer hello accepted node_id=local-node-DO ...`
- `[11:42:40.130] #INFO: peer hello accepted node_id=local-node-DO ...`
- `[11:42:42.165] #INFO: peer hello accepted node_id=local-node-DO ...`
- `[11:42:44.211] #INFO: peer hello accepted node_id=local-node-DO ...`

On DO side the same cadence is visible for `test-node-CY`.

### 2) CY resolves DO foreign account as authoritative (`home_lookup_status=ok`) with trusted path live
**FAIL**

With both nodes running and status reporting trusted path healthy (`live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health="ok"`), CY lookup for DO account still does not become authoritative:

- Query: `GET /v1/account/32ecaa...c1c5` on CY
- Result:
  - `home_lookup_status: "not_found"`
  - `local_view_only: true`
  - no authoritative `ok` state observed

No sampled foreign DO account produced `home_lookup_status=ok` from CY during this run.

### 3) Unknown/unavailable semantics when peer path unavailable
**PASS**

After stopping node-2 (DO), semantics remained explicit/non-collapsed in CLI preflight:
- `tx-import: recipient home-shard init state is not authoritative (home_lookup_status=not_found); verify trusted peer connectivity before submit`
- `tx-import: recipient home-shard init state is unavailable via protocol peer path; verify trusted peer connectivity before submit`

This confirms unknown/unavailable signaling is still preserved under degraded peer-path conditions.

## Key runtime snippets
- CY status (while both nodes up): `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health="ok"`.
- DO status (while both nodes up): `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health="ok"`, plus repeated peer reconnect evidence.
- After DO stop: `http://127.0.0.1:3031/v1/status` unavailable (`DO_DOWN` check), while CY still serves account lookups.

## Blockers
1. Reconnect churn remains active (~2s re-hello loop), so remediation target for churn is not met.
2. Authoritative foreign resolution (`home_lookup_status=ok`) from CY to DO not reproduced.
3. Potential environment expectation mismatch if operator requires non-default genesis/passphrase values.

## Cleanup
- Test processes cleaned: yes.
- Node scripts launched for this run were terminated (`taskkill /T /F`), no `pwmd`/`pwm` processes left running.
