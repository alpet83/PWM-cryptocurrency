# Live peer connectivity: node scripts probe

## Scope

Проверка live-нод, запущенных из корня репозитория:
- `node-1.ps1` -> CY node, `127.0.0.1:3030`.
- `node-2.ps1` -> DO node, `127.0.0.1:3031`.

Цель: понять, почему `peer connectivity unhealthy` повторяется, хотя обе ноды живы.

## Script args

`node-1.ps1`:

```powershell
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3030 --state-root ./tmp/state-testnet --data-file ./tmp/state-testnet/pwm-data.json --genesis-file ./tmp/genesis-custom.json --genesis-passphrase "12345" --network-id testnet-qa --domain-hi 0x2C --cluster-id test-cluster-CY --node-id test-node-CY --transport-real --transport-peer-seed 127.0.0.1:3031
```

`node-2.ps1`:

```powershell
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3031 --state-root ./tmp/state-testnet2 --network-id testnet-qa --domain-hi 0x32 --cluster-id local-cluster-DO --node-id local-node-DO --transport-real --transport-peer-seed 127.0.0.1:3030
```

Important mismatch: node 2 does not pass `--genesis-file`, `--genesis-passphrase`, or `--data-file`; node 1 does.

## Live observations

Terminal logs confirm the current script launches include reciprocal `--transport-real` and `--transport-peer-seed`.

Both nodes reached ready state and keep sealing blocks:
- `3030`: `pwmd listening on http://127.0.0.1:3030 ... identity=(testnet-qa,0x2C,test-cluster-CY,test-node-CY)`.
- `3031`: `pwmd listening on http://127.0.0.1:3031 ... identity=(testnet-qa,0x32,local-cluster-DO,local-node-DO)`.

Both nodes repeat:

```text
#WARN: peer connectivity unhealthy: live_peer_count=0 seed_count=1 next_reconnect_in_ms=...
```

No `peer hello accepted ...` or `peer hello rejected ...` lines were found in the terminal captures. That means the live raw transport handshake is not reaching the handshake validation path.

HTTP probes:
- `GET http://127.0.0.1:3030/v1/status`: `200`, `ready=true`, `peer_seed_count=1`, `live_peer_count=0`, `peer_relay_health=unhealthy_no_live_peer`, `effective_genesis_hash=9ab080cbfc8a9216fc274e3f4c29ee7e4a9da56c076835d7ad1325f22935453d`, `genesis_mismatch_total=0`.
- `GET http://127.0.0.1:3031/v1/status`: `200`, `ready=true`, `peer_seed_count=1`, `live_peer_count=0`, `peer_relay_health=unhealthy_no_live_peer`, `effective_genesis_hash=678c973671ef3fc404b65895af1e6a55683ef0112c2016e846fed33b37803f46`, `genesis_mismatch_total=0`.
- `GET /v1/dev/peers`: `404` on both live nodes.
- `GET /v1/flow/recent`: `200`, empty rows on both nodes.
- `GET /v1/peer/hello`: `405`, so the peer hello route exists but only accepts `POST`.

The genesis hashes differ. The `genesis_mismatch_total=0` counter stayed zero because no peer hello was accepted/rejected by the live transport path.

## Code-level clue

`crates/pwmd/src/transport.rs` outbound real transport connects to each seed with `TcpStream::connect(seed)`, writes a length-prefixed JSON `NodeHello`, then waits for a length-prefixed `NodeHello` response.

Search found no production `TcpListener`/`accept` path for that raw framed transport in `transport.rs`; only tests create such a listener. The production listener on `--listen 127.0.0.1:3030/3031` is the HTTP server from `lifecycle.rs`.

So the current live topology points the raw socket dialer at an HTTP server port. That produces retryable connect/handshake failures and only surfaces as `live_peer_count=0`; it does not produce handshake accept/reject logs.

## Verdict

**FAIL**

Primary root cause category: **transport socket handshake issue / missing inbound raw transport accept path**.

Secondary blocker: **wrong startup args / genesis hash mismatch**. Even after a raw transport accept path exists, these two current scripts would not form a healthy trusted peer relationship because node 1 and node 2 advertise different `effective_genesis_hash` values.

## Remediation

1. Decide the transport contract:
   - Either implement a real inbound raw transport listener/accept loop and bind it to a dedicated transport address, then point `--transport-peer-seed` at that transport address.
   - Or change real transport seed dialing to use the existing HTTP `POST /v1/peer/hello` route instead of length-prefixed raw TCP frames.

2. Expose/check peer diagnostics:
   - Fix or verify live availability of `GET /v1/dev/peers`; docs and router source say it should exist, but current live nodes return `404`.

3. Align genesis before re-testing:
   - If `./tmp/genesis-custom.json` + passphrase `12345` is intended, add the same `--genesis-file ./tmp/genesis-custom.json --genesis-passphrase "12345"` to `node-2.ps1`.
   - Start from clean compatible state roots/snapshots, or explicitly verify the persisted snapshots were created from the same genesis.
   - Re-query `/v1/status` and require identical `effective_genesis_hash` before judging peer transport health.

4. Expected post-fix smoke criteria:
   - both `/v1/status` responses show identical `effective_genesis_hash`;
   - both show `peer_seed_count=1`, `live_peer_count>=1`, `peer_relay_health` no longer `unhealthy_no_live_peer`;
   - logs contain `peer hello accepted ...` on each side, or explicit `peer hello rejected ... reason=...` if validation fails.

## Commands run

- Read `node-1.ps1`, `node-2.ps1`, and both live terminal captures.
- Queried `GET /v1/status`, `GET /v1/dev/peers`, `GET /v1/flow/recent`, and route probes for `/v1/peer/hello`.
- Searched logs and transport source for peer, transport, handshake, genesis, listener, and accept/reject clues.

No code changes were made.
