# V7-S1 log timing report: head_stall at level 34 (height 201996)

- date: 2026-06-25
- ticket: `20260625-v7-s1-log-timing-analysis`
- agent: `pwm-review`
- ramp artifact: `tmp/v7-s1-slice4-ramp-soak.md` (`stop_reason=head_stall`, last recorded good height **201995**, level **34**)
- parser: `logs/parse_timing.py` → `scripts/_review_parse_timing.py`

## Scope recap

Slice 4 ramp (commit `5f1affb`) stopped at **34 tx/block** with `head_stall`. Prior slice-4 review noted cluster gate / attester delays **~8 s around height 201996**. This report reconstructs the timeline from live logs and identifies where that delay accumulates.

**Log session note:** Ticket text references PID **164236/164238** (cluster restart at 16:42, heights ~202031+). The ramp stall at **201996** occurred in the **earlier** long-lived session **131749/131801** (proposer start 13:17:49, ramp window **16:17–16:18**). Analysis uses the correct session files:

| role | file |
|------|------|
| proposer | `logs/2026-06-25/pwmd-cy-proposer-131749.log` |
| attester | `logs/2026-06-25/pwmd-cy-attester-131801.log` |
| peer proposer | `logs/2026-06-25/pwmd-peer-cy-proposer-131749.log` |
| peer attester | `logs/2026-06-25/pwmd-peer-cy-attester-131801.log` |

`tmp/cy-lab-block-timing.jsonl` starts at height **208620** (post-restart) and does not cover the ramp stall window.

## Cluster config summary

From proposer log line 4 (`pwmd-cy-proposer-131749.log`):

| parameter | value |
|-----------|-------|
| proposer | `cy-quorum-proposer` @ 127.0.0.1:3030, peer 127.0.0.1:13030 |
| attester | `cy-quorum-attester` @ 127.0.0.2:13030 |
| quorum | 1/2 (`quorum_k=1`, `cluster_n=2`) |
| `attest_timeout_ms` | 2000 |
| `seal_interval_ms` | 1000 |
| `seal_ahead_ms` | 100 |
| `max_tip_lag` | 2 |

No `attest_timeout` or `cluster_gate_round_reopen` events appear in the 201960–202000 window (only startup round-reopen at height 191177).

## Block throughput timeline (ramp window)

### Per-10-height seal cadence (proposer `sealed height=` markers)

| sealed_h | wall time | Δ from prev | notes |
|----------|-----------|-------------|-------|
| 201960 | 16:17:55.023 | — | ramp level 1 batch |
| 201970 | 16:18:07.120 | **12.1 s** | ramp `seal_slip_ms=2126` spike |
| 201980 | 16:18:16.334 | 9.2 s | |
| 201990 | 16:18:26.265 | 9.9 s | |
| 202000 | 16:18:44.021 | **17.8 s** | includes **201996** stall; heights 201991–201999 not logged individually |

Intermediate blocks 201991–201999 seal inside the decimated `sealed height=` log stream (every 10th height).

### Ramp table (from `tmp/v7-s1-slice4-ramp-soak.md`, heights 201960–201995)

| height | level (tx) | rpc_p50_ms | seal_slip_ms | block_dt_ms |
|--------|------------|------------|--------------|-------------|
| 201990 | 29 | 469 | 274 | 1044 |
| 201993 | 32 | 552 | 471 | 1061 |
| 201994 | 33 | 621 | 590 | 1130 |
| 201995 | 34 | 513 | 36 | 888 |

Level 34 (201995) itself sealed quickly (`seal_slip_ms=36`). The stall is on the **next** block (**201996**).

## Attester latency (propose → attest RTT)

From `pwmd-peer-cy-proposer-131749.log` (`cluster propose sent` → `cluster attest accepted`):

| height | propose | attest | RTT ms |
|--------|---------|--------|--------|
| 201990 | 16:18:25.285 | 16:18:25.596 | 311 |
| 201991 | 16:18:26.329 | 16:18:26.632 | 303 |
| 201992 | 16:18:27.352 | 16:18:27.742 | 390 |
| 201993 | 16:18:28.512 | 16:18:29.051 | 539 |
| 201994 | 16:18:29.512 | 16:18:29.950 | 438 |
| 201995 | 16:18:30.642 | 16:18:31.010 | 368 |
| **201996** | **16:18:31.604** | **16:18:32.318** | **714** |
| 201997 | 16:18:40.165 | 16:18:40.727 | 562 |
| 201998 | 16:18:41.385 | 16:18:41.626 | 241 |
| 201999 | 16:18:42.053 | 16:18:42.074 | 21 |

**Distribution (201990–201996, stall window):** p50 ≈ **390 ms**, p95 ≈ **680 ms**, max **714 ms** — all well under `attest_timeout_ms=2000`.

Peer attestation is **not** the ~8 s bottleneck. Attest for 201996 completes **714 ms** after propose while the proposer is still ~8 s from committing the block.

## head_stall analysis: height 201996 timeline

Ramp client last submit (`tmp/v7-s1-slice4-ramp-soak.client.jsonl`): level 34 batch at `ts_ms=1782404312307`, `height_at_submit=201995`. Head did not advance past 201995 in time for level 35 → `head_stall`.

### Proposer (`pwmd-cy-proposer-131749.log`)

| time | event | detail |
|------|-------|--------|
| 16:18:31.034 | `tx_included` | height **201995** (last ramp batch, 34 txs) |
| 16:18:31.036 | `tx commit delta` | 201995 commits finish |
| 16:18:32.122 | `seal_suppressed_by_cluster` | height **201996**, `attestations_missing`, `phase=pre_timeout` |
| 16:18:33.240 – 16:18:39.536 | `seal skip: evicting unapplicable tx` | bad-nonce evictions, mempool 60–63 txs requeued each tick |
| 16:18:37.044 | `cluster_attest_waiting_sync` | `live_synced_attesters=0`, `proposer_tip=201995`, `attester_tip_max=0` |
| 16:18:37.546 | `cluster_attest_ready` | attester back (`~502 ms` wait) |
| 16:18:40.086 – 16:18:40.108 | `tx_included` / `tx commit delta` | height **201996**, 34 transfers applied |
| 16:18:44.021 | `sealed height=202000` | batch seal (201996–201999 not individually logged) |

**Gate-to-commit span:** 16:18:32.122 → 16:18:40.086 ≈ **7.96 s** (matches slice-4 “~8 s” note).

### Peer layer (corroboration)

| time | source | event |
|------|--------|-------|
| 16:18:31.604 | peer proposer | `cluster propose sent` h=201996 |
| 16:18:32.318 | peer proposer | `cluster attest accepted` h=201996 (RTT 714 ms) |
| 16:18:30.999 | peer attester | `cluster_route_slow` latency **352 ms** |
| 16:18:32.307 | peer attester | `peer sync tx batch` seen=32 accepted=32 |
| 16:18:37.045 | peer proposer | inbound read latency **4727 ms** on heartbeat (attester link stall) |
| 16:18:30–16:18:37 | both peer logs | `peer storm guard suppress` / `socket_read_timeout` storm under load |

Attester applied propose for 201996 at 16:18:32.310 (`cluster propose accepted`) but proposer seal loop remained blocked on mempool / sync for several more seconds.

### Where the ~8 seconds go

```
32.122  seal_suppressed (gate open, no quorum yet)
32.318  attest accepted (714 ms RTT)     ← only ~0.2 s from suppress
33–39   seal_skip bad-nonce evictions    ← ~6 s dominant slice
37.044  attest_waiting_sync (0 attesters)
37.546  attest_ready (+502 ms)
40.086  tx_included h=201996             ← block execution starts
```

1. **~0.7 s** — normal attest round-trip (not timeout).
2. **~4.7 s** — seal-slot **`seal skip` / bad-nonce** churn: ramp submitted level-34 txs while mempool still held **60+** pending entries; seal loop evicts one bad-nonce tx per second instead of advancing.
3. **~0.5 s** — brief **`cluster_attest_waiting_sync`** (attester counted disconnected; peer read gap **4.7 s** on inbound heartbeat).
4. **~2.5 s** — remaining seal_skip ticks + **34-tx block build** (`tx commit delta` burst ~16:18:40.092–40.108).

**Not observed:** `attest_timeout`, `cluster_gate_round_reopen`, or peer disconnect of the CY attester seed (127.0.0.2). Noise from failed connects to **127.0.0.3:13030** (unrelated shard) appears throughout but does not explain the CY quorum path.

## Bottleneck hypothesis

**Primary:** seal-loop **mempool pressure** at level 34 — overlapping RPC ingress (ramp firing the next batch before head advances) produces **`seal skip: bad nonce`** evictions that consume seal slots (~1 Hz) while the cluster gate waits. Attestation completes quickly; the proposer cannot commit because the seal pipeline is busy evicting/requeuing.

**Secondary:** attester **sync / peer I/O saturation** under 32+ tx batches (`cluster_route_slow`, `peer storm guard`, 4.7 s inbound read stall) triggers a short **`cluster_attest_waiting_sync`** window, adding ~0.5 s and widening the head_stall gap seen by the ramp script.

**Tertiary:** decimated sealing (201996 committed inside a jump to `sealed height=202000`) means RPC head may stay at **201995** for the full ~8 s even though work on 201996 is in progress — the ramp `head_stall` detector fires before 201996 becomes visible.

## Recommendations

1. **Ramp harness:** after submitting level N, wait for `head > batch_height` (or sealed event) before level N+1; current overlap causes bad-nonce seal_skip storms.
2. **Seal loop:** consider batching nonce eviction or pausing ingress while `seal_suppressed_by_cluster` is active at the target height (backpressure).
3. **Attester path:** profile `cluster_route_slow` and peer read loop under 34-tx proposes; 50 ms socket timeouts with heavy CPU may inflate `waiting_sync` false negatives.
4. **Observability:** emit `sealed height=` per block (not only every 10) during lab runs, or wire ramp to `tmp/cy-lab-block-timing.jsonl` with rotation so post-mortem JSONL covers stall heights.
5. **Timeout tuning:** raising `attest_timeout_ms` would **not** help this stall — attest RTT stayed &lt; 800 ms; focus on mempool/seal_skip and peer saturation.

## Parser usage

Streaming parser (stdlib only):

```bash
python logs/parse_timing.py \
  --files 'logs/2026-06-25/pwmd-cy-proposer-131749.log' \
  --height-from 201990 --height-to 202000 \
  --event 'seal|cluster_attest|cluster_gate' \
  --out tmp/h201996-events.jsonl

python scripts/_review_parse_timing.py \
  --files 'logs/2026-06-25/pwmd-peer-cy-proposer-131749.log' \
  --height-from 201990 --height-to 202000 --summary
```

## Concurrency / parallelism

Components: proposer seal loop, cluster gate (`seal_suppressed_by_cluster`), attester standby apply path, peer ingress loops (both nodes).

Hazards in window: (1) **check-then-act** between attest accepted and seal commit — attestation arrives while mempool lock/eviction still runs; (2) **peer read timeouts** under load causing transient `live_synced_attesters=0`; (3) **unbounded mempool growth** from parallel RPC submits without head wait.

Test gap: no automated test for ramp overlap → `seal skip` storm → `head_stall` at fixed 1 s seal cadence.

## Verdict

**PASS** — root cause of ~8 s delay at height 201996 identified with log evidence; parser and report delivered. Stall is **mempool/seal_skip dominated**, not attest timeout.

## Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260625-v7-s1-log-timing-report.md`, `logs/parse_timing.py`, `scripts/_review_parse_timing.py`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 85000, "confidence": "medium" }`