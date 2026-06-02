# Review: V5 Pre-publish Naming / Style Sweep

Ticket: 20260602-v5-prepublish-naming-style-sweep-review  
Date: 2026-06-02  
Reviewer: pwm-coding  
Fix ticket: 20260602-v5-prepublish-naming-violations-fix-coding

## Scope

Mandatory full scan was executed with:

```bash
python scripts/check_entity_name_segments.py \
  crates/pwm-core/src crates/pwm-cli/src crates/pwmd/src crates/pwm-tui/src \
  crates/pwm-tui/tests crates/pwm-core/tests crates/pwm-cli/tests crates/pwmd/src/tests
```

No product Rust code was edited in this ticket. Output was captured in:

- `tmp/20260602-naming-style-sweep.json`

## Acceptance Coverage

All requested items are present:

1. Report artifact: this file.
2. JSON-based summary: totals, top files, entity-kind split.
3. Explicit verdict.
4. Explicit handoff list for fix-coding ticket.
5. Wire JSON / u128 subsection.
6. No product Rust edits.

## Scanner Summary (JSON-derived)

```json
{
  "policy": {
    "prod_max": 4,
    "test_max": 5
  },
  "totals": {
    "files": 176,
    "violations": 6
  },
  "top_files": [
    {
      "path": "crates/pwmd/src/block_timing.rs",
      "count": 5,
      "items": [
        {
          "line": 1059,
          "name": "profile_time_json_stats_merges_schema",
          "entity": "fn",
          "segments": 6,
          "limit": 5,
          "kind": "test"
        },
        {
          "line": 1081,
          "name": "trim_jsonl_tail_keeps_latest_rows",
          "entity": "fn",
          "segments": 6,
          "limit": 5,
          "kind": "test"
        },
        {
          "line": 1101,
          "name": "pendrec_ms_fields_serialize_with_two_decimals",
          "entity": "fn",
          "segments": 7,
          "limit": 5,
          "kind": "test"
        },
        {
          "line": 1122,
          "name": "pendrec_ms_fields_parse_string_float_and_int",
          "entity": "fn",
          "segments": 8,
          "limit": 5,
          "kind": "test"
        },
        {
          "line": 1146,
          "name": "trim_pending_map_tail_keeps_latest_by_height",
          "entity": "fn",
          "segments": 8,
          "limit": 5,
          "kind": "test"
        }
      ]
    },
    {
      "path": "crates/pwmd/src/transport/peer_session/inbound.rs",
      "count": 1,
      "items": [
        {
          "line": 111,
          "name": "INBOUND_SOCKET_READ_LOG_SLOW_MS",
          "entity": "const_or_static",
          "segments": 6,
          "limit": 4,
          "kind": "prod"
        }
      ]
    }
  ],
  "by_kind": {
    "test": 5,
    "prod": 1
  }
}
```

## Module Banner Snapshot

Source-tree banner check was run for:

- `crates/pwm-core/src`
- `crates/pwm-cli/src`
- `crates/pwmd/src`
- `crates/pwm-tui/src`

Result:

- `total`: 163
- `banner_ok`: 160
- `banner_missing`: 3

Sample missing banners:

- `crates/pwm-core/src/marks.rs`
- `crates/pwmd/src/tests/snapshot_backend_replay.rs`
- `crates/pwm-tui/src/test_support.rs`

Note: `crates/pwmd/src/transport/peer_session/inbound.rs` and `crates/pwmd/src/block_timing.rs` both have valid `//!` banners.

## Findings (ordered by severity)

1. High: production naming violation blocks style gate.
   - File: `crates/pwmd/src/transport/peer_session/inbound.rs:111`
   - Symbol: `INBOUND_SOCKET_READ_LOG_SLOW_MS` (6 segments, limit 4)
   - Impact: fails production naming policy, should be fixed before pre-publish gate is considered clean.

2. Medium: test naming debt in one file.
   - File: `crates/pwmd/src/block_timing.rs`
   - 5 test function names exceed 5-segment test cap.
   - Impact: style debt in tests; not production behavior risk.

## Wire JSON / u128

No wire JSON or peer protocol symbols were changed in this ticket. The report is read-only and does not alter serialization, `u128` field handling, or wire compatibility.

## Verdict

FAIL

Reason: there is still 1 production naming violation (`prod` kind). `PASS_WITH_NITS` is not applicable because that status is valid only for test-only backlog.

## Handoff to 20260602-v5-prepublish-naming-violations-fix-coding

1. Rename production constant in `crates/pwmd/src/transport/peer_session/inbound.rs:111` to a <=4 segment name.
2. Rename 5 overlong test functions in `crates/pwmd/src/block_timing.rs` to <=5 segment names.
3. Rerun:
   - `python scripts/check_entity_name_segments.py crates/pwm-core/src crates/pwm-cli/src crates/pwmd/src crates/pwm-tui/src crates/pwm-tui/tests crates/pwm-core/tests crates/pwm-cli/tests crates/pwmd/src/tests`
4. Confirm `violations=0` before promoting pre-publish style gate to PASS.

## Participation / Token Estimate

```yaml
agent: pwm-coding
result: FAIL
artifacts:
  - docs/reviews/20260602-v5-prepublish-naming-style-sweep-review.md
  - tmp/20260602-naming-style-sweep.json
commands:
  - python scripts/check_entity_name_segments.py crates/pwm-core/src crates/pwm-cli/src crates/pwmd/src crates/pwm-tui/src crates/pwm-tui/tests crates/pwm-core/tests crates/pwm-cli/tests crates/pwmd/src/tests > tmp/20260602-naming-style-sweep.json
  - node -e "...json summary from tmp/20260602-naming-style-sweep.json..."
  - python - <<'PY' ...module banner scan... PY
token_usage:
  source: estimate
  input: 7000
  output: 1400
  total: 8400
  confidence: medium
```