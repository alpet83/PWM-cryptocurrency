# Review: wire serde hardening — U128Visitor and envelope shape guards (fc16364)

- **date:** 2026-07-03
- **ticket:** `20260703-wire-serde-hardening-review`
- **coding_ticket:** `20260703-wire-serde-hardening`
- **commit:** `fc16364e31959320eceb5c6bb8324419dd0b6517`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `crates/pwm-core/src/ser_json_u128.rs`, `crates/pwm-core/src/tx.rs` (`validate_tx_shape_inner`)

---

## 1. Scope recap

Coding ticket `20260703-wire-serde-hardening` closes two wire-scope gaps from [`20260702-wire-rpc-security-scope.md`](20260702-wire-rpc-security-scope.md):

1. **U128Visitor** — explicit signed-integer handlers with precise negative rejection; `visit_u128` passthrough; no `visit_f64`.
2. **Envelope shape guards** — reject `import_fee` / `import_provenance` on non-`Import` bodies and `burn_purpose` on non-`BurnMark` bodies via `PolicySchemaInvalid`.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `visit_i64` / `visit_i128` reject negatives with precise error | **PASS** | `u128::try_from(v).map_err(|_| E::custom("negative integer is invalid for u128"))` (`ser_json_u128.rs:57–63`). Test `neg_int_rejects_precise` asserts message on `{"amount":-1}` (`:85–91`). |
| 2 | `visit_u128` passthrough | **PASS** | `visit_u128` returns `Ok(v)` (`:69–71`). |
| 3 | `visit_f64` **not** implemented | **PASS** | No `visit_f64` in `U128Visitor` or `pwm-core` crate (grep). Float JSON numbers still hit serde default invalid-type path. |
| 4 | `import_fee` / `import_provenance` rejected on Transfer, Stake, Export, Init, Policy | **PASS** | Catch-all after valid `Import` arm: `_ if tx.import_fee.is_some() \|\| tx.import_provenance.is_some()` → `PolicySchemaInvalid` (`tx.rs:667–668`). Runs before `Policy` arm, so Policy txs with envelope fields are rejected. Test `import_fee_rejects_non_import` covers Transfer + `import_fee` (`:1048–1054`). |
| 5 | `burn_purpose` rejected on non-BurnMark bodies | **PASS** | After `BurnMark` arm (which requires normalized purpose), `_ if tx.burn_purpose.is_some()` → `PolicySchemaInvalid` (`:656`). Catches Transfer, Stake, Import, Policy, etc. Test `burn_purpose_rejects_non_burn` (`:1057–1063`). |
| 6 | Existing positive u128 paths (string, u64) unchanged | **PASS** | `visit_str` / `visit_string` / `visit_u64` unchanged (`ser_json_u128.rs:48–67`). `u128_str_max_ok` still passes max decimal string (`:94–100`). |

---

## 3. Guard ordering (correctness)

Match order in `validate_tx_shape_inner` is intentional and sound:

```text
BurnMark     → validate burn_purpose (required)
_ burn_purpose present → reject (non-BurnMark, incl. Import)
Import       → MIN_IMPORT_FEE check; import_provenance allowed
_ import_fee/import_provenance → reject (non-Import)
Policy       → policy-specific rules
```

Placing import-field rejection **after** the `Import` arm preserves valid Import txs with `import_provenance` and fee checks. Placing burn_purpose rejection **after** `BurnMark` preserves required purpose on burns while blocking envelope smuggling on other bodies.

---

## 4. Wire JSON / u128

| Input | Behavior (post-slice) |
|-------|-------------------------|
| Decimal string | `visit_str` → `parse::<u128>()` (unchanged) |
| JSON u64 number | `visit_u64` → cast (unchanged) |
| JSON negative int | `visit_i64` → `"negative integer is invalid for u128"` |
| JSON float | No `visit_f64` → generic invalid type |
| JSON u128 (if serde emits) | `visit_u128` → accepted |

`expecting()` text still reads `"a decimal u128 string or u64 number"` — accurate for happy path; signed/u128 numeric acceptance is a superset (nit only).

---

## 5. Safety

| Risk (wire scope) | Assessment |
|-------------------|------------|
| Negative amount smuggling via JSON number | **Mitigated** — explicit rejection with stable error string. |
| Unsigned envelope fields on wrong `TxBody` | **Mitigated** — shape guards return `PolicySchemaInvalid` before policy logic runs. |
| Float truncation to amount | **Unchanged safe** — no `visit_f64`. |
| Import with `burn_purpose` | **Rejected** at burn_purpose catch-all before Import arm. |

---

## 6. Tests

Coding ticket ran `cargo test -p pwm-core --lib` (209 passed). New regressions:

- `neg_int_rejects_precise`
- `import_fee_rejects_non_import`
- `burn_purpose_rejects_non_burn`

Existing `import_fee_rejects_below_minimum` unchanged.

**Gaps (non-blocking):** no unit test for `import_provenance` on non-Import; no serde test asserting float rejection; no explicit `visit_i128` JSON fixture (serde_json typically routes small negatives through `visit_i64`).

---

## 7. BLOCKERs

None.

---

## 8. Nits (non-blocking)

1. **NIT-1:** Add `import_provenance_rejects_non_import` mirroring the import_fee test.
2. **NIT-2:** Add serde test that `{"amount":1.5}` fails (documents float policy).
3. **NIT-3:** Optionally widen `expecting()` to mention non-negative integers.

---

## 9. Verdict

**Approve** — U128Visitor implements the required signed/u128 visitors without `visit_f64`; negative integers get a precise error; string and u64 paths are unchanged. Envelope shape guards correctly reject cross-body field smuggling with proper match ordering. Meets all ticket acceptance criteria.

---

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-wire-serde-hardening-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 12000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-wire-serde-hardening-review.md'
git commit -m 'docs(v7): wire serde hardening review — U128Visitor and shape guards (fc16364)'
```