# Review: V7-6 21B devnet genesis + validator onboarding + throughput gate (8dabd0b)

- date: 2026-06-29
- ticket: `20260629-v7-6-devnet-review`
- coding_ticket: `20260629-v7-6-devnet-launch`
- commit: `8dabd0b` (branch `main` at review time)
- scope: `configs/devnet-genesis.json`, `docs/runbooks/devnet-validator-onboarding.md`, `docs/reviews/20260629-v7-6-devnet-ramp.md` (config/docs only — no `crates/` changes)

## 1. Scope recap

V7-6 delivers launch-candidate **documentation and genesis manifest** (PARTIAL on live ramp):

| deliverable | path |
|-------------|------|
| 21B genesis manifest | `configs/devnet-genesis.json` |
| Validator onboarding runbook | `docs/runbooks/devnet-validator-onboarding.md` |
| Throughput gate evidence note | `docs/reviews/20260629-v7-6-devnet-ramp.md` |

Done criterion per ticket: genesis valid, runbook complete, gate evidence present (live ramp deferred — sandbox).

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| 1. Genesis 21B supply + distribution | **PASS** | `total_supply_raw` = `21000000000000000` (= 21B PWM × 1e6 raw); funding sum matches (`configs/devnet-genesis.json:12-55`) |
| 2. Validator rows + V6 PoS params | **PASS** with nit | Two `validators.set` rows with matching `pubkey_hex` / `acct_hex` to funding entries (`:57-69`); `min_validator_stake` / `pwm_stake_min` = `100000` raw; self-stake `100000000000000` raw (100M PWM) ≫ threshold |
| 3. Onboarding runbook | **PASS** with nit | Prerequisites, genesis copy, stake steps, epoch wait, RPC curls, TUI check, troubleshooting (`devnet-validator-onboarding.md`) |
| 4. Throughput gate evidence + rerun cmd | **PASS** | `20260629-v7-6-devnet-ramp.md` cites V7-S1 ~76 tx/s; `--out-prefix` matches script (`cy_cluster_transfer_ramp_soak.py:710-731`) |
| 5. PARTIAL live ramp acceptable | **PASS** (staged) | Prior evidence crosses ≥50 tx/s gate; launch-candidate rerun on devnet genesis explicitly deferred — honest PARTIAL |
| 6. No private key material | **PASS** | `validator_keys: []`; only `pubkey_hex` in manifest; runbook warns OOB key bundle |
| 7. Build breakage | **PASS** (N/A) | No Rust changes; genesis is external JSON |

## 3. Genesis manifest analysis

### Supply arithmetic

| bucket (raw) | PWM (÷1e6) |
|--------------|------------|
| 20,000,000,000,000,000 | 20,000,000,000 |
| 300,000,000,000,000 | 300,000,000 |
| 100,000,000,000,000 ×2 | 200,000,000 |
| 400,000,000,000,000 | 400,000,000 |
| 100,000,000,000,000 | 100,000,000 |
| **Σ 21,000,000,000,000,000** | **21,000,000,000** |

Matches `network.total_supply_pwm` / `total_supply_raw` metadata (`:7-8`).

### Distribution vs `docs/genesis-21b-design.md`

| design bucket | design PWM | devnet PWM | note |
|---------------|------------|-------------|------|
| IPv4 claim pool | 20B | 20B | ✓ |
| Bootstrap premine | 500M | 300M + 200M validator self-stake | devnet splits bootstrap into explicit validator stakes |
| Team reserve | 400M | 400M | ✓ |
| Faucet | 100M | 100M | ✓ |

Plausible for launch candidate; not a supply error.

### Validator / epoch consistency

- `schema_version: 5` — supported by `pwmd` genesis loader (`snapshot/genesis.rs`).
- `policy_ver: 1`, stake mins aligned with V6 admission model.
- IPv4 phase-1 `allocation` 4B PWM within 20B escrow; `registry_address` matches escrow account hex (`:84-89`).
- **Nit:** both validators use `der_idx: 1` — acceptable for bootstrap, document coordinator mapping.

## 4. Onboarding runbook

Covers:

- Prerequisites and OOB key handling
- Genesis layout table (matches JSON buckets)
- Join flow: fund → stake ≥ `min_validator_stake` → epoch boundary → RPC/TUI verification
- Throughput gate command with correct `--out-prefix`

**Gap (nit):** no copy-paste `pwmd`/`pwm` CLI invocation with `--genesis` path — references "release command template" only. External operator may need coordinator template.

## 5. Throughput gate (PARTIAL)

`docs/reviews/20260629-v7-6-devnet-ramp.md`:

- Status `PARTIAL` — no live ramp in worker sandbox (no RPC + private key bundle).
- Evidence table references `mvp_v7.md` / V7-S1 closure (~76 tx/s sustained) and V7-S2/V7-S3 ramp reports (≥50 tx/block).
- Operator rerun command:

```powershell
python scripts/cy_cluster_transfer_ramp_soak.py --rpc http://127.0.0.1:8080 --out-prefix docs/reviews/20260629-v7-6-devnet-ramp-live
```

**Assessment:** Excluding live ramp in sandbox is **acceptable for V7-6 coding ticket** (config + docs). Prior V7-1 evidence satisfies the **≥50 tx/s** engineering gate for codebase readiness. **Launch sign-off** still requires rerun against `pwm-devnet-1` with operator key bundle — explicitly documented.

## 6. Style and module shape

Config/docs-only slice — no production Rust identifiers to audit.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). Genesis balances use decimal string raw units — consistent with existing genesis v5 JSON.

## 7. Safety

- No secrets committed in genesis manifest.
- Runbook instructs keeping seeds outside repo.
- Large premine buckets are coordinator-controlled — expected for devnet launch.

## 8. Tests

No automated test loads `configs/devnet-genesis.json` in repo (nit). Manual JSON syntax + sum check performed in review.

## 9. Concurrency / parallelism

Not in diff scope (spot-check only: no new shared-state surfaces observed).

## 10. BLOCKERs

None. Total supply is 21B PWM; onboarding runbook includes stake, epoch, and verification steps.

## 11. Nits (non-blocking)

1. **NIT-1:** Add `scripts/_review_verify_devnet_genesis.py` (or extend existing genesis verifier) to CI-check sum == `total_supply_raw`.
2. **NIT-2:** Runbook: concrete `pwmd` start example with genesis path flag (redact keys).
3. **NIT-3:** Note bootstrap 300M vs design-doc 500M split in runbook §Genesis Layout.
4. **NIT-4:** Link a dedicated ramp report with tx/s numbers (not only tx/block) for audit trail.

## 12. Verdict

**Approve with nits** — 21B genesis manifest arithmetic correct; validator pubkeys and stake params consistent with V6 admission; onboarding runbook actionable; throughput evidence + rerun command present; no private keys committed. PARTIAL live ramp is acceptable for this docs/config slice with documented operator follow-up.

## 13. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-v7-6-devnet-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 32000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-6-devnet-review.md'
git commit -m 'docs(v7-6): devnet genesis and onboarding review (8dabd0b)'
```