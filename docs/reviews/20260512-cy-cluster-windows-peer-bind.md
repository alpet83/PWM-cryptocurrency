# Review: CY cluster lab — Windows peer bind / порты

## Findings

- **Root cause:** In `logs/.../pwmd-peer-*.log`, both nodes logged `peer listener bind 127.0.0.1:3130|3131 failed` (Windows **os error 10013** — socket access forbidden / port in an excluded range). With no listener, outbound dials to seeds timed out; **no steady peer session** → proposer never persisted `(height,0)` round state locally → `missing_round_state` in `run_cluster_gate`. This is environmental, not RFC16 logic.
- **Scripts:** `cy-cluster-common.ps1` now uses peer ports **33430 / 33431 / 33432** (RPC ports unchanged). Comment documents conflict with reserved Windows ranges.
- **Observability:** `spawn_peer_listener_loop` logs a second `warn!` **without** `target: "pwmd::peer"` so the bind failure appears on the main console (previously only in `pwmd-peer` files).

## Verification

- `cargo test -p pwmd`: PASS.
- Two-process lab (proposer + attester, new ports): `pwmd-peer` shows `tcp connect succeeded`, `cluster propose sent`, main log shows `sealed height=1` and further heights.

## Risks / follow-up

- `node-1.ps1` / `node-2.ps1` still default to **3130/3131**; same 10013 can appear outside `cy-cluster-*`. Operators can change `--transport-peer-listen` or run `netsh interface ipv4 show excludedportrange protocol=tcp` to pick free ports.
