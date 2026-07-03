# Sprint 14 — Slice 20 testing remediation (re-run after fixes)

Repo: `P:/opt/docker/pwm-protocol`
Run dir: `tmp/slice20-e2e-accept-20260429`
Date: 2026-04-29

## Overall verdict
**FAIL** — Slice20 end-to-end acceptance is still not met after the latest remediation.

## Checklist (must pass all 6)
1) **Two-shard CY/DO boot from current genesis flow:** **PASS**
- Both nodes reported `phase=ready` and explicit runtime identity:
  - CY: `shard=CY state_ns=domain-hi-0x2c ... mode=shard_enforced(explicit-domain-config)`
  - DO: `shard=DO state_ns=domain-hi-0x32 ... mode=shard_enforced(explicit-domain-config)`

2) **Local same-hi transfer in CY succeeds (routing + balance/nonce effects):** **FAIL**
Evidence:
- `tx-send` executed but state updates were incorrect:
  - log contains `#DEBUG: tx_included | tx_kind=transfer ... bal_before=1000000 bal_after=1000010 bal_delta=+10`
  - subsequent `GET /v1/account/<sender>` showed `nonce=0` (no nonce increment).
- Transfer to a second CY address did not create the receiver account in-state (receiver remained `account not found`).

3) **Cross-shard CY->DO transfer via pwm-cli completes expected lifecycle (finalize/import path if required):** **FAIL**
Observed behavior:
- `tx-send --to <DO account>` on CY created roaming intent:
  - `status=exported` and produced `export_id=de8ea35c...d6d1c`
- Even after operator finalize on CY:
  - `POST /v1/roaming-intents/<intent_id>/finalize` returned `status=relayed`
- `tx-import` on DO still fails with:
  - `HTTP 400 ... invalid import: export_id is not known`

4) **Restart from pwm-data.json on both nodes without snapshot replay mismatch:** **FAIL**
Evidence (CY restart):
- CY startup degraded with:
  - `snapshot chain mismatch: block[62] state_root does not match replayed state`
  - `ready_degraded (snapshot error: snapshot chain mismatch: ...)`
- DO restart was OK (`snapshot loaded` / `ready`).

5) **Routing guard log labels use CY/DO runtime labels (no shard=A/B):** **FAIL**
Evidence:
- `pwmd-cy.log` contains:
  - `tx routing guard: shard=A sender_hi=0x2C ...`
Expected: runtime label `shard=CY` / `shard=DO` (no legacy `A|B`).

6) **tx balance delta observability logs visible as expected:** **FAIL**
Evidence:
- No `tx commit delta: ...` lines were found.
- Logs showed only `tx_included ... bal_delta=...` lines for transfer; the expected `tx commit delta` category/wording is missing for this run.

## Blockers list (with failing step)
1. (Step 2) Local same-hi transfer has incorrect balance/nonce effects and/or does not initialize the intended receiver account.
2. (Step 3) Export/Finalize/Import handoff does not make `export_id` known on target; `tx-import` fails with `export_id is not known`.
3. (Step 4) CY snapshot replay mismatch on restart: `snapshot chain mismatch: block[62] ... state_root ...`.
4. (Step 5) Routing guard logs still emit legacy `shard=A` instead of `shard=CY`.
5. (Step 6) Missing expected tx commit delta observability strings (only `tx_included ... bal_delta` observed).

## Strongest repro commands (copy/paste ready)
### Common paths/values used in this run
```
RUN_DIR="tmp/slice20-e2e-accept-20260429"
CY_STATE="$RUN_DIR/cy"
DO_STATE="$RUN_DIR/do"
GENESIS="$RUN_DIR/genesis.json"
WALLET_CY="$RUN_DIR/wallet-cy.yaml"
WALLET_DO="$RUN_DIR/wallet-do.yaml"
DO_HEX="326160ace400596d92e7df931cfda30758cb51be268a4d62737d3556969665a0"
SENDER_HEX="2c55b356440049c5fd7e4b55bf7f7857455b0c4e04e46c3ec1d6b88fdeb058b5"
INTENT_ID="de8ea35cbc4d94a9b2887996488074aa66396216bc1bce2f91378d96e12a6d1c"
EXPORT_ID="de8ea35cbc4d94a9b2887996488074aa66396216bc1bce2f91378d96e12a6d1c"
```

### Start nodes (used for steps 1–6)
```
# CY
target/debug/pwmd.exe `
  --listen 127.0.0.1:4030 `
  --state-root "$CY_STATE" `
  --data-file "$CY_STATE/pwm-data.json" `
  --genesis-file "$GENESIS" `
  --genesis-passphrase 12345 `
  --network-id testnet-s14 `
  --domain-hi 0x2C `
  --cluster-id cluster-CY `
  --node-id node-CY `
  --transport-real `
  --transport-peer-seed 127.0.0.1:4040

# DO
target/debug/pwmd.exe `
  --listen 127.0.0.1:4040 `
  --state-root "$DO_STATE" `
  --data-file "$DO_STATE/pwm-data.json" `
  --genesis-file "$GENESIS" `
  --genesis-passphrase 12345 `
  --network-id testnet-s14 `
  --domain-hi 0x32 `
  --cluster-id cluster-DO `
  --node-id node-DO `
  --transport-real `
  --transport-peer-seed 127.0.0.1:4030
```

### Step 2 repro (local same-hi transfer on CY)
```
target/debug/pwm.exe --rpc http://127.0.0.1:4030 tx-send `
  --wallet "$WALLET_CY" `
  --to "$SENDER_HEX" `
  --amount 10 `
  --fee 1

# observe:
# - GET http://127.0.0.1:4030/v1/account/$SENDER_HEX should reflect correct debit/credit and nonce increment,
#   but in this run nonce stayed 0 and bal_delta was inconsistent with expected debit+credit.
```

### Step 3 repro (cross-shard lifecycle: tx-send -> finalize -> tx-import)
1) Create roaming intent on CY:
```
target/debug/pwm.exe --rpc http://127.0.0.1:4030 tx-send `
  --wallet "$WALLET_CY" `
  --to "$DO_HEX" `
  --amount 100 `
  --fee 1
```

2) Finalize on CY:
```
Invoke-RestMethod -Uri ("http://127.0.0.1:4030/v1/roaming-intents/" + $INTENT_ID + "/finalize") -Method Post
```

3) Import on DO (fails):
```
target/debug/pwmd.exe --rpc http://127.0.0.1:4040 tx-import `
  --wallet "$WALLET_DO" `
  --to "$DO_HEX" `
  --amount 100 `
  --export-id "$EXPORT_ID"
```
Expected: `204 No Content` and target state updated.
Actual: `HTTP 400 ... invalid import: export_id is not known`.

### Step 4 repro (restart causing CY snapshot replay mismatch)
After any run that produces the same `"$CY_STATE/pwm-data.json"` content, restart CY:
```
target/debug/pwmd.exe (same CY args as above, but with existing --data-file "$CY_STATE/pwm-data.json")
```
CY should enter:
- `ready_degraded`
- `snapshot chain mismatch: block[62] state_root does not match replayed state`

### Step 5 repro (routing guard labels)
Inspect CY logs:
```
type "$RUN_DIR/logs/pwmd-cy.log"
```
Expected: routing guard logs mention `shard=CY` / `shard=DO`.
Actual (in this run): `tx routing guard: shard=A ...`.

## Cleanup
Processes stopped: **yes**.

