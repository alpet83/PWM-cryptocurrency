# Sprint 14 Slice 24 TUI CY->DO Domain Mismatch Testing

Date: 2026-04-29

## Verdict

Reproduced. This is a wallet data / validation bug, not the new strict recipient-init gate.

`tmp/genesis.yaml` is a schema v3 encrypted wallet whose active account is CY, but its encrypted signing payload belongs to the DO account. TUI and CLI both trust the active account header for `domain_u16` and `derivation_index`, then sign with the single decrypted payload key. For the active CY derivation index `105053`, that payload key computes sender `domain_hi=0xDB`; the CY node correctly rejects it before recipient policy is relevant.

Minimal fix: when loading/unlocking encrypted schema v3 wallets, verify that decrypted `signing_key_hex` plus the active `derivation_index` computes the active `account_id_hex`. Reject the wallet with a clear error if it does not match. Longer term, schema v3 multi-account encrypted wallets need per-account signing material or a master seed payload so switching accounts can derive the selected account's key.

## Reproduction

Wallet metadata:

```text
PWM_WALLET_PASSPHRASE=1234 ./target/debug/pwm wallet show --wallet tmp/genesis.yaml
schema_version 3
wallet_mode encrypted
derivation_index 105053
domain_u16 11515
account_id_hex 2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e
id_pretty pwm1-CY/FB-f1E1D7001-td108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e
```

Wallet accounts:

```text
./target/debug/pwm wallet account list --wallet tmp/genesis.yaml
* id_hex=2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e ... derivation_index=105053
  id_hex=32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5 ... derivation_index=583600
```

Built a pwmd genesis JSON from the supplied wallet/passphrase:

```text
env PWM_WALLET_PASSPHRASE=1234 PWM_GENESIS_PASSPHRASE=1234 \
  ./target/debug/pwm genesis-build \
  --wallet tmp/genesis.yaml \
  --out tmp/slice24-cy-do/genesis.json \
  --premine-bal 1000000
```

Generated accounts:

```text
2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e der_idx=105053 bal=1000000 hi=0x2C
32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5 der_idx=583600 bal=1000000 hi=0x32
9f77a572d7942e8a447a0f560c6e4423ac10fe80b5ad4154e98c1fed0a576609 der_idx=1 bal=0 hi=0x9F
```

Started a CY node:

```text
P:\opt\docker\PWM-cryptocurrency\target\debug\pwmd.exe \
  --listen 127.0.0.1:3130 \
  --domain-hi 0x2C \
  --network-id slice24-test \
  --cluster-id cy \
  --node-id cy-1 \
  --genesis-file tmp\slice24-cy-do\genesis.json \
  --genesis-passphrase 1234 \
  --state-root tmp\slice24-cy-do\state-cy \
  --log-file off
```

Node status and accounts:

```json
{"phase":"ready","ready":true,"shard":"CY","state_namespace":"domain-hi-0x2c","roaming_relay_mode":"manual_handoff_required"}
```

```text
GET /v1/accounts
2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e initialized=true nonce=0 balance=1000000
32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5 initialized=true nonce=0 balance=1000000
```

CLI path that mirrors TUI F6 cross-domain submit:

```text
PWM_WALLET_PASSPHRASE=1234 PWM_CLI_RPC_TIMEOUT_MS=5000 \
  ./target/debug/pwm --rpc http://127.0.0.1:3130 tx-send \
  --wallet tmp/genesis.yaml \
  --to 32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5 \
  --amount 1 \
  --fee 1
```

Result:

```text
exit_code=2
stderr:
cross-domain send failed with HTTP 409 Conflict. details: tx sender domain_hi=0xDB does not match node domain_hi=0x2C
```

## Source of 0xDB

A local probe decrypted the wallet payload with passphrase `1234` without printing secrets, then recomputed account IDs from the decrypted signing key:

```text
active_id=2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e
account idx=105053 declared_hi=0x2C declared_id=2cfb...ae5e computed_from_payload=db22039c2d080659a219e423c1be0a8d5cc9142627f5e1c37eeb5df5b2ecb94a computed_hi=0xDB matches_declared=false
computed_pretty=pwm1-$DB22!-f039C2D08-t0659a219e423c1be0a8d5cc9142627f5e1c37eeb5df5b2ecb94a
account idx=583600 declared_hi=0x32 declared_id=32ec...c1c5 computed_from_payload=32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5 computed_hi=0x32 matches_declared=true
```

So `0xDB` is not an allowed cluster/domain code from config. It is the first byte of the account ID computed from:

- decrypted wallet signing key, which matches the DO account at derivation index `583600`
- active CY derivation index `105053`

That mixed key/index pair creates a synthetic account ID `db22039c...`, which the CY node rejects.

## Code Path

TUI wallet loading chooses the active v3 account header and decrypts one wallet payload:

```text
crates/pwm-core/src/wallet_read.rs
parse_wallet_read_v3_header(): active_account_id_hex -> active.derivation_index/domain_u16/account_id_hex
```

```text
crates/pwm-tui/src/main.rs
load_wallet_identity(): account_id = wallet.account_id_human; signing_key = decrypted payload signing_key_hex
```

TUI F6 does not use the selected owner row as sender for wallet mode:

```text
crates/pwm-tui/src/main.rs
f6_send_form_for_identity(): IdentitySource::Wallet(w) => w.account_id_human.clone()
```

The submit path enforces that `from` equals the active wallet identity, then signs with the decrypted key and active header metadata:

```text
crates/pwm-tui/src/main.rs
signing_material_for_sender(): returns (w.signing_key, w.domain, w.derivation_index)
submit_roaming_intent(): SignedTx::sign_body(&sk, dom, idx, nonce, TxBody::Export { ... })
```

The node rejects based on the signed tx's computed sender, before recipient checks:

```text
crates/pwmd/src/tx_policy.rs
sender = tx.computed_account_id()
if sender_hi != local_domain_hi { HTTP 409 ... }
```

## Relatedness

Not caused by the strict recipient-init gate. In the repro both CY and DO accounts are initialized in the source node account list, and the failure is `HTTP 409` from the local sender-domain guard.

There is still an older TUI account-selection limitation: in wallet mode, selecting a different row in the Owner panel does not change `from`; TUI always uses the wallet active account. That is a separate UX/selection bug, but it does not explain `0xDB` here because the active account is already CY and the bad high byte comes from mismatched encrypted signing material.

## Cleanup

The spawned CY `pwmd` process `bfb4102d-9b26-4027-a505-bbd36b301465` was killed after the reproduction. I did not kill other pre-existing `pwmd` / `pwm-tui` processes visible on the host.
