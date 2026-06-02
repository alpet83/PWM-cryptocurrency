# Timestamp correlation

## Symptom B: 08:51 startup timeout
### orig_B_proposer (`logs/2026-05-31/pwmd-cy-proposer-082709.log`)
- line 7 `[08:27:10.573] #INFO: snapshot loading started: P:\opt\docker\PWM-cryptocurrency\tmp\state-cy-proposer\pwm-data.json`
- line 12 `[08:27:10.575] #INFO: pwmd listening on http://127.0.0.1:3030 peer=127.0.0.1:13030 shard=CY state_ns=domain-hi-0x2c identity=(testnet-qa,0x2C,test-cluster-CY,cy-proposer) mode=shard_enforced(explicit-domain-config)`
- line 13 `[08:27:18.596] #INFO: snapshot startup load ok | path=P:\opt\docker\PWM-cryptocurrency\tmp\state-cy-proposer\pwm-data.json mode=epochs tip_h=65300 canonical_h=65300 total_ms=8022 summary_read_ms=1 epochs_ms=44 validate_ms=7976 into_runtime_ms=0 absorb_tail_ms=0 ch_http_ms=0 ch_parse_ms=0 ch_branch=`
- line 16 `[08:27:18.596] #INFO: pwmd startup phase: ready (snapshot loaded)`
- line 4 `[08:27:10.561] #INFO: cluster_attest enabled=true role=Proposer members=cy-quorum-proposer,cy-quorum-attester quorum=1/2 blocks_per_hour=3600 seal_interval_ms=1000 attest_timeout_ms=2000 heartbeat_interval_ms=1000 seal_ahead_ms=100 note=s2_lease_orthogonal_genesis_timing`
- line 49 timeout height=65301 elapsed_ms=6907 limit_ms=2000 at 08:51:16.461
### orig_B_attester (`logs/2026-05-31/pwmd-cy-attester-085107.log`)
- line 7 `[08:51:08.130] #INFO: snapshot loading started: P:\opt\docker\PWM-cryptocurrency\tmp\state-cy-attester\pwm-data.json`
- line 12 `[08:51:08.132] #INFO: pwmd listening on http://127.0.0.2:3030 peer=127.0.0.2:13030 shard=CY state_ns=domain-hi-0x2c identity=(testnet-qa,0x2C,test-cluster-CY,cy-attester) mode=shard_enforced(explicit-domain-config)`
- line 13 `[08:51:16.303] #INFO: snapshot startup load ok | path=P:\opt\docker\PWM-cryptocurrency\tmp\state-cy-attester\pwm-data.json mode=epochs tip_h=65300 canonical_h=65300 total_ms=8173 summary_read_ms=0 epochs_ms=44 validate_ms=8127 into_runtime_ms=0 absorb_tail_ms=0 ch_http_ms=0 ch_parse_ms=0 ch_branch=`
- line 16 `[08:51:16.304] #INFO: pwmd startup phase: ready (snapshot loaded)`
- line 4 `[08:51:08.119] #INFO: cluster_attest enabled=true role=Attester members=cy-quorum-proposer,cy-quorum-attester quorum=1/2 blocks_per_hour=3600 seal_interval_ms=1000 attest_timeout_ms=2000 heartbeat_interval_ms=1000 seal_ahead_ms=100 note=s2_lease_orthogonal_genesis_timing`

## Symptom A/C: 10:16 and repro lag/pending
### orig_A_proposer (`logs/2026-05-31/pwmd-cy-proposer-101620.log`)
- heartbeat=[('Proposer', 1000)]
- waiting_sync_count=139 lag_counts={'2': 138, '65300': 1}
- timeout_count=2 pending_ge_100=14 pending_max=336
### orig_A_attester (`logs/2026-05-31/pwmd-cy-attester-101623.log`)
- heartbeat=[('Attester', 1000)]
- waiting_sync_count=0 lag_counts={}
- timeout_count=0 pending_ge_100=0 pending_max=None
- sync_stall line 17 `[10:17:04.976] #INFO: sync_catchup_stall node_id=cy-proposer rem=1 local_h=65323 head_h=65324 cup_active=false cup_try=0`
- sync_stall line 220 `[10:25:03.892] #INFO: sync_catchup_stall node_id=cy-proposer rem=1 local_h=65516 head_h=65517 cup_active=false cup_try=0`
### repro_proposer (`tasks\20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence\repro-proposer-stdout.log`)
- heartbeat=[('Proposer', 1000)]
- waiting_sync_count=40 lag_counts={'2': 39, '65500': 1}
- timeout_count=5 pending_ge_100=10 pending_max=380
### repro_attester (`tasks\20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence\repro-attester-stdout.log`)
- heartbeat=[('Attester', 1000)]
- waiting_sync_count=0 lag_counts={}
- timeout_count=0 pending_ge_100=0 pending_max=None
- sync_stall line 12 `[10:30:11.453] #INFO: sync_catchup_stall node_id=cy-proposer rem=1 local_h=65565 head_h=65566 cup_active=false cup_try=0`
- sync_stall line 14 `[10:32:31.838] #INFO: sync_catchup_stall node_id=cy-proposer rem=1 local_h=65630 head_h=65631 cup_active=false cup_try=0`
