# Independent review: MVP V3 Sprint 2 — Epoch Snapshot schema + replay gate

**Date:** 2026-05-16  
**Ticket:** `tasks/20260516-v3-sprint2-snapshot-replay.json`  
**Scope:** `crates/pwmd/src/snapshot/{epoch,incremental,io}.rs`, `docs/guide-node-storage-and-snapshot.md`, `docs/runbook-store-snapshots.md`, `docs/plans/mvp_v3.md` (plan text; frontmatter status not re-baselined by this review).

## 1. Scope recap

Sprint V3-2 (per `docs/plans/mvp_v3.md`) targets an explicit **Epoch Snapshot manifest** compatibility contract (`pwm-epochs-manifest.json` / `schema_v`), **no** Bootstrap Snapshot / pruning / cleanup-chain runtime semantics, a **lightweight replay determinism gate**, and operator docs updates. The change set centralizes manifest schema acceptance (`EPOCH_MAN_SCHEMA_CUR`, `ensure_epoch_man_schema`), threads checks through epoch load paths and trusted validation, adds focused tests (`v3_replay_det_gate_ok`, `epoch_man_v*`, tail rejection), and documents the focused `cargo test` command.

## 2. Requirements fit

**Met:**

- Manifest schema is named and centralized; unsupported versions yield a single explicit error shape (`unsupported epoch manifest schema …`).
- v1 writers still emit `schema_v: 1` via `mk_manifest` / `write_manifest` precondition; behavior for valid v1 trees remains coherent with the prior “only 1 is supported” intent.
- Docs explicitly separate Epoch manifest `schema_v`, genesis `schema_version`, and snapshot wire `version`, and defer Bootstrap Snapshot semantics (aligned with V3-2 boundaries).

**Partial / wording gap vs plan:**

- The plan text speaks of a “migration table”; the implementation provides a **compatibility gate** (`ensure_epoch_man_schema`) rather than an explicit multi-version migration map. This is proportionate for “only v1 exists today,” but the **plan language overshoots** the delivered artifact unless “table” is read loosely as “contract entry point.”

## 3. Style and module shape

- New/edited snapshot sources carry appropriate `//!` banners; helpers are small and localized.
- **`pwm-testing` reports `check_entity_name_segments.py` clean** on the three touched snapshot files; no policy violations flagged there.
- Test naming stays within the test naming budget; production identifiers in the touched slice remain concise.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- Changes are local-disk snapshot/manifest validation and replay validation; no new trust boundaries on RPC or peer wire.
- Error paths return `Result` with strings; no new obvious panic surface in the reviewed paths beyond pre-existing `expect` in tests and a few internal invariants (e.g. non-empty line unwrap in epoch append) that predate this slice’s goal.

## 5. Tests

**Strengths:**

- `epoch_man_v1_ok` / `epoch_man_v2_err` and `epoch_man_*` in `epoch.rs` cover accept/reject.
- Epoch integration tests cover persistence paths and tail loads.

**Gaps / clarity:**

- `v3_replay_det_gate_ok` exercises **`replay_validate` / full replay on an in-memory `SnapshotData` fixture**, not JsonFile epoch manifest I/O. It is a meaningful **replay-path determinism** latch for CI/local focus, but it does **not** by itself prove manifest read/write compatibility. Operator docs now partially scope “what it catches”; a single sentence clarifying “no `epochs/` manifest round-trip in this test” would fully align expectations (see nits).

## 6. Findings (severity order)

1. **Low — documentation precision (gate scope):** The replay gate is valuable but **does not touch on-disk epoch manifest**; readers could infer broader coverage than exists. *Mitigation:* keep the gate, clarify in the guide that it targets the replay validation path, not manifest schema evolution I/O.

2. **Low — plan vs implementation vocabulary:** “Migration table” in `mvp_v3.md` is stronger than the delivered **`ensure_epoch_man_schema` gate**. Not a functional defect; adjust planning language or add a one-line “migration map deferred until v2 exists” note in a future doc pass.

3. **Low — informational reads without immediate schema gate:** `read_epoch_manifest` / `load_manifest` deserialize before `ensure_epoch_man_schema` runs at call sites. **All epoch consumers reviewed** call `ensure_epoch_man_schema` before trusting contents; the **drift-detection** branch in `load_snapshot_timed` may observe `canonical_h` without rejecting unknown `schema_v` first, but **later loads still reject**. Acceptable; optional hardening would fail fast for consistency of error ordering only.

4. **Mechanical (auto-closed in review pass):** `docs/runbook-store-snapshots.md` used a paragraph sign in a cross-reference; replaced with plain “раздел 3”. **`docs/guide-node-storage-and-snapshot.md`** gained an explicit note that `v3_replay_det_gate_ok` does not exercise `epochs/` manifest I/O; pair with `epoch_man_v` / epoch persistence tests when changing on-disk manifest format.

## 7. Verdict

**PASS_WITH_NITS** — implementation matches V3-2 intent; remaining items are doc/plan wording precision and optional fail-fast polish, not blockers.

---

## Participation / token estimate

```text
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/sprint-v3-2-snapshot-replay-review-20260516.md
token_usage: { "source": "estimate", "input": 28000, "output": 5200, "total": 33200, "confidence": "medium" }
```
