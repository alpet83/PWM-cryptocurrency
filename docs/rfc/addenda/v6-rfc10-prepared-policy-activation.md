# RFC 10 Addendum (V6): Prepared policy activation storage

**Parent:** [10-wallet-file-format-v3.md](../10-wallet-file-format-v3.md)  
**Status:** Normative for MVP v6 (V6-7 implementation)  
**Depends on:** [ADR 0011](../../adr/0011-policy-activation-target.md)

## 1. Purpose

Operators preparing corporate accounts with dormant `routing.emergency_redirect` need a **signed, ready-to-broadcast** `ActivatePolicy` without re-entering secrets at incident time. V6 stores this in the wallet file per account.

## 2. Schema extension (additive, wallet schema v3)

Optional field on `accounts[]` entries:

```text
prepared_policy_activation: Option<PreparedPolicyActivation>

PreparedPolicyActivation {
  policy_id: string,              // stable policy id in account policy set
  activation_target_id_hex: string,  // 32-byte account id hex
  activation_target_pretty: string,
  signed_tx_b64: string,        // canonical serialized signed PolicyTx
  fee_pwm: u64,                   // MUST be 0 in V6
  expected_nonce: u64,            // sender nonce at preparation time
  prepared_at_unix_sec: u64,
  expires_note: null,           // reserved; V6: no auto-expiry
}
```

Encrypted payload MAY mirror the same structure inside AEAD JSON for `encrypted` mode wallets.

## 3. Invariants

- Loader MUST reject `prepared_policy_activation` where `fee_pwm != 0`.
- `activation_target_id_hex` MUST match `rescue_address` in live account state when activation is broadcast (operator responsibility; node rejects mismatch per ADR 0011).
- Replacing prepared activation: `tx-init` or `wallet account prepare-activation` overwrites prior entry deterministically.

## 4. CLI flags (V6-7)

- Default: save into wallet `accounts[]` entry.
- `--save-activation-tx <path>`: additionally write standalone signed tx file (format: existing pwm-cli signed tx envelope).

## 5. Non-goals (V6)

- Multi-policy prepared queue (one prepared activation per account in V6).
- Auto-broadcast on incident.
- TUI orchestration beyond display + path hint.
