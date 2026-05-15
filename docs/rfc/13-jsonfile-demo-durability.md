# RFC 13 — JsonFile demo durability (batch seal persist)

**Status:** accepted (demo strategy B)  
**Scope:** `pwmd` JsonFile autosnapshot path only; ClickHouse semantics stay unchanged.

## 1. Intent

JsonFile snapshot mode is a **demo/runtime convenience path**, not a mainnet-scale durability backend.
The goal of this slice is to reduce steady-state disk pressure by avoiding heavy persist on every seal.

## 2. Runtime policy

- Seal-time autosnapshot persist runs only on `SNAP_CHK_BLK_IV` boundary (current value: `100`).
- When persist is called, JsonFile performs full convergence:
  - `sync_epoch_to_tip(...)`
  - `save_checkpoint_summary(...)`
- `POST /v1/shutdown` performs the same full flush.
- ClickHouse Json fallback on seal failure also uses full Json flush (safe-first fallback).

## 3. Durability caveat (demo)

Between periodic checkpoints, abrupt process termination (for example `kill -9`) can lose the latest in-memory tail that was not flushed yet.
This is an explicit trade-off for lower I/O in demo mode and must not be treated as production durability behavior.

## 4. Operational note

Operators can still force disk convergence via API save/shutdown paths; these paths align manifest/summary with current tip.
Periodic cadence is anchored to `SNAP_CHK_BLK_IV` and should be documented whenever autosnapshot defaults are changed.
