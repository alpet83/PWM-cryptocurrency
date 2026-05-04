# PWM-cryptocurrency

PWM native chain implementation (Rust) for matrixchain-oriented runtime experiments: local `pwmd` node, CLI/TUI operator flows, domain-first runtime scenarios, and transport hardening.

## Current State (Sprint 11 migration track)

- `pwmd` runs relay-baseline mode by default in a neutral (non `A|B`) startup profile; shard-enforced behavior is activated only for explicit domain config.
- State is persisted via JSON snapshots (`pwm-data.json`), including startup load + save on tx/seal.
- If snapshot is missing or invalid, node falls back to genesis state (degraded startup is reported).
- Two-node local setup with explicit `cluster_domain_hi` values is supported (separate processes, ports, and state roots).
- Real transport supports multiple configured seed peers (`--transport-real`, `--transport-peer-seed ...`) with handshake/backoff telemetry.
- CLI/TUI operator hardening is in place (explicit nonce/submit errors, configurable RPC timeout envs).
- Domain-first operator contract is in place (`--domain-hi` + `--domain-cluster` as primary UX; `--cluster-domain-hi` is deprecated compat alias); `--shard` remains deprecated compat alias (soft-break, warning, no hard removal in Sprint 11).

## Storage Layout

- Default `--state-root` is `state`.
- Config-level default (`PwmdConfig::default`) uses neutral snapshot path `state/neutral/127.0.0.1+3030/pwm-data.json` (listen tag mirrors default RPC bind).
- CLI runtime default (without explicit identity flags and without `--shard`) uses **`state/neutral/<listen-addr>/pwm-data.json`**, where `<listen-addr>` is `SocketAddr` with `:` replaced by `+` (e.g. `127.0.0.1+3030`). This isolates two Neutral processes that share `--state-root` but use different `--listen`.
- Override anytime with **`--data-file PATH`** (absolute path recommended for ops scripts).
- CLI runtime default with explicit identity flags builds namespaced path `state/domain-hi-0xNN/pwm-data.json`.
- Snapshot path is formed automatically as:
  - neutral relay-default mode (no `--shard`) -> `state/neutral/<listen-tag>/pwm-data.json`
  - explicit domain mode -> `state/domain-hi-0xNN/pwm-data.json`
  - explicit alias mode (`--shard A|B`) -> `state/shard-a|shard-b/pwm-data.json`
- Namespace target is domain-based for explicit mode, with deterministic legacy alias mapping for compat mode.
- **Migration:** older Neutral setups that used flat `state/pwm-data.json` must move the file into the new subdirectory or pass `--data-file` explicitly.
- JsonFile persistence now uses a summary `pwm-data.json` plus `epochs/` JSONL files and a manifest. Normal startup trusts the checkpoint summary and loads only the recent tail; full replay is available with `--snapshot-verify-chain` / `PWM_SNAPSHOT_VERIFY_CHAIN`. See `docs/guide-node-storage-and-snapshot.md`.

## Operator quick start (domain-first)

Перед запуском с конкретным `--domain-hi` см. словарь поддерживаемых доменных кластеров: `docs/DOMAINS.md`.

Single node (explicit domain config):

```powershell
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3030 --network-id devnet --domain-hi 0x10 --cluster-id local-cluster-a --node-id local-node-a
```

Two nodes with different domains and real transport seeds:

```powershell
# Node A
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3030 --state-root state-a --network-id devnet --domain-hi 0x10 --cluster-id local-cluster-a --node-id local-node-a --transport-real --transport-peer-seed 127.0.0.1:3031

# Node B
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3031 --state-root state-b --network-id devnet --domain-hi 0x20 --cluster-id local-cluster-b --node-id local-node-b --transport-real --transport-peer-seed 127.0.0.1:3030
```

Minimal smoke checks:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/dev/peers"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/dev/peers"
```

Expected:
- both nodes report `phase=ready`;
- each node reports connected/seen peer data in `/v1/dev/peers`;
- explicit launches use `domain-hi-0xNN` namespace.

## Network Scope and Limits

- Not limited to strict 1-to-1: one node can manage/connect multiple seed peers.
- Current topology is seed-based (explicit peer list), not a full autonomous discovery mesh.
- Cross-shard EXPORT/IMPORT transaction flow remains deferred until core-level support is finalized.

## Key Docs

- Draft whitepaper: `DRAFT_WHITEPAPER.md`
- Whitepaper (RU): `DRAFT_WHITEPAPER-ru.md`
- White spec: `docs/WHITE_SPEC_v0.md`
- GEO sharding explained (simple): `docs/GEO-SHARDING-EXPLANATION.md`
- Roaming runbook sample: `docs/ROAMING-SAMPLE.md`
- MVP checklist: `docs/MVP-checklist.md`
- Dev smoke guide: `docs/tester-guide-devnet-smoke.md`
- Node storage and snapshot modes: `docs/guide-node-storage-and-snapshot.md`
- CLI/TUI domain-first multi-node guide: `docs/tester-guide-cli-tui-scenarios.md`
- Domain clusters dictionary: `docs/DOMAINS.md`
- Phase 1 checklist: `docs/PHASE1_CHECKLIST.md`
- Phase 1 release summary: `docs/PHASE1_RELEASE_SUMMARY.md`
