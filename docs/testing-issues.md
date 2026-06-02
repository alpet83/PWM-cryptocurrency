# Testing issues log

Журнал **очевидных опечаток и harness-блокеров**, исправленных **pwm-testing** на месте (см. **`docs/AGENT_PROMPT_testing.md`** §Obvious typo and harness fixes).

**Правило:** каждая такая правка — **новая строка** в таблице ниже **в той же сессии**, что и fix (до handoff с `PASS`).

| Date | Ticket | File | Symptom | Fix | Fixed by | Retest |
|------|--------|------|---------|-----|----------|--------|
| 2026-06-02 | `20260602-v5-tui-test-support-mk-acct-row-coding` | `crates/pwm-tui/tests/wallet_roaming.rs` | `footer_rpc_online_one` asserted `spans.len()==1` but `status_footer_line` emits multi-span footer with styled F-keys | Drop stale single-span assert; check no red health spans + flattened prefix | pwm-testing | PASS (wallet_roaming rerun) |
| 2026-05-24 | `20260524-v5-s8-slice1-op-smoke-marks-testing-rerun` | `scripts/devnet_v5_operator_smoke.ps1` | Redundant tx-init on genesis pre-initialized account (AlreadyInit); invalid `--fee` on tx-stake | Skip tx-init when initialized; drop stake `--fee` | orchestrator | PASS (rerun2) |
| 2026-05-24 | `20260524-v5-s8-slice1-op-smoke-marks-debug` | `scripts/devnet_v5_operator_smoke.ps1` | PARTIAL when genesis marks already at u32::MAX; harness required marks growth | PASS when saturated baseline + marks_last_block advances | orchestrator | PASS (rerun2) |
| 2026-05-27 | `20260524-v5-s8-slice3-op-smoke-ipv4-claim-testing-rerun2` | `scripts/devnet_v5_operator_smoke.ps1` | PowerShell parser error in `Submit-ClaimIPv4Batch` from smart quote mojibake plus line-broken `-and` expression | Use ASCII text and wrap `ipv4_claimed_phase` predicate in parentheses | pwm-testing | validated; final smoke FAIL `E_POLICY_SCHEMA_INVALID` |
| 2026-05-27 | `20260524-v5-s8-slice3-op-smoke-ipv4-claim-testing-rerun2` | `scripts/devnet_v5_operator_smoke.ps1` | Windows PowerShell 5.1 rejects `ConvertFrom-Json -Depth` during genesis phase injection | Drop unsupported read-side `-Depth`; keep write-side `ConvertTo-Json -Depth` | pwm-testing | validated; final smoke FAIL `E_POLICY_SCHEMA_INVALID` |
| 2026-05-27 | `20260524-v5-s8-slice3-op-smoke-ipv4-claim-testing-rerun2` | `scripts/devnet_v5_operator_smoke.ps1` | IPv4 phase injection searched obsolete `$genesis.accounts`/`id` shape and hard-coded wallet index `1` | Read funded account from `gen_cfg.funding.accounts` and use its `der_idx` for wallet claimant | pwm-testing | validated; final smoke FAIL `E_POLICY_SCHEMA_INVALID` |
| 2026-05-27 | `20260524-v5-s8-slice3-op-smoke-ipv4-claim-testing-rerun2` | `scripts/devnet_v5_operator_smoke.ps1` | Phase injection failed to add missing `ipv4_claim_phases` because `-not ... -contains` precedence was wrong | Parenthesize property-existence check | pwm-testing | validated; final smoke FAIL `E_POLICY_SCHEMA_INVALID` |
| 2026-05-27 | `20260524-v5-s8-slice3-op-smoke-ipv4-claim-testing-rerun2` | `scripts/devnet_v5_operator_smoke.ps1` | Rewritten genesis JSON had UTF-8 BOM under Windows PowerShell 5.1 and `pwmd` rejected it at byte 1 | Write post-processed genesis with .NET UTF8Encoding without BOM | pwm-testing | validated; final smoke FAIL `E_POLICY_SCHEMA_INVALID` |
| 2026-05-27 | `20260524-v5-s8-slice3-op-smoke-ipv4-claim-testing-rerun2` | `scripts/devnet_v5_operator_smoke.ps1` | `-Ipv4ClaimOnly` still ran marks/deferred loops because slice booleans ignored the IPv4-only switch | Gate marks/deferred off when `Ipv4ClaimOnly` is set | pwm-testing | validated; final smoke FAIL `E_POLICY_SCHEMA_INVALID` |
| 2026-05-27 | `20260524-v5-s8-slice3-op-smoke-ipv4-claim-testing-rerun2` | `scripts/devnet_v5_operator_smoke.ps1` | `claim-ipv4-batch` JSON parsing was contaminated by `cargo run` status lines captured via `2>&1` | Run helper through `cargo run --quiet` | pwm-testing | validated; final smoke FAIL `E_POLICY_SCHEMA_INVALID` |
| 2026-06-01 | `20260612-v5-snapshot-genesis-anchor-light-coding` | `crates/pwmd/src/snapshot/types.rs` + `crates/pwmd/src/tests/snapshot_roaming.rs` | `snap_or_mk_quota` + `snap_reject_quota_mismatch` FAIL: v3 snapshots have no `marks_quota`; injected rows were ignored. | **Cancelled:** tests removed in `20260602-v5-pwmd-remove-legacy-marks-quota-coding`; legacy `marks_quota` mirror stripped from pwmd snapshot wire (v3 uses `stored_marks` only). | pwm-coding | N/A — tests deleted |

## Entry template (copy for new rows)

```markdown
| YYYY-MM-DD | `<ticket-id>` | `path/to/file` | one-line symptom | one-line fix | pwm-testing | PASS / FAIL / pending |
```

**Notes column (optional):** add as trailing text in Symptom or a footnote row if the fix spans multiple files.

---

_Last updated: 2026-06-02_
