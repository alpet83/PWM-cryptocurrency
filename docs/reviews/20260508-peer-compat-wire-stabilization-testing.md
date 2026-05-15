# Peer compat and wire stabilization — testing report (2026-05-08)

## Scope

Validation after stabilization commits:

- `be89b30` — fix peer wire u128 transport and handshake guard routing
- `5dbdc49` — chore(task): record peer compat coding artifact

Ticket: `tasks/20260508-peer-compat-and-wire-stabilization.json`

## Preflight

- `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1`
- **PASS** — reported `target/debug` ~226 464 982 bytes (under 4096 MiB threshold).

## Targeted `cargo test` (pwmd)

| Command | Result |
|---------|--------|
| `cargo test -p pwmd wire_decode` | **PASS** — 10 tests |
| `cargo test -p pwmd peer_session` | **PASS** — 20 tests |
| `cargo test -p pwmd tip_divergence` | **PASS** — 4 tests |

## Wave A (`scripts/wave_a_same_shard_stop.py --keep-artifacts`)

- **Exit:** `1` (hash divergence gate; scenario assertion failure, not compile/runtime crash).
- **Artifacts:** `F:\Temp\pwm_wave_a_v7tsmppw\` (includes `wave-a-report.json`, `logs/node1.log`, `logs/node2.log`).

### `wire_decode_failed` / `u128 is not supported`

- Grepped both node logs for `wire_decode`, `u128 is not supported`, `protocol_error`: **no matches** (empty result).
- aligns with passing `wire_decode` unit tests (hex u128 + legacy paths).

### Peer close / reconnect / reason labels

- Captured logs are short (~3 KB each) and mostly **startup**, **seal height**, **tx routing**, **graceful shutdown** at `debug-stop-height`.
- No lines containing `disconnect`, `reconnect`, `hello`, or explicit peer-session close reasons at this log verbosity — **operator observation: not available from this run**; use raised `RUST_LOG` / peer trace flags if product owns richer diagnostics.

### Wave failure evidence (divergence, not wire)

From harness stderr / diagnostics:

```text
tip_hash_equal=False
last_epoch_hash_equal=False
nodeA.tip_hash=a8c881ca448a093fecd08ead4847458df1054bfef5b8b1c28500a2e176ba71dc
nodeB.tip_hash=b2e12e4f6d419ceffcddf9b4266912cb7748ac6d7d07a7b9f44709524947019d
wave-a failed: wave-a hash divergence: tip_hash_equal=false, last_epoch_hash_equal=false
```

## Verdict

**PARTIAL**

- **PASS** for wire/u128 stabilization goals (no observed `u128` decode protocol errors in Wave A logs; `wire_decode` tests green).
- **PASS** for requested `peer_session` and `tip_divergence` test filters.
- **FAIL** Wave A end-to-end acceptance gate (tip / last-epoch hash mismatch between nodes) — **out of scope for this note’s primary goal** but blocks calling the full scenario “green”.

## Participation / token estimate

- `agent`: pwm-testing
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 18000, "confidence": "low" }`
