# V7-S3 Worker Scale Results

Date: 2026-06-27

## Configuration

| item | value |
|------|-------|
| build | debug, pwmd 0.1.71 |
| host logical CPUs | 16 |
| worker split | 1 affinity + 7 general |
| cluster | isolated proposer + attester, copied lab snapshots |
| ramp | 4 tx step, target 58 tx/block |

The isolated cluster used 127.0.0.4/5; the active lab cluster and its state
directories were not modified.

## Comparison

| metric | V7-S2 baseline | V7-S3 |
|--------|----------------|-------|
| sustained tx/block | 52 | 58 |
| change | - | +11.5% |
| final-level reject rate | 0% | 0% |
| stop reason | block_dt_overrun | max_tx_level |
| worker queue wait p50 | not available | 1 ms |
| worker queue depth max | not available | 35 |

The ramp reached level 58 with 58 accepted transactions and no rejection.
Level 56 had one transient rejection (1.8%); the next level completed cleanly.
The complete generated report is tmp/v7-s3-worker-scale.md.

## Selected Levels

| level | accepted | rejected | rpc p50 ms | seal slip ms |
|-------|----------|----------|------------|--------------|
| 44 | 44 | 0 | 1135 | 511 |
| 48 | 48 | 0 | 848 | 357 |
| 52 | 52 | 0 | 953 | 439 |
| 56 | 55 | 1 | 1023 | 468 |
| 58 | 58 | 0 | 1176 | 676 |

## Conclusion

The sprint criterion is met: debug sustained level increased above 52 tx/block.
Worker queue wait is no longer the dominant latency source. The remaining RPC
latency is dominated by client process/signing overhead and seal/apply work, so
further scaling should target the transaction submission harness and state
precheck/apply hot path rather than adding more worker threads.
