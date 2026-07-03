# V7-6 Devnet Throughput Gate

Date: 2026-06-29
Ticket: `20260629-v7-6-devnet-launch`
Target: `>= 50 tx/s` sustained transfer ramp throughput for devnet launch readiness.

## Result

Status: `PARTIAL`

The repository now has a launch-candidate genesis manifest and validator onboarding runbook. A fresh live ramp was not executed in this worker session because no already-running devnet RPC or private validator key bundle was available. The committed gate note therefore records current evidence and the exact command shape for the operator rerun.

## Current Evidence

| Source | Evidence |
| --- | --- |
| `docs/plans/mvp_v7.md` | V7 overview records the optimized transfer path at about `76 tx/s` sustained after V7-S1 flamegraph work. |
| `docs/reviews/v7-s2-ramp-results.md` | Ramp rows after S2 show successful progression beyond the earlier 50-row target, with follow-up notes mentioning 68 tx/block after V7-S3 changes. |
| `docs/reviews/v7-s3-worker-scale-results.md` | Worker-scale run records 58 tx/block with zero rejects in the debug log slice. |

This evidence is enough to show the codebase has previously crossed the launch throughput target, but it is not a replacement for the final launch-candidate rerun against `configs/devnet-genesis.json` plus the real validator key bundle.

## Smoke Performed

The benchmark harness was checked for import/CLI availability with:

```powershell
python scripts/cy_cluster_transfer_ramp_soak.py --help
```

## Required Operator Rerun

After the launch coordinator installs the private validator key bundle and starts the node set, run:

```powershell
python scripts/cy_cluster_transfer_ramp_soak.py --rpc http://127.0.0.1:8080 --out-prefix docs/reviews/20260629-v7-6-devnet-ramp-live
```

Pass condition:

- sustained throughput is at least `50 tx/s`
- report is committed under `docs/reviews/`
- `/v1/perfmon` output is captured when available
- node logs show no sustained reject or seal stalls during the plateau

## Notes

- `configs/devnet-genesis.json` intentionally does not commit validator private keys.
- Direct node boot from the devnet genesis manifest requires the operator-supplied key bundle at launch time.
- No wire compatibility impact: this ticket adds config and docs only.
