# RCA: attester Standby prints occasional `Sync progress 99%`

Дата: 2026-05-13  
Тикет: `tasks/20260519-attester-sync-progress-99-debug.json`  
Агент: `pwm-debug`  
Фокус: `transport:peers` + same-shard sync / `Sync progress`  
Результат: `PASS` для корневой причины по предоставленным логам; `repro.deterministic=no` из-за нерегулярности операторского симптома.

## Краткий вывод

Свежая пара логов по LastWriteTime:

- `logs/2026-05-13/pwmd-cy-attester-155753.log` (`LastWriteTime=2026-05-13 19:08:21`, `Length=39795`)
- `logs/2026-05-13/pwmd-peer-cy-attester-155753.log` (`LastWriteTime=2026-05-13 19:08:41`, `Length=415365`)

Корневая причина наблюдаемого `#INFO: Sync progress 99%` у Standby attester: это операторский прогресс same-shard live-following, а не "залипший" epoch catch-up. В текущей реализации Standby подавляет `Sync progress` только для healthy live short-tail, когда `rem <= 1`; редкие моменты, где локальная память отстаёт на 2-3 блока от растущего proposer tip (`rem=2/3`), не попадают под suppress и печатаются как `99%`. Peer-лог в те же окна показывает `node_id=cy-proposer`, `lag=1/2`, `cup_on=false`, `live_hdr`, затем `apply ok`; `catchup`, `cup_started`, `nack` и wire decode errors в свежей паре отсутствуют.

Связь с новым `--peers-list` есть, но не как причина `Sync progress 99%`: общий `tmp/cy-lab-peers.yaml` содержит `cy-follower` (`127.0.0.3:13030`), который в этом прогоне недоступен, поэтому peer-лог зашумлён reconnect timeout-ами. Эти timeout-ы идут по seed `127.0.0.3:13030`, тогда как sync/progress события идут по `node_id=cy-proposer`.

## Evidence

### Attester role and visible symptom

```text
logs/2026-05-13/pwmd-cy-attester-155753.log:3
[15:57:53.902] #INFO: deployment_profile=single_sealer seal_role=Standby validator_identity_hash=abad69e0015b728c7ff3ad18275777e533bf762b350abbadc03a6cc9a4c4cc7e node_instance_id=cy-quorum-attester lease_ttl_ms=10000 takeover_timeout_ms=8000 takeover_max_tip_lag=1 lease_backend=ProcessLocal lease_path=-
logs/2026-05-13/pwmd-cy-attester-155753.log:11
[15:58:01.582] #INFO: snapshot startup load ok | path=P:\opt\docker\PWM-cryptocurrency\tmp\state-cy-attester\pwm-data.json mode=epochs tip_h=19100 canonical_h=19100 total_ms=7644 summary_read_ms=0 epochs_ms=50 validate_ms=7592 into_runtime_ms=0 absorb_tail_ms=0 ch_http_ms=0 ch_parse_ms=0 ch_branch=
logs/2026-05-13/pwmd-cy-attester-155753.log:15
[15:58:42.710] #INFO: Sync progress 99% rem=2 goal=19119 mem=19117/19119 disk=19100/19119
logs/2026-05-13/pwmd-cy-attester-155753.log:21
[16:00:58.038] #INFO: Sync progress 99% rem=3 goal=19186 mem=19183/19186 disk=19100/19186
```

Хвост подтверждает ту же форму позднее:

```text
logs/2026-05-13/pwmd-cy-attester-155753.log:327
[16:09:30.479] #INFO: Sync progress 99% rem=2 goal=19432 mem=19430/19432 disk=19400/19432
```

### Same-shard sync mode at the same moments

```text
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:235
[15:58:42.710] #INFO: peer sync on_tip lag node_id=cy-proposer local_h=19117 head_h=19119 lag=2 persisted_h=19100 live_stall=0 cup_on=false can_cup=true
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:236
[15:58:42.710] #INFO: peer sync on_tip live_hdr node_id=cy-proposer next_from=19118 req_lim=128 lag=2
```

Поздний хвост совпадает с последней `99%` строкой и показывает применение двух блоков:

```text
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:3918
[16:09:30.479] #INFO: peer sync on_tip lag node_id=cy-proposer local_h=19430 head_h=19432 lag=2 persisted_h=19400 live_stall=0 cup_on=false can_cup=true
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:3919
[16:09:30.479] #INFO: peer sync on_tip live_hdr node_id=cy-proposer next_from=19431 req_lim=128 lag=2
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:3922
[16:09:30.536] #INFO: peer sync apply ok node_id=cy-proposer blocks=2
```

Агрегаты по свежей паре логов:

```text
9   Sync progress
3   standby sync checkpoint
457 peer sync on_tip lag
457 peer sync on_tip live_hdr
314 peer sync apply ok
0   peer sync on_tip cup_started
0   peer sync on_tip cup_skipped
0   peer sync catchup
0   peer sync nack
0   nack
0   wire_decode_failed
426 tcp connect timeout seed=127.0.0.3:13030
853 peer reconnect decision seed=127.0.0.3:13030
852 cluster propose accepted
```

### Peers-list correlation

`tmp/cy-lab-peers.yaml`:

```text
tmp/cy-lab-peers.yaml:1
shards:
tmp/cy-lab-peers.yaml:2
  "0x2C":
tmp/cy-lab-peers.yaml:3
    - id: cy-proposer
tmp/cy-lab-peers.yaml:4
      peer: 127.0.0.1:13030
tmp/cy-lab-peers.yaml:6
    - id: cy-attester
tmp/cy-lab-peers.yaml:7
      peer: 127.0.0.2:13030
tmp/cy-lab-peers.yaml:9
    - id: cy-follower
tmp/cy-lab-peers.yaml:10
      peer: 127.0.0.3:13030
```

Attester действительно стартует с self listen `127.0.0.2:13030`, принимает inbound от proposer и дополнительно dial-ит proposer из `--peers-list`:

```text
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:1
[15:57:53.905] #INFO: peer listener active at 127.0.0.2:13030
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:7
[15:57:55.496] #INFO: peer session open seed=inbound node_id=cy-proposer domain_hi=0x2C
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:13
[15:58:01.603] #INFO: peer tcp connect started seed=127.0.0.1:13030 remote=127.0.0.1:13030
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:21
[15:58:01.789] #INFO: peer session open seed=127.0.0.1:13030 node_id=cy-proposer domain_hi=0x2C
```

Недоступный follower из списка создаёт reconnect noise:

```text
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:10
[15:58:01.603] #INFO: peer reconnect decision seed=127.0.0.3:13030 reason=retry_after_close repeated=0
logs/2026-05-13/pwmd-peer-cy-attester-155753.log:23
[15:58:02.326] #WARN: peer tcp connect timeout seed=127.0.0.3:13030 remote=127.0.0.3:13030 timeout_ms=1000
```

Это усиливает ощущение "постоянной сетевой активности", но не совпадает с `Sync progress 99%`: прогресс и `live_hdr/apply ok` привязаны к `cy-proposer`, а не к `127.0.0.3`.

### Code evidence

`Sync progress` формируется из `maybe_log_sync_prog`, а `99%` получается при положительном `rem`, даже если деление дало 100:

```text
crates/pwmd/src/transport/peer_session/sync_live.rs:92
    let rem = goal.saturating_sub(local_h);
crates/pwmd/src/transport/peer_session/sync_live.rs:94
    if rem > 0 && pct == 100 {
crates/pwmd/src/transport/peer_session/sync_live.rs:95
        pct = 99;
```

Standby suppress ограничен только short-tail с `snap.rem <= 1`:

```text
crates/pwmd/src/transport/peer_session/sync_live.rs:100
/// Live "short tail": following peer close to tip without epoch CUP — suppress noisy `Sync progress` on Standby only.
crates/pwmd/src/transport/peer_session/sync_live.rs:107
    let tip_lag = peer_tip_h.saturating_sub(local_h);
crates/pwmd/src/transport/peer_session/sync_live.rs:108
    !cup_active && peer_tip_h > 0 && tip_lag < SYNC_CUP_SHORT_TAIL_MAX && snap.rem <= 1
crates/pwmd/src/transport/peer_session/sync_live.rs:171
        let tail_quiet = sync_prog_tail_quiet(st.cup_active, peer_tip_h, local_h, &snap);
crates/pwmd/src/transport/peer_session/sync_live.rs:172
        let suppress_short_tail_standby = matches!(app.seal_role, SealRole::Standby) && tail_quiet;
```

`on_tip` вызывает progress до live header request. При `lag=2` это даёт видимую `99%`, после чего обычная live-ветка запрашивает headers:

```text
crates/pwmd/src/transport/peer_session/sync_live.rs:564
    let lag = head_h - local_h;
crates/pwmd/src/transport/peer_session/sync_live.rs:566
    maybe_log_sync_prog(app, node_id, local_h, head_h, persisted_h).await;
crates/pwmd/src/transport/peer_session/sync_live.rs:630
    let cup_req =
crates/pwmd/src/transport/peer_session/sync_live.rs:631
        cup_on || lag >= SYNC_CUP_LAG_MIN || (lag >= SYNC_CUP_SHORT_TAIL_MAX && live_stall >= 2);
crates/pwmd/src/transport/peer_session/sync_live.rs:670
    info!(
crates/pwmd/src/transport/peer_session/sync_live.rs:672
        "peer sync on_tip live_hdr node_id={} next_from={} req_lim={} lag={}",
```

Успешное применение блоков сбрасывает live stall и снова обновляет progress state:

```text
crates/pwmd/src/transport/peer_session/sync_live.rs:1256
    match apply_blk_batch(app, &apply_rows).await {
crates/pwmd/src/transport/peer_session/sync_live.rs:1265
                let st = peer_sync(&mut hs, node_id);
crates/pwmd/src/transport/peer_session/sync_live.rs:1266
                st.live_stall = 0;
crates/pwmd/src/transport/peer_session/sync_live.rs:1275
            maybe_log_sync_prog(app, node_id, local_h, tip_h, persisted_h).await;
crates/pwmd/src/transport/peer_session/sync_live.rs:1278
                "peer sync apply ok node_id={} blocks={}",
```

## Ranked hypotheses

### H1 - Confirmed: Standby progress policy leaks `rem=2/3` live-following

Observed pattern:

- `Sync progress 99% rem=2/3`;
- matching `peer sync on_tip ... lag=2`, `cup_on=false`;
- `peer sync on_tip live_hdr`;
- `peer sync apply ok blocks=1/2`;
- no CUP, no NACK, no wire decode failure.

Interpretation: attester is synchronized enough to live-follow proposer, but the console policy treats `rem>1` as visible progress even for Standby. This is a UX/observability semantics issue, not a confirmed transport correctness bug.

### H2 - Contributing: peer mesh now dials absent `cy-follower`

Observed pattern:

- `tmp/cy-lab-peers.yaml` includes `127.0.0.3:13030`;
- attester repeatedly times out to that seed;
- no `node_id=cy-follower` sync progress path is visible in the evidence.

Interpretation: the new multishard `--peers-list` made the missing follower visible as reconnect noise. It does not explain the `Sync progress 99%` line directly, but it plausibly explains the operator's "constant synchronization/network activity" impression.

### H3 - Contributing: dual proposer sessions can duplicate announces

Observed pattern:

- attester has inbound proposer session and outbound seed session to `127.0.0.1:13030`;
- repeated `cluster propose accepted` and sometimes duplicate `peer sync on_tip ... live_hdr` appear for nearby heights;
- eventual `apply ok` follows.

Interpretation: dual same-node sessions can create small transient `lag=2` windows or repeated requests. This remains below CUP thresholds and recovers via live sync.

### H4 - Rejected for this run: stuck CUP / NACK storm / wire decode failure

Evidence:

- `peer sync on_tip cup_started`: `0`
- `peer sync on_tip cup_skipped`: `0`
- `peer sync catchup`: `0`
- `peer sync nack`: `0`
- `nack`: `0`
- `wire_decode_failed`: `0`

This rejects the suspected catchup/NACK root cause for the provided latest logs.

## Repro status

No new long-running process was started. The investigation used the operator-provided live CY logs, which already contain the symptom and the corresponding same-shard peer trace. Reproduction remains non-deterministic because `99%` requires a timing window where proposer tip advances enough for `rem=2/3` before the Standby apply path catches up; in the latest log it happened 9 times over the captured interval.

## Commands run

- `Get-ChildItem logs/**/pwmd-cy-attester-*.log` / `logs/**/pwmd-peer-cy-attester-*.log` sorted by `LastWriteTime`: PASS.
- `rg -n -i "Sync progress|catchup|nack|standby|cluster|peer|tip|wire|error|warn|sync" ...`: PASS; large extract captured by Cursor tool output.
- Targeted `rg -n` over `crates scripts docs tasks` for `maybe_log_sync_prog`, `sync_live`, `standby sync`, `peer sync on_tip`, `nack`, `peers-list`: PASS.
- Counted key signatures in latest attester pair: PASS.
- Read `sync_live.rs`, `peer_list.rs`, `tmp/cy-lab-peers.yaml`: PASS.
- CQDS note: `colloquium-cqds-mcp` skill and descriptors were read; current `CallMcpTool` wrapper exposed no arguments field, so this investigation used the allowed terminal `rg` fallback.

## Instrumentation and cleanup

- Temporary instrumentation: none.
- Production logic changes: none.
- Reverted: not applicable.
- Processes started by this agent: none.
- Cleanup: no `pwmd`/`pwm-tui` process was started by this investigation, so nothing was killed.

## Next steps

- `pwm-coding`: decide whether Standby quiet-tail should suppress `rem <= 3` or all `lag < SYNC_CUP_SHORT_TAIL_MAX` live-following progress, or emit a distinct low-rate `Sync live-following` line instead of `Sync progress 99%`.
- `pwm-coding`: consider deduping same `node_id` proposer sessions or suppressing duplicate live header requests when an equivalent request is already in flight.
- `pwm-coding` / scripts: for CY two-node runs, either do not include absent `cy-follower` in `tmp/cy-lab-peers.yaml`, or lower reconnect noise for seeds that are known optional.
- `pwm-testing`: add an acceptance check that distinguishes harmful sync patterns (`cup_started`, `nack`, `rem` growing, persistent `cup_on=true`) from healthy live-following with transient `rem=2`.

## Open risks / unknowns

- The root cause is confirmed for the latest provided logs, but a future run with actual `nack`/`catchup` lines would be a different incident and should be investigated separately.
- This report does not measure CPU impact of the `127.0.0.3` reconnect loop; it only establishes it is not the direct source of `Sync progress 99%`.

## Handoff footer

```yaml
agent: pwm-debug
result: PASS
verbosity_focus: transport:peers
instrumentation:
  reverted: yes
  files: []
repro:
  deterministic: no
  notes: "Symptom observed 9 times in latest logs; no new soak started."
artifacts:
  rca_md: docs/debug/20260519-attester-sync-progress-99-rca.md
  logs:
    - logs/2026-05-13/pwmd-cy-attester-155753.log
    - logs/2026-05-13/pwmd-peer-cy-attester-155753.log
cleaned: yes
token_usage:
  source: estimate
  input: null
  output: null
  total: 30000
  confidence: low
```
