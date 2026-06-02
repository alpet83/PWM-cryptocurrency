# V5-2 Slice 4 Review: ClaimIPv4Batch Tx Shape

## 1. Scope recap

Reviewed V5-2 slice4 after coding and testing PASS for the introduction of `TxBody::ClaimIPv4Batch` across:

- [crates/pwm-core/src/tx.rs](../../crates/pwm-core/src/tx.rs)
- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs)
- supporting API/lifecycle/snapshot adapters in `pwmd`

Claimed scope was:

- add a new `ClaimIPv4Batch { phase, batch_root, registry_sig }` transaction shape;
- keep it distinct from retired ClaimTx;
- provide deterministic signing/serde;
- reject invalid shape in `validate_tx_shape`;
- keep full apply logic deferred to V5-5.

## 2. Requirements fit

The slice matches its intended boundary.

- `ClaimIPv4Batch` is a distinct tx kind and is not overloaded onto the retired ClaimTx path.
- Signing and JSON round-trip coverage are present in `pwm-core` tests.
- `validate_tx_shape` rejects the zero-signature placeholder path used as the current invalid-shape stub.
- Active state apply returns `UnsupportedTxKind`, which is consistent with the ticket and plan saying on-chain handling belongs to V5-5.
- `pwmd` API/lifecycle/snapshot adapters know the new tx kind as `claim_ipv4_batch`.

## 3. Style and module shape

This is a clean slice.

The new tx variant is added locally in the expected tx/state surfaces, and the unsupported-apply boundary is explicit rather than silently half-implemented. That is the right shape for a staged rollout.

### Wire JSON / u128

Not applicable.

`ClaimIPv4Batch` carries `phase: u8`, `batch_root: [u8; 32]`, and `registry_sig: [u8; 64]`. This slice does not introduce new peer-facing `u128` fields or JSON large-integer hazards.

## 4. Safety

No blocking safety findings for this slice.

The explicit `UnsupportedTxKind` apply path is preferable to accidental partial execution before registry verification exists.

## 5. Tests

Evidence reviewed:

- coding handoff in [tasks/done/20260524-v5-s2-slice4-ipv4-batch.json](../../tasks/done/20260524-v5-s2-slice4-ipv4-batch.json)
- testing handoff in [tasks/done/20260524-v5-s2-slice4-ipv4-batch-testing.json](../../tasks/done/20260524-v5-s2-slice4-ipv4-batch-testing.json)
- commit `4df1e02`
- targeted tests reported by pwm-testing:
  - `tx::tests::claim_ipv4_batch_signing_json`
  - `tx::tests::claim_ipv4_batch_rejects_zero_registry_sig`

The evidence is sufficient for this review role.

## 6. Verdict

Approve with nits.

Non-blocking note: when V5-5 lands, keep the tx-kind string `claim_ipv4_batch` stable across API/log/snapshot surfaces so operational tooling does not see needless naming churn.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260524-v5-s2-slice4-ipv4-batch-review.md
token_usage: { "source": "estimate", "input": 13000, "output": 1600, "total": 14600, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s2-slice4-ipv4-batch-review.md'
git add 'tasks/20260524-v5-s2-slice4-ipv4-batch-review.json'
git commit -m 'docs(v5-2): add slice4 review gate report'
```