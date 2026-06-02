# Devnet V5 Operator Smoke (Marks / Inflation)

This runbook documents how to run the basic operator-level smoke test for the lazy marks + inflation path introduced in V5.

## Prerequisites

- Rust toolchain + cargo on PATH
- Repository root
- `CARGO_TARGET_DIR` can be overridden if you want to keep build artifacts outside the default target (recommended for repeated runs)

## Quick Start (Happy Path)

```powershell
# Optional: manual backup before experiments (no CleanState)
.\scripts\devnet_state_backup.ps1 -Label before_experiment

# From repo root, clean previous state (archives first by default)
.\scripts\devnet_v5_operator_smoke.ps1 -CleanState

# Run with default 90s smoke window
.\scripts\devnet_v5_operator_smoke.ps1
```

Useful switches:

- `-AccountInfoOnly` runs only the `pwm account-info` marks-output smoke (slice 4); it still does genesis/nodes/init/stake setup unless `-SkipNodes` is used.

- `-CleanState` — archives then removes previous `tmp/state-*`, `tmp/cy-*`, and genesis wallet files (zip under `tmp/archives/`; use `-SkipArchive` to disable)
- `-SkipArchive` — with `-CleanState`, delete without backup (destructive)
- `-MaxStateArchives 30` — retain newest N zip archives (default 30)
- `-SkipGenesis` — reuse existing `tmp/genesis-custom.json` + wallet
- `-SkipNodes` — only run the logic without starting pwmd (useful for debugging the script itself)
- `-MarksOnly` — run only the marks growth checks (slice 1)
- `-DeferredOnly` — run only deferred policy smoke (slice 2); still does genesis/nodes/init/stake setup unless `-SkipNodes`
- `-DeferredLeadBlocks 20` — blocks ahead of current head for `activate_at_height` (slice 2)
- `-DeferredWaitSeconds 120` — wait for head to reach activation height
- `-DeferredPolicy default_behavior` — policy id for deferred smoke
- `-SmokeSeconds 120` — increase observation window for marks slice
- `-ReportPath C:\temp\my-report.md` — custom report location

## What the script does (Slice 1)

1. (Optional) Clean state
2. Generate / reuse demo genesis + funded wallet (reuses `demo-devnet-start.ps1`)
3. Start CY **proposer + attester** (RFC16 quorum; single proposer alone does not seal blocks)
4. `pwm tx-init` on the demo account **unless** genesis row is already initialized (`initialized: true` in GenCfg.state0)
5. `pwm tx-stake` (small amount)
6. Poll `/v1/head` + `/v1/account` until marks evidence or timeout
7. Write a markdown report under `tmp/devnet_v5_operator_smoke_*.md`

## Expected PASS criteria (slice 1)

- Script exits with code **0** (PASS); **3** = harness exception; **4** = marks timeout; **5** = deferred slice failed
- Report shows staker account id, baseline marks/marks_last_block, and evidence of lazy marks touch:
  - **normal:** `marks` and `marks_last_block` both increased, or
  - **saturated:** baseline `marks == 4294967295` (u32::MAX) and `marks_last_block` increased after stake
- Grep-friendly line in report: `PASS_EVIDENCE: slice=marks account=...`

## Expected PASS criteria (slice 2 — deferred policy)

- Full run (default): slice 1 PASS, then slice 2 runs automatically
- `-DeferredOnly`: skips marks polling; still starts cluster and stakes unless `-SkipNodes`
- Flow:
  1. `tx-policy-set --activation deferred --activate-at-height (head + N)`
  2. Before height: `active_policies == 0`; `tx-policy-activate` exits non-zero
  3. After head >= activate_at: `tx-policy-activate` exits non-zero (**PolicyDenied** / already active via deferred evaluator). Stored `active_policies` may remain **0** by design (ADR 0005: activation is height-gated in evaluator, not always materialized in the bitfield).
- Grep-friendly line: `PASS_EVIDENCE: slice=deferred account=... policy=... activate_at=... active_before=0 active_after=...`

### Example PASS excerpt (slice 2)

```markdown
## Slice2: deferred policy activation (ADR 0005)
- head H0=42; activate_at=62 (lead=20 blocks); policy=default_behavior
- active_policies before height: 0
- pwm exit before height: 2
- head reached 63 (target>=62)
- stored active_policies=0 (evaluator-gated deferred; bitfield may stay 0)
- pwm exit at/after height: 2
PASS_EVIDENCE: slice=deferred account=pwm1-... policy=default_behavior activate_at=62 stored_active_policies=0 head=63 activate_exit_before=2 activate_exit_after=2
**Result**: PASS
```

### Example PASS excerpt (report)

```markdown
- staker account: pwm1-...
- marks baseline: 0, marks_last_block: 0
- head: 12 -> 16; marks=0 marks_last_block=0
- marks advanced: 0 -> 3; marks_last_block: 0 -> 15
PASS_EVIDENCE: account=pwm1-... marks=0->3 marks_last_block=0->15 head=16
**Result**: PASS
```

## Expected PASS criteria (slice 3 — IPv4 claim)

- Full run or `-Ipv4ClaimOnly` must execute a real on-chain `ClaimIPv4Batch`.
- The script must:
  1. Inject a test `ipv4_claim_phases` entry into genesis **after** generation and **before** starting nodes.
  2. Use the dedicated helper (`cargo run -p pwm-cli --bin claim-ipv4-batch`) to produce a correctly signed transaction.
  3. Submit it via `POST /v1/tx`.
  4. Poll the claimant until `ipv4_claimed_phase == phase` **and** balance has increased.
- Must emit a clear `PASS_EVIDENCE` line (see example below).
- `-Ipv4ClaimOnly` must still perform the full genesis + real claim path.

### Example PASS excerpt (slice 3)

```markdown
## Slice3: ClaimIPv4Batch happy path (V5-5)
- ipv4_claim phase ensured in genesis before node startup.
- Claimant from helper: pwm1-...
- ClaimIPv4Batch tx accepted
- Observed ipv4_claimed_phase == 7 and positive balance delta
PASS_EVIDENCE: slice=ipv4_claim phase=7 claimant=pwm1-... registry=pwm1-... balance_before=... balance_after=... delta=...
**Result**: PASS
```

## Expected PASS criteria (slice 4: `pwm account-info`)

- Full run: slice 4 runs after the stake/marks baseline has produced a good operator account.
- `-AccountInfoOnly`: skips slice polling but still starts the cluster, initializes/reuses the demo account, stakes, and waits for the stake nonce before running the CLI check.
- The script runs `pwm account-info --wallet tmp/demo-genesis-wallet.yaml` against the live RPC endpoint.
- The CLI stdout must include `head_height=`, `marks_stored=`, `marks_effective=`, `marks_sat_pct=`, `marks_last_block=`, and `staked=`.
- The smoke asserts `marks_last_block > 0` and `staked > 0`.
- Grep-friendly line: `PASS_EVIDENCE: slice=account_info account=... head_height=... marks_stored=... marks_effective=... marks_sat_pct=... marks_last_block=... staked=...`

### Example PASS excerpt (slice 4)

```markdown
## Slice4: pwm account-info marks output (V5-6/V5-7)
- Running pwm account-info via demo wallet ...
- account-info fields observed: head_height=3 marks_stored=4294967295 marks_effective=4294967295 marks_sat_pct=100 marks_last_block=1 staked=1000000000
PASS_EVIDENCE: slice=account_info account=pwm1-... head_height=3 marks_stored=4294967295 marks_effective=4294967295 marks_sat_pct=100 marks_last_block=1 staked=1000000000
**Result**: PASS
```

## Known Limitations

- Uses a minimal **2-node CY cluster**.
- IPv4 claim currently relies on the small `claim-ipv4-batch` helper for signing.
- Full integration with real wallet derivation for both registry and claimant is still being hardened.
- `-AccountInfoOnly` validates the live CLI surface; it does not alter account-info math.

## Testing handoff (pwm-testing)

For **pwm-testing** on Windows, run the live smoke via MCP **`cq_process_ctl`** (**`spawn` + long `wait`**, `host: true`), per **`docs/AGENT_PROMPT_testing.md`**.

1. Skill **`colloquium-cqds-mcp`**, `project_id: 5`; contract from **`cq_help`** (`cq_process_ctl#spawn`, `#wait`, `#status`, `#io`, `#kill`).
2. **`spawn`**: `cwd` = repo root on Windows (e.g. `P:\opt\docker\PWM-cryptocurrency`); **`command`** array example:

```text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File P:\opt\docker\PWM-cryptocurrency\scripts\devnet_v5_operator_smoke.ps1 -CleanState -SmokeSeconds 120
```

3. In **`env`**, set **`CARGO_TARGET_DIR`** outside the clone if needed (see **`docs/AGENT_PROMPT_testing.md`** Windows section).
4. **`wait`** timeout: allow genesis + cold `cargo` + block production (orient **300–600 s** minimum).
5. On completion, verify exit code **0** and report contains **`PASS_EVIDENCE:`**; ensure **`pwmd`** processes are gone (`#kill` / `taskkill` if needed).

See also **`docs/runbooks/demo-devnet-quickstart.md`** and **`docs/runbooks/cy-cluster-policy-matrix-e2e.md`** for the same `cq_process_ctl` pattern.

## Troubleshooting

- RPC not becoming ready → increase `-StatusWaitSeconds` or check logs in `tmp/devnet-v5-smoke-*/`
- No marks appearing → ensure you staked on an account that is eligible for the current inflation schedule
- PowerShell syntax errors → run with `-SkipNodes` first to validate argument parsing

---

**Last updated**: 2026-05-28 (V5-8 Slice 4 account-info smoke)
