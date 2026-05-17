# Review: MVP V4-5 CLI/TUI/wallet operator policy path

**Ticket:** `tasks/20260517-v4-sprint5-cli-tui.json`  
**Info:** `tasks/20260517-v4-sprint5-cli-tui-info.json`  
**Reviewer:** `pwm-review` (independent)  
**Date:** 2026-05-17

## 1. Scope recap

Slice targets **operator exposure** of the V4 policy runtime: CLI for `tx-init` V4 extension and `tx-policy-*`, wallet/rescue cosign path for emergency routing activation, **inspection** of policy-related account state in CLI docs, TUI, and `pwmd` API docs, plus a **pwmd crate version marker** and lockfile alignment. Explicit out of scope: full governance, org registry, policy DSL. Checklist anchors: `docs/plans/mvp_v4.md` Sprint V4-5, `docs/rfc/10-wallet-file-format-v3.md`, operator docs `docs/pwm-cli.md`, `docs/pwm-tui.md`, `docs/pwmd.md`.

## 2. Requirements fit

**Met (by code + docs read):**

- CLI surfaces `tx-policy-set`, `tx-policy-activate`, `tx-policy-deactivate`, and V4-capable `tx-init` aligned with `cli_cmd.rs` / `cli_dispatch.rs` / `cmd_tx.rs` and documented in `docs/pwm-cli.md`.
- Signing path reuses **`load_tx_signer_source`** and adds **`load_wallet_account_signer`** for rescue derivation index (`signer.rs`), consistent with existing wallet/multi-account patterns.
- Rescue cosign is **`Cosignature` with `CosignRole::Rescue`** only in CLI (`append_rescue_cosign`), not org/witness flows.
- **Emergency gating:** `run_tx_policy_activate` appends rescue cosign **only** when the selected policy id equals `PolicyKind::RoutingEmergencyRedirect` and rejects rescue flags for other policies — matches “minimal mutation” and avoids implying broad multisig.
- TUI **`AcctRow`** and `poll_snapshot` JSON parsing add **optional/tolerant** fields (`rescue_address`, `active_policies`, `dormant_policies`, `finalized`, owner metadata); missing keys degrade to empty/false/0 — backward-friendly for older nodes.
- **`AcctOut`** in `pwmd` adds V4 inspection fields with `serde(default)` / `skip_serializing_if` patterns — additive JSON contract.
- **`common.rs`** maps extended `TxError::Policy*` variants to stable **`E_POLICY_*`** codes documented in `docs/pwmd.md`.
- **Tests:** `crates/pwm-cli/src/tests/mod.rs` includes parse coverage for V4 `tx-init`, policy set/activate/deactivate, rescue flags; `crates/pwm-cli/tests/cli_smoke.rs` exercises `--help` for new subcommands. Prior `pwm-testing` delegations in the ticket are recorded as PASS (not re-executed in this review session).

**Gaps / partial coverage (non-blocking):**

- No automated test here asserts **end-to-end** cosign bytes vs node acceptance (relies on `pwm-core` signing-message tests and node validation).

**Documentation follow-up (2026-05-17, post–PASS_WITH_NITS):** orchestrator closed doc nits — see **Addendum §10**.

## 3. Style and module shape

- New/changed modules keep **`//!` English banners** where files are non-trivial (`cli_cmd.rs`, `cli_dispatch.rs`, `cmd_tx.rs`, `signer.rs`, `account_view.rs`, `models.rs`).
- **`python scripts/check_entity_name_segments.py`** on the listed paths: **no violations** (prod max 4 segments, test max 5).
- Dispatch remains thin; policy logic lives in `cmd_tx.rs` — consistent with existing `run_tx_*` layout.

### Wire JSON / u128

**Scope:** REST **`POST /v1/tx`** accepts **`SignedTx`** JSON (client–node), not peer sync wire in this slice.

**Assessment:**

- `TxBody::Policy { fee: u128 }` uses **`#[serde(with = "crate::ser_json_u128")]`** in `pwm-core` — **serde_json-safe** for policy fees.
- Account REST responses: balances and policy-adjacent scalars in **`AcctOut`** use **string** or plain **u16/u8/bool** as appropriate; no new raw `u128` fields without encoding were introduced in the reviewed `AcctOut` block.
- **Peer/framed JSON (`PeerWireMsg`, catch-up blocks):** not changed in the artifacts reviewed — **no new `derive`-only `u128` on peer payloads** in this slice.

## 4. Safety

- **Trust boundaries:** CLI still posts only to operator-configured `--rpc` / `PWM_RPC`; rescue credentials are optional separate paths — same class of risk as `--master` override (expected for dev/ops).
- **Cosign preimage:** `append_rescue_cosign` signs **`tx.signing_message()`**; the canonical message in `pwm-core` encodes the **Policy** body (tag 9) and **does not** fold in `cosigns`, so rescue signs the **same intent** as the primary signature — consistent with a standard “cosign over tx hash / signing message” pattern.
- **Panic/unwrap:** hot paths use `exit_user_error` / `Result` patterns consistent with surrounding `cmd_tx`; no new obvious `unwrap` hotspots reviewed beyond existing CLI style.
- **Role enum surface:** Core still defines `Organization` / `Witness` roles, but CLI only emits **Rescue** — low risk of this slice implying member registry/governance.

## 5. Tests

- **Strong:** CLI **parse tests** for V4 init and policy commands; **subprocess help smoke** for `tx-policy-*` and `tx-init`.
- **Missing (acceptable for slice or follow-up):** integration test firing **`tx-policy-activate`** against a live/mock node with rescue cosign verification; behavioral test that **non-emergency** + rescue flags exits before RPC (currently user-visible string only).

## 6. Verdict (initial slice review)

**PASS_WITH_NITS** — prioritized doc nits were filed; see §10.

**Final verdict (after doc fixes):** **PASS** — all listed nits verified closed in workspace docs.

## 7. Participation / token estimate (orchestrator)

**Final re-review participation (incremental verify only):**

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/20260517-v4-sprint5-cli-tui-review.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 3500,
    "confidence": "medium"
  }
}
```

**Initial review (full slice)** token estimate remains ~12000 (see git history / prior orchestrator notes if needed).

## 8. Version marker / `Cargo.lock`

- **`crates/pwmd/Cargo.toml`** version **0.1.55** matches **`Cargo.lock`** entry for package `pwmd` — justified **crate semver bump** for an API-facing slice; no protocol version churn identified in touched files.

## 9. Glossary

**GLOSSARY.md: без изменений** (подслайсовое ревью; новый обязательный жаргон для глоссария не заведён отдельно от уже существующих терминов policy/V4).

## 10. Addendum — final re-review (doc nits)

**Date:** 2026-05-17 (same ticket; PASS_WITH_NITS doc pass)

**Verification (read-only, targeted):**

| Prior nit | Status |
|-----------|--------|
| `pwm-cli.md` `tx-burn-mark`: `--mark-amount` type vs Rust `u32` | **Closed** — documented as `--mark-amount <u32>` (line ~307). |
| Clarify rescue cosign only for `routing.emergency_redirect` | **Closed** — English paragraph under `tx-policy-activate`: flags only when activating emergency redirect; rejects on other policies; governance wording explicit. |
| `pwm-tui.md` `poll_data` bullets vs policy/rescue fields | **Closed** — step 4 `detail_line` lists `active_policies`, `dormant_policies`, `finalized`, `rescue`, owner metadata; intro already mentions these facets. |
| `--rescue-wallet` without `--rescue-account-index` | **Closed** — same paragraph warns default-signer fallback and recommends explicit index for production. |

**Optional micro-precision (non-blocking):** step 3 in `pwm-tui.md` still summarizes `rows` as the older column list (`id`, balance, marks, …) without mentioning that each row’s JSON also hydrates V4 fields used for `detail_line`; behavior is documented in step 4, so operators are not misled.

---

**One-line verdict (quote, final):** `PASS`
