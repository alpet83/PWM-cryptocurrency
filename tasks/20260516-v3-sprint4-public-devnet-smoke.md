# MVP V3 Sprint 4 — integrated public-devnet smoke

**Ticket:** `20260516-v3-sprint4-public-devnet-closeout` (slice `slice-0-integrated-smoke`)  
**Runner:** pwm-testing  
**Verdict:** `PASS` (retest 2026-05-16 after deterministic demo-wallet path; initial run was `PASS_WITH_NITS`, см. Nits)  
**Date (UTC-ish):** 2026-05-16

## Retest note (2026-05-16)

После фикса **детерминированного demo wallet** в `scripts/demo-genesis-build.ps1` повторный прогон (см. заметки в `tasks/20260516-v3-sprint4-public-devnet-closeout.json`) дал **`PASS`** на чистом пути без заранее подготовленного YAML. Описанный ниже прогон отражает первый интеграционный заход; ключевые шаги и проверки API совпадают.

## Environment

| Item | Value |
|------|--------|
| Repo root | `P:\opt\docker\PWM-cryptocurrency` |
| Host OS | Windows 10 (`win32 10.0.19045`) |
| Preflight (`tools/dev/preflight_target_debug.ps1`) | `target/debug` **226 464 982** bytes, below 4096 MiB — **removed: no** |
| `CARGO_TARGET_DIR` | `F:\pwm-test\PWM-cryptocurrency` (created/exists) |

## Preflight: processes / ports

- `pwmd` / `pwm-cli` foreground processes: none before start (`Get-Process pwmd`).
- `netstat` filter for `:3030` / `:13030`: **empty** before bring-up.

## Genesis (clean-ish path)

- **Isolated genesis output:** `tmp\v3-smoke-s4\genesis-custom.json`
- **Wallet:** reused `tmp\pwm-testing-demo-wallet.yaml` (`derivation_index` 22979, CY-compliant) — see **Nits** for first-attempt brute-force wallet init failure.
- **Genesis passphrase:** fallback `12345` (LAB/DEMO ONLY; aligns with CY cluster scripts).

### Commands executed

```powershell
$env:CARGO_TARGET_DIR = 'F:\pwm-test\PWM-cryptocurrency'
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\dev\preflight_target_debug.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\demo-genesis-build.ps1 `
  -WalletPath 'tmp\pwm-testing-demo-wallet.yaml' `
  -OutputPath 'tmp\v3-smoke-s4\genesis-custom.json' -SkipVerify
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\demo-genesis-verify.ps1 `
  -GenesisPath 'tmp\v3-smoke-s4\genesis-custom.json' -ExpectedPremineRaw 21000000000000000
```

Premine verifier: **`Premine verified: 21000000000000000 raw`** (exit **0**) — canonical `21_000_000_000_000_000` raw.

### Build warmup

```powershell
$env:CARGO_TARGET_DIR = 'F:\pwm-test\PWM-cryptocurrency'
cargo build -p pwmd --bin pwmd
```

## Devnet topology

- **Mode:** documented **CY 3-node** (`cy-cluster-follower.ps1`, `cy-cluster-attester.ps1`, `cy-cluster-proposer.ps1`), same order spacing ~3 s between spawns (follower→attester→proposer).
- **Environment (inherited by child shells):**

  ```text
  PWM_DEMO_GENESIS_PATH=P:\opt\docker\PWM-cryptocurrency\tmp\v3-smoke-s4\genesis-custom.json
  PWM_DEMO_GENESIS_PASSPHRASE=12345
  CARGO_TARGET_DIR=F:\pwm-test\PWM-cryptocurrency
  ```

- **Genesis file:** isolated under `tmp\v3-smoke-s4` (fresh JSON for this run).
- **State / logs:**
  - `tmp\state-cy-proposer`, `tmp\state-cy-attester`, `tmp\state-cy-follower` — **deleted recursively before smoke** then repopulated by nodes.
  - Node stdout/stderr: `tmp\v3-smoke-s4\logs\*.{out|err}.txt`
- **API base (documented smoke):** proposer **`http://127.0.0.1:3030`**.

## API smoke (`/v1/*`)

Captured live (truncation only where noted):

- **`GET /v1/status`:** `"phase":"ready"`, `"ready":true`, `"shard":"CY"`, `"node_id":"cy-proposer"`
- **`GET /v1/head`:** `height`=4 during capture, non-empty hex `tip`
- **`GET /v1/accounts`:** envelope shape **`{ "accounts": [ … ] }`**; premier row id `2c1c…` with premine-aligned balance fields
- **`GET /v1/account/<id>`:** 200 OK for first genesis account id; balance fields consistent with list entry

Smoke PowerShell snippet used:

```powershell
Invoke-RestMethod 'http://127.0.0.1:3030/v1/status'
Invoke-RestMethod 'http://127.0.0.1:3030/v1/head'
$resp = Invoke-RestMethod 'http://127.0.0.1:3030/v1/accounts'
$id = $resp.accounts[0].id
Invoke-RestMethod "http://127.0.0.1:3030/v1/account/$id"
```

## Cleanup

- Stopped **`pwmd` PIDs 22816, 66736, 71716** via `Stop-Process -Force`
- Post-check `netstat` `:3030` filter: empty
- **No intentionally long-lived demos left running**

## Nits (`PASS_WITH_NITS`)

1. **`wallet init --country CY` brute-force ran ~67 s then panicked `no match` (exit 101)** when invoked with `-WalletPath tmp\v3-smoke-s4\demo-wallet.yaml` + `-ForceRecreateWallet`. Workaround for reproducible CI/smoke: **reuse a known-good CY wallet** (`tmp\pwm-testing-demo-wallet.yaml`) or pass deterministic `-DerivationIndex` (requires policy-valid index document in runbook if made normative).

2. **Docs alignment:** `/v1/accounts` is `{ accounts: [...] }`; runbook/API doc examples previously treated the response like a bare array — **corrected mechanically** (`docs/runbooks/demo-devnet-quickstart.md`, `docs/api-v1.md`, echoed hint in `scripts/demo-devnet-start.ps1`).

## CQDS tooling

Host long-running `cargo run`/background harness was delegated to **`Start-Process` + log redirects** inside this pwsh harness (local session). MCP `cq_process_ctl` schemas were consulted via `cq_help`; no blocking CQDS outages observed.
