# RCA: blockchain sync quiet-tail acceptance after CUP short-tail fixes

Дата: 2026-05-13  
Тикет: `tasks/20260514-blockchain-sync-quiet-acceptance.json`  
Фокус: `transport:peer-sync`

## Краткий вывод

Текущий операторский симптом "sync never stops" в свежем CY-хвосте не выглядит как залипший epoch CUP. Последний полный attester-прогон `11:41:15` показывает здоровый live-following: каждый новый tip proposer даёт `lag=1`, затем `live_hdr`, затем `apply ok`; `cup_on=false`, `cup_started=0`, `cup_skipped=0`, `cup_demoted=0`, NACK нет.

Корневая причина наблюдаемого шума: после догона attester продолжает следовать за постоянно растущим proposer tip, а `maybe_log_sync_prog` печатает `Sync progress 99%/100%` для каждого нового `goal`. Это операторски похоже на "синхронизация не выключилась", но по transport-логам это live short-tail на 1 блок, не CUP и не бесконечный catch-up.

## Evidence

Основной свежий артефакт:

- `logs/2026-05-13/pwmd-peer-cy-attester-114115.log`
- `logs/2026-05-13/pwmd-cy-attester-114115.log`
- terminal snapshot: `C:\Users\Alexander\.cursor\projects\p-opt-docker-PWM-cryptocurrency\terminals\2.txt`

Подсчёт по свежему `114115`:

- `peer sync on_tip live_hdr`: 86
- `peer sync apply ok`: 68
- `headers rejected`: 7
- `Sync progress`: 35
- `peer sync on_tip cup_started`: 0
- `peer sync on_tip cup_skipped`: 0
- `cup_demoted`: 0
- `nack`: 0
- `catchup`: 0 в sync-смысле; единственное совпадение было `tx_catchup_ms` в строке cluster attest config.

Характерный фрагмент live-following:

```text
logs/2026-05-13/pwmd-peer-cy-attester-114115.log:53
[11:41:32.639] #INFO: peer sync on_tip lag node_id=cy-proposer local_h=17380 head_h=17381 lag=1 persisted_h=17380 live_stall=0 cup_on=false can_cup=true
logs/2026-05-13/pwmd-peer-cy-attester-114115.log:54
[11:41:32.639] #INFO: peer sync on_tip live_hdr node_id=cy-proposer next_from=17381 req_lim=128 lag=1
logs/2026-05-13/pwmd-peer-cy-attester-114115.log:55
[11:41:32.661] #INFO: peer sync apply ok node_id=cy-proposer blocks=1
```

Характерный операторский шум при фактическом `rem=0`:

```text
logs/2026-05-13/pwmd-cy-attester-114115.log:16
[11:41:38.917] #INFO: Sync progress 99% rem=1 goal=17384 mem=17383/17384 disk=17380/17384
logs/2026-05-13/pwmd-cy-attester-114115.log:17
[11:41:38.939] #INFO: Sync progress 100% rem=0 goal=17384 mem=17384/17384 disk=17380/17384
```

Диск отстаёт ожидаемо только до standby checkpoint:

```text
logs/2026-05-13/pwmd-cy-attester-114115.log:20
[11:41:51.510] #INFO: standby sync checkpoint range=17390..17390 flush_iv=10
logs/2026-05-13/pwmd-cy-attester-114115.log:31
[11:41:54.711] #INFO: Sync progress 99% rem=1 goal=17392 mem=17391/17392 disk=17390/17392
logs/2026-05-13/pwmd-cy-attester-114115.log:32
[11:41:54.734] #INFO: Sync progress 100% rem=0 goal=17392 mem=17392/17392 disk=17390/17392
```

В терминале attester та же форма продолжается до остановки процесса: `99% rem=1` / `100% rem=0`, `mem=goal`, `disk` обновляется на кратных 10 (`standby sync checkpoint`). Это подтверждает "живое следование за tip", а не накопление хвоста.

## Timeline of Modes

1. Startup: attester грузит snapshot `tip_h=17380`, затем ready.
2. First visible progress: `Sync progress 100% rem=0 goal=17380`, то есть узел стартует уже выровненным по сохранённому состоянию.
3. Live short-tail loop: proposer продолжает sealing; каждое новое объявление tip приходит с `lag=1`.
4. Для каждого `lag=1`: `on_tip lag` -> `on_tip live_hdr` -> `apply ok blocks=1`.
5. Progress logger печатает `99% rem=1` перед apply и `100% rem=0` после apply для нового `goal`.
6. Standby disk checkpoint происходит пачками по `flush_iv=10`; между checkpoint `disk < goal`, но `mem=goal`, поэтому это persistence cadence, не transport lag.

В этом свежем хвосте нет переходов в CUP:

- нет `peer sync on_tip cup_started`;
- нет `peer sync catchup start/progress/finish/fail`;
- нет `peer sync cup_demoted_short_tail`;
- нет `peer sync nack`.

## Why prior CUP fixes did not change the symptom

Prior short-tail fixes addressed active/entry behavior around CUP. The current symptom is different: the peer is no longer stuck in CUP, but the operator-visible progress line still fires for every new short-tail goal.

Code path:

- `on_tip` records `lag`, calls `maybe_log_sync_prog`, demotes active short-tail CUP via `try_demote_cup_tail`, then for `lag > 0` decides between CUP and live.
- For `lag=1`, `cup_req = false` unless `cup_on` is true; logs show `cup_on=false`.
- The code falls through to `peer sync on_tip live_hdr`.
- `on_blk_batch` applies one block, resets `live_stall=0`, and calls `maybe_log_sync_prog` again.
- `sync_prog_tick` allows a new log after a completed sync when a new peer `goal` appears (`lag_resume`) and then logs `100%` on completion for that new goal.

Net effect: with a live proposer, "quiet tail" is not actually quiet if the acceptance script counts all `Sync progress` lines. A synchronized standby can still emit progress continuously because it is following newly produced blocks.

## Ranked hypotheses and confirmation patterns

### H1 - Confirmed: live-following progress noise

Pattern:

- Many `Sync progress 99% rem=1` followed quickly by `Sync progress 100% rem=0`.
- Matching peer log has `lag=1`, `cup_on=false`, `live_stall=0`, `on_tip live_hdr`, `apply ok blocks=1`.
- No `catchup start/progress/finish/fail`, no `cup_started`, no NACK.

Interpretation: sync is not "stuck"; it is doing normal live short-tail block following. Operator quiet criterion is too broad if it treats every progress line as bad.

### H2 - Contributing: duplicate live header requests create minor churn

Pattern:

- Repeated `on_tip live_hdr next_from=N` before previous response is fully reconciled.
- Occasional `headers rejected reason=continuity_start expected=N got=Some(N-1)`.
- Followed by another `on_tip live_hdr` and `apply ok blocks=1`.
- `live_stall` temporarily becomes 1, then resets to 0 after apply.

Interpretation: duplicate/stale live hdr responses add peer-log noise and transient `live_stall`, but at `lag=1` they do not trigger CUP. This is a useful cleanup area because it can inflate "activity" without indicating real catch-up.

### H3 - Not observed in latest tail: stuck active CUP after demotion

Pattern that would confirm:

- `peer sync on_tip lag ... lag<32 ... cup_on=true`
- then either `peer sync cup_demoted_short_tail ...` or, if broken, repeated `peer sync on_tip cup_started ... cup_on=true`
- live batch logs absent or live hdr/blk responses ignored;
- `peer sync catchup progress` without `catchup finish`, or wrong-epoch `on_cup_done` suspicion.

Current evidence argues against this for `114115`: all observed tail work is live, `cup_on=false`, and CUP pattern counts are zero.

### H4 - Not observed in latest tail: retry/NACK storm

Pattern that would confirm:

- `peer sync nack node_id=... reason=...`
- `peer sync catchup aborted by nack ...`
- repeated `catchup fail` / retry backoff lines.

Current evidence argues against it: NACK count is zero in the fresh attester peer log.

## Repro status

Attempted strict repro:

```text
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/cy_cluster_two_node_smoke.ps1 -SmokeSeconds 120 -RequireQuietTail
```

Result: did not reach runtime. Windows PowerShell failed while parsing `scripts/cy_cluster_two_node_smoke.ps1` with `ParserError` around the `Write-Host` lines containing `>=` and nearby non-ASCII text. Therefore this command is not a deterministic transport repro in this environment until the script parse issue is fixed or run under an encoding-compatible shell.

Manual/recent CY logs are sufficient for this RCA because they contain the exact operator-visible symptom and the corresponding peer-mode evidence.

## Suggested fix area for pwm-coding

1. Adjust operator progress semantics, not CUP first. For `lag < SYNC_CUP_SHORT_TAIL_MAX` and `rem <= 1`, either suppress repeated `Sync progress 100% rem=0` for live-following, rate-limit it much more aggressively, or emit a distinct low-rate line such as `Sync live-following` / `Sync caught up`.
2. Update `scripts/cy_cluster_two_node_smoke.ps1 -RequireQuietTail` so quiet-tail does not fail only because a healthy live follower logs `Sync progress` for newly sealed blocks. Count harmful patterns instead: `rem` staying positive, `persisted_h << tip` growing, CUP/catchup failures, NACK, or `cup_on=true` below short-tail.
3. Add a peer-mode transition log: `idle`, `live_short_tail`, `cup_active`, `cup_demoted`, `cup_backoff`. This makes future "sync never stops" reports directly distinguish console progress from transport mode.
4. In `on_tip`/`ask_hdr`, avoid issuing duplicate live header requests for the same `next_from` while `wait_hdr_from` / `in_hdr` already covers it. This should reduce `headers rejected ... expected=N got=Some(N-1)` churn.
5. Keep the prior CUP demotion fix; do not revert it. It is still necessary for the H3 pattern, but it is not the dominant cause in the latest evidence.

## Minimal instrumentation proposal

No temporary instrumentation was added. If more signal is needed, add one debug-only transition log in `sync_live.rs` around `on_tip` after CUP demotion and before `ask_hdr`, gated by `#[cfg(debug_assertions)]`, with fields:

- `mode=live_short_tail|cup_active|idle`
- `lag`
- `cup_on`
- `live_stall`
- `in_hdr`
- `wait_hdr_from`
- `in_blk`
- `wait_blk_len`
- `pend_blk_len`

This is optional; the current peer logs already prove the latest symptom is live-following, not CUP.

## Cleanup

- Rust source changes: none.
- Temporary instrumentation: none.
- Reverted: not applicable.
- Long-running process cleanup: no `pwmd` process was started by the failed strict smoke; no kill was needed.

## Orchestrator handoff

- `agent`: `pwm-debug`
- `result`: `PASS`
- `verbosity_focus`: `transport:peer-sync`
- `instrumentation`: none; `reverted: yes`
- `repro`: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/cy_cluster_two_node_smoke.ps1 -SmokeSeconds 120 -RequireQuietTail`; `deterministic: no` for transport because script parse failed before runtime
- `artifacts`: `docs/debug/20260514-blockchain-sync-quiet-rca.md`, `logs/2026-05-13/pwmd-peer-cy-attester-114115.log`, `logs/2026-05-13/pwmd-cy-attester-114115.log`
- `commands`: CQDS `cq_help`, `cq_project_ctl list_projects`, `cq_files_ctl start_grep`; host `cq_process_ctl spawn/wait/io` for strict smoke (failed at script parse); local `rg`/Python log extraction (passed)
- `cleanup`: cleaned yes; no spawned node processes remained from the failed strict smoke
- `token_usage`: `{ "source": "estimate", "input": 52000, "output": 9000, "total": 61000, "confidence": "medium" }`
