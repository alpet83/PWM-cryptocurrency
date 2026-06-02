# V5-5 Slice 1 Review: ClaimIPv4Batch Apply Happy Path

## 1. Scope recap

Reviewed coding output for commit `f016074` (post-coding gate, pre-testing gate) against V5-5 slice1 scope:

- replace `UnsupportedTxKind` branch for `TxBody::ClaimIPv4Batch` with real apply logic;
- verify registry signature over canonical claim message;
- lookup phase in `GenCfg.ipv4_claim_phases`;
- credit claimant account and set `ipv4_claimed_phase`;
- include happy-path `claim_` test;
- no `pwmd`/CLI expansion in this slice.

Scope artifacts checked:

- `tasks/done/20260524-v5-s5-slice1-apply-happy.json`
- `tasks/introductory/20260524-v5-s5-ipv4-claim-onchain.md`
- `crates/pwm-core/src/state.rs`

## 2. Requirements fit

Slice1 requirements are satisfied.

- Canonical message tag and preimage are implemented in `state.rs` as:
  - `PWM/IPV4/CLAIM/V1 || phase_u8 || batch_root || claimant_account_id`
- Phase lookup uses `gen_cfg.ipv4_claim_phases` and fails cleanly when not found.
- Registry key is resolved from on-chain state account (`registry_address`) and signature is verified with `crypto::verify`.
- Claimant signer semantics are preserved: transaction signer is claimant (via `computed_account_id`), while `registry_sig` is a separate authorization proof.
- Apply path mutates claimant state as required on success:
  - `balance_pwm += allocation` (saturating add),
  - `ipv4_claimed_phase = Some(phase)`,
  - `nonce += 1`.
- Happy-path test `claim_ipv4_batch_happy_apply` is present and passes.
- Diff scope is limited to `crates/pwm-core/src/state.rs` (+ ticket metadata), no CLI/pwmd scope creep.

## 3. Style and module shape

No naming-policy violations detected in touched production/test symbols.

Evidence:

- `python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs`
- Output: `violations: []`.

Module shape is narrow and coherent for slice1; no large unrelated blob growth outside intended `ClaimIPv4Batch` path and local helpers.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

No blocking safety findings in reviewed scope.

- Signature authorization is explicit and bound to claimant id, reducing replay ambiguity across claimants.
- Registry account must exist and be initialized to provide signing key; otherwise apply rejects.
- Double-claim guard exists via `ipv4_claimed_phase.is_some()`.

Non-blocking note for slice2/reject matrix hardening: current error code mapping uses generic `PolicySchemaInvalid`/`PolicyDenied` for some reject modes; acceptable for slice1 per scope, but slice2 tests should lock stable reject taxonomy.

## 5. Tests

Executed validation commands:

- `python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs` -> PASS (0 violations)
- `cargo test -p pwm-core claim_ --lib` -> PASS (6 passed, 0 failed)

Observed passing test includes `state::tests::claim_ipv4_batch_happy_apply`.

## 6. Verdict

Approve with nits.

Slice1 goal is met and ready to proceed to testing gate. Nits are non-blocking and mainly about tightening reject-code specificity in slice2.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260524-v5-s5-slice1-apply-happy-review.md
token_usage: { "source": "estimate", "input": 16000, "output": 2200, "total": 18200, "confidence": "medium" }
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s5-slice1-apply-happy-review.md'
git add 'tasks/20260524-v5-s5-slice1-apply-happy-review.json'
git commit -m 'docs(v5-5): add slice1 claim ipv4 apply review gate'
```