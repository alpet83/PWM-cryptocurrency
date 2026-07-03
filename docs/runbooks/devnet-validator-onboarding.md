# Devnet Validator Onboarding

This runbook describes the PWM devnet validator path for `pwm-devnet-1` using the V6 stake-gated validator model.

## Scope

- Genesis manifest: `configs/devnet-genesis.json`
- Chain id: `pwm-devnet-1`
- Total supply: `21,000,000,000 PWM`
- Validator admission: stake-gated, using `min_validator_stake` from genesis
- Private validator key material: supplied by each operator out of band; the committed genesis file intentionally does not contain launch private keys

## Prerequisites

1. Build the node and CLI from the release commit selected by the launch coordinator.
2. Copy `configs/devnet-genesis.json` to every node host.
3. Generate or import the operator wallet seed on the validator host.
4. Keep the validator seed and genesis key bundle outside the repository and outside shared docs.
5. Open the configured RPC and P2P ports only to the intended network segment until the bootstrap coordinator announces the devnet peer list.

## Genesis Layout

The launch candidate genesis file uses the following 21B PWM distribution, expressed on chain in raw units with `1 PWM = 1,000,000` raw units.

| Bucket | PWM | Purpose |
| --- | ---: | --- |
| IPv4 claim pool escrow | 20,000,000,000 | Claim allocation reserve |
| Bootstrap operations premine | 300,000,000 | Launch liquidity and operational funding |
| Validator A self-stake | 100,000,000 | Initial proposer/attester set member |
| Validator B self-stake | 100,000,000 | Initial proposer/attester set member |
| Team operations reserve | 400,000,000 | Operational reserve |
| Test faucet reserve | 100,000,000 | Faucet and test grants |

The initial validator set has two public accounts so proposer and attester flows can start without waiting for new operators to cross an epoch boundary.

## Bootstrap Node

Use the launch coordinator's selected addresses and ports. A local dry-run shape is:

```powershell
cmd /c build_project.cmd check -p pwm-core -p pwmd
```

Then start `pwmd` with the launch genesis file and the operator's private key bundle according to the release command template. Do not commit or paste the private key bundle into the repository.

## Joining As A Validator

1. Create or import the validator account on the joining node.
2. Fund the account from the faucet or coordinator allocation.
3. Submit a stake transaction with at least `min_validator_stake` from `configs/devnet-genesis.json`.
4. Wait until the next epoch boundary so the active validator index can admit the account.
5. Confirm that the node appears in the validator set from RPC status, logs, or TUI.

Admission depends on stake being visible at epoch recompute time. If the stake transaction is still pending, wait for seal confirmation before expecting active-set changes.

## Operator Checks

Use these checks after the node starts and after staking:

```powershell
curl http://127.0.0.1:8080/v1/status
curl http://127.0.0.1:8080/v1/head
curl http://127.0.0.1:8080/v1/account/<validator-account>
```

In the TUI, check that the head height advances, the local account balance reflects the stake, and the validator view shows the expected active-set state after the epoch boundary.

## Throughput Gate

The devnet throughput gate is `>= 50 tx/s` sustained on the transfer ramp harness. The standard command shape is:

```powershell
python scripts/cy_cluster_transfer_ramp_soak.py --rpc http://127.0.0.1:8080 --out-prefix docs/reviews/20260629-v7-6-devnet-ramp-live
```

Run this against the launch candidate node set after the private validator key bundle is installed. Commit the resulting report under `docs/reviews/` for the exact release candidate.

## Troubleshooting

- If the node rejects genesis, first validate the JSON syntax and then confirm the runtime command is using the matching key bundle.
- If staking succeeds but the validator is not active, wait for the configured epoch boundary and verify the stake amount is at least `min_validator_stake`.
- If ramp throughput falls below 50 tx/s, collect `/v1/perfmon`, node logs, and the ramp report before changing config.
