# Code review: peers list YAML bootstrap (`--peers-list`, `peer_list.rs`)

**Ticket:** `tasks/20260518-slice-peers-yaml-bootstrap.json`  
**Date:** 2026-05-18  
**Reviewer agent:** pwm-review

## 1. Scope recap

The slice targets static bootstrap peers for multi-node dev/lab runs: optional `--peers-list <PATH>`, implicit default `<state_root>/peers.yaml` when that file exists, YAML shape `peers: ["host:port"]`, union with `--transport-peer-seed` preserving stable merge order and deduplicating by `SocketAddr`, removal of the node's own `peer_listen` from seeds, and writing the effective list back after a successful `run_with` in `main.rs`. New module `crates/pwmd/src/peer_list.rs` centralizes load/merge/filter/save.

## 2. Requirements fit

- **CLI and default path:** `--peers-list` is documented in help; default file is used only when present (`pick_peer_file`), matching "if exists" semantics.
- **Merge and dedupe:** `merge_peer_seeds` walks file peers then CLI seeds, keeping first-seen order for uniqueness -- aligned with the stated merge behavior and covered by a unit test.
- **Self filter:** `drop_self_seed` compares `SocketAddr` to the resolved `peer_listen` -- correct for exact matches; no hostname or alternate representation normalization.
- **Persistence:** After `run_with` returns `Ok`, `save_peer_file` writes to the same `peer_list_path` that was chosen at startup (explicit path or existing default). If neither an explicit path nor a pre-existing default file was selected, **no file is created**; operators who rely only on `--transport-peer-seed` will not get an on-disk snapshot unless they already use a peer file path. This matches a narrow reading of "same file" but is looser than "always persist somewhere under state_root."
- **Errors:** Missing/unreadable YAML for a **chosen** path fails fast at startup with a clear message; save failures log and exit with code 1.

## 3. Style and module shape

- New logic lives in `peer_list.rs` instead of inflating `main.rs` beyond wiring -- appropriate micro-modularity for this slice.
- `peer_list.rs` has a short English `//!` banner; user-facing error strings are clear and include path context.
- **`python scripts/check_rust_fn_name_segments.py`** on `crates/pwmd/src/peer_list.rs` and `crates/pwmd/src/main.rs` reported **no violations** (production max 4 segments, tests max 5).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- **Trust boundary:** Path is operator-controlled (`--peers-list` / state dir). No new network-facing serde types; local YAML only.
- **Panics:** No new `unwrap` in the peer-list hot path beyond existing test expectations; startup uses `unwrap_or_else` with `exit(2)` for load errors.
- **Resource limits:** Full file is read into memory for YAML parse -- acceptable for expected small operator files; unusually large `peers.yaml` could be heavy but is a local configuration footgun, not a remote DoS vector.
- **Shutdown semantics:** Persistence runs only after `run_with` completes successfully. Runtime failures skip the save path -- consistent with "after successful graceful completion," not "always on any exit."

## 5. Tests

- **Unit tests** in `peer_list.rs` cover merge order/dedupe, self-removal with duplicates, malformed YAML error shape, and default file pick when `peers.yaml` exists.
- **pwm-testing** reported full `cargo test -p pwmd` pass (351 tests) in their notes; integration coverage of main's save path may still be indirect -- acceptable for this slice but end-to-end "write file on shutdown" is not strongly asserted in-repo from this review alone.

## 6. Verdict

**PASS_WITH_NITS**

Non-blocking observations for follow-up or documentation (not merge blockers):

1. **Persistence scope:** When no peer file path is active (no `--peers-list` and no pre-existing `<state_root>/peers.yaml`), the effective seed set is **not** written to disk. Product/docs should state this so operators are not surprised when using CLI-only seeds.
2. **Self filter and address forms:** Deduplication of "self" is literal `SocketAddr` equality (e.g. `127.0.0.1:13030` vs `0.0.0.0:13030` will not match). Worth a one-line operator note in help or ops docs if not already elsewhere.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260518-peers-list-yaml-slice.md
token_usage:
  source: estimate
  input: 12000
  output: 3200
  total: 15200
  confidence: medium
```

## 8. Glossary

Not a sprint-final wrap-up review. `docs/GLOSSARY.md` was not updated as part of this review artifact.
