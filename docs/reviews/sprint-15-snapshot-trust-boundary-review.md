# Sprint 15 — snapshot trust boundary RFC

## Scope

This note defines the operational trust boundary after the JsonFile loader moved to trust-default startup. It is not a new consensus rule: it describes what a node assumes about its own persisted disk state when restarting.

## Trust boundary

In normal JsonFile mode, `pwmd` trusts the local snapshot summary and epoch manifest as operator-controlled state. Startup validates that:

- `pwm-data.json` matches the active genesis account set and snapshot version;
- `checkpoint_height` equals manifest `canonical_h`;
- manifest `tip_hash` equals the hash of the tail tip block;
- the loaded tail length matches `TAIL_BLOCK_CAP` semantics;
- tail blocks link to each other and, for a non-genesis tail, to the parent block stored in `epochs/`;
- tail PoA headers, `tx_root`, producer index, signatures, and final state root are internally consistent.

The normal path does not replay every historical transaction from genesis. This is intentional: the local disk checkpoint is treated as a trusted state anchor, similar to how mature nodes avoid reprocessing the entire historical store on every restart.

## Audit and recovery mode

Use `--snapshot-verify-chain` or truthy `PWM_SNAPSHOT_VERIFY_CHAIN` when the operator wants a full genesis-to-tip replay. The loader also forces full verification automatically when the summary checkpoint lags the manifest tip. That fallback handles interrupted/partial persistence where the epoch files advanced but the summary did not.

## Residual risks

- A local attacker or broken disk that mutates both summary and epochs consistently may evade the normal trust-default checks.
- The normal path validates the recent tail and persisted state root, not the semantic history of old transactions.
- Full replay remains the stronger local audit tool, but it costs time and memory proportional to chain history.

## JsonFile vs ClickHouse

The trust-default model applies to JsonFile epoch storage. ClickHouse snapshot load currently reconstructs from stored blocks and performs full replay validation; JsonFile `SnapshotLoadOpts` do not weaken ClickHouse validation. Operators should treat CH as the audit-heavy backend until a separate CH checkpoint trust model is specified.

## Recommendation

Use normal mode for routine restarts, CI smoke nodes, and trusted operator disks. Use audit mode after manual file surgery, unexplained state-root errors, suspected corruption, or before promoting long-lived data into a shared environment.

---

## Participation / token estimate

- `agent`: `pwm-coding`
- `result`: `PASS`
- `token_usage`: estimate, total ~1200, confidence low
