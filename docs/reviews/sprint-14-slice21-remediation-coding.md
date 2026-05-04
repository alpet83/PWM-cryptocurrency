# Sprint 14 Slice21 remediation coding

Date: 2026-04-29

## Changes

- Fixed the Rust parse/fmt blocker in `crates/pwmd/src/snapshot.rs`.
- Replaced snapshot version match guards from `SNAPSHOT_VERSION as u64` / `SNAPSHOT_V1 as u64` to `u64::from(SNAPSHOT_VERSION)` / `u64::from(SNAPSHOT_V1)`.
- Kept the intended v2 behavior unchanged: v2 snapshots still decode through `SnapshotDataV2`, v1 snapshots still load through the legacy canonical path and are upgraded to `SNAPSHOT_VERSION`.
- No additional v2 hex/decimal migration fixes were needed because the focused snapshot tests passed.

## Command Results

```text
$ cargo fmt
exit 0
```

```text
$ cargo check -p pwmd
exit 0
Checking pwmd v0.1.12
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.94s
```

```text
$ cargo test -p pwmd snapshot_
exit 0
running 25 tests
25 passed; 0 failed; 0 ignored; 114 filtered out

running 0 tests
0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
