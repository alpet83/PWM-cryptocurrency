# Peer compat and wire stabilization — final gate review (2026-05-08)

## Scope recap

- **Ticket:** `tasks/20260508-peer-compat-and-wire-stabilization.json`
- **Coding:** `be89b30` — wire `u128` canonicalization, handshake guard matrix, sync lane reason split, tests and notes
- **Ticket trace:** `5dbdc49`
- **Testing report:** `1719627` → `docs/reviews/20260508-peer-compat-wire-stabilization-testing.md` (**PARTIAL**)

Claimed goals: remove `serde_json` large-integer decode failures on peer wire; formalize early same-shard vs inter-shard handling with explicit reject and drop reasons; preserve legacy decode paths where feasible.

## Requirements fit

**Wire `u128` (canonical hex)**

- Emit path uses fixed-prefix lowercase hex (`0x` + `{:x}`), which is unambiguous and avoids JSON number precision loss for values above `u64`.
- Decode path accepts `0x` / `0X` hex, legacy decimal strings, and non-negative integer visitors (`u64` / `i64`), matching the stated backward-compatibility story.
- **Caveat (documented elsewhere, still true):** payload fields that arrive strictly as JSON numbers cannot express `u128` values beyond `u64::MAX`; interop for large magnitudes depends on string form. This is an inherent JSON limitation, not a regression from this slice.
- Unit coverage in `wire_decode` exercises legacy decimal, hex without normalizing whether tests use `0x` prefix on decode for account views (one test uses bare `0xff...` — still valid because the parser treats `0x`-prefixed branch; bare hex with `from_str_radix` is used when `0x` stripped — the test `dec_acct_u128_hex_ok` uses `"0xffffffffffffffffffffffffffffffff"` which exercises the `0x` branch; cross-shard test uses same). Encoding test asserts serialized amount string matches full-width hex for `u128::MAX`.

**Handshake / classification**

- After successful `validate_node_hello`, code uses `classify_peer` then applies **Native (same-shard)** `cluster_id` equality against runtime `expected_cluster_id` — addresses accidental cross-cluster pairing on the same `domain_hi`.
- When `sync_profile` is present but capabilities do not satisfy `supports_sync_v1()`, rejection reason is split into `same_shard_sync_profile_incompatible` vs `inter_shard_sync_profile_incompatible`, improving observability.
- Inbound `HelloAck` on rejection now carries the concrete string reason instead of a single generic label — matches the ticket narrative.
- Dedicated unit tests in `incoming_hello.rs` cover cluster mismatch and both branches of sync-profile incompatibility.

**Steady-state sync routing**

- `route_sync_stub` applies ordering: shard header match → inter-shard guard (`inter_shard_sync_forbidden`) → same-shard full-v1 guard (`same_shard_profile_mismatch`) before processing. This matches the intended “early classification” split for sync frames and sync-tx traffic (drops are metered separately).

**Gaps vs ideal “full green”**

- End-to-end Wave A remains **red** on **tip / last-epoch hash divergence** between nodes (`pwm-testing` PARTIAL). Per the testing report, this is **not** ascribed to wire decode / `u128` errors (no matching log lines in the captured run). It is a **residual integration / harness or chain-state** concern, outside the narrow wire-stabilization proof but relevant to calling the full slice “complete” for operators.

## Style and module shape

- Touched areas keep English module/docs tone; `wire_serde` remains a small focused helper.
- `python scripts/check_rust_fn_name_segments.py` on `wire_serde.rs`, `incoming_hello.rs`, `wire_decode.rs`: **no violations** (policy prod ≤4 / test ≤5 segments).
- No new large blobs in façade `lib.rs` for this slice.

## Safety

- Reject paths set peer error state and structured logging; bridge trust refusal logic for Native peers is unchanged in intent, only ordered after the new guards.
- `u128` parsing rejects negatives on the `i64` path with a custom error; string parse failures surface as deserialization errors → existing wire decode failure handling.
- No new unchecked trust boundary: classification still rests on validated hello + configured `cluster_id` / bridge expectations.

## Tests

- **Strong:** expanded `wire_decode` matrix for `AccountViews` / `CrossShardFacts`, negative `amount`, canonical encode emission; `incoming_hello` guards.
- **`pwm-testing`:** `wire_decode`, `peer_session`, `tip_divergence` filters **PASS**; Wave A **fails assertion** on hash equality — see residual risks.

## Verdict

**Approve with nits** (implementation satisfies the slice’s wire + handshake/matrix goals).

**Overall conveyor gate:** **PARTIAL** — blocked only on Wave A scenario acceptance, not on unit-level or log-level evidence that this slice’s `u128` / handshake fixes are wrong.

### Must-fix (for this slice’s stated coding goals)

- **None.** No issue found that would require reopening `be89b30` before merge on wire/handshake correctness alone.

### Follow-ups (prioritized)

1. **Wave A / same-shard parity:** investigate deterministic chain tips and epoch hashes in the harness (seeds, timing, divergence guard interaction) — owns the remaining **PARTIAL** bit.
2. **Operator visibility:** optionally raise peer/trace log level in Wave runs to validate new reject and `HelloAck.reason` strings under load (testing note: short logs at default verbosity).
3. **Interop documentation:** keep explicit that large `u128` on JSON wire must use string form; numeric JSON remains `u64`-bounded at the serde layer.
4. **Broader compat matrix:** `protocol_version` / `tx_features` policy remains as before; future ticket if rollout needs finer negotiation.

---

## Participation / token estimate

- `agent`: `pwm-review`
- `result`: `PARTIAL` (implementation gate: approve with nits; **full slice** gate: PARTIAL via Wave A per `pwm-testing`)
- `artifacts`:
  - `docs/reviews/20260508-peer-compat-wire-stabilization-final-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 14000, "confidence": "low" }`

**One-line verdict for merge quote:** Approve merging `be89b30` with nits; overall slice PARTIAL until Wave A hash parity is tracked to closure.
