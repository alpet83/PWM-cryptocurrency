# S2.1 external lease backend design

Дата: 2026-05-09  
Тикет: `tasks/20260509-s21-external-lease-backend.json`  
Роль: `pwm-debug`, design gate без правок product Rust.

## 1. Recommendation

Для MVP рекомендую **file-lock based lease backend over a JSON lease record** в явном `lease_dir` на локальном или shared volume. Это самый подходящий шаг для текущего проекта: уже есть process-local `LeaseRuntime`, существующие поля lease в handshake/heartbeat (`owner_id`, `term`, `expires_at_ms`, `last_tip`, `fence`) и testnet/dev сценарии запускаются вокруг локальных процессов, Docker volume и host-mode harness. Такой backend не требует операционного etcd/Consul, не тянет новый сетевой сервис в MVP и хорошо покрывает главный gap S2: два независимых процесса с тем же validator key на одном host/shared volume не должны одновременно seal.

Граница рекомендации: file-lock backend допустим как **MVP local/shared-volume coordinator**, не как полноценный multi-host consensus service. Если target testnet переходит к независимым машинам без надежного общего POSIX-like lock volume, следующий backend должен быть lightweight KV с server-side CAS/revision и lease TTL (например etcd/Consul; Redis только при строгом CAS + persistence + monotonic fencing discipline). Интерфейс backend нужно сразу проектировать так, чтобы file backend был первой реализацией, а KV добавлялся без переписывания seal-loop.

## 2. Lease Record Schema

Один record на `validator_identity_hash`; для file backend путь вида:

`<lease_dir>/<validator_identity_hash>.lease.json`

Минимальная схема:

```json
{
  "schema_version": 1,
  "owner_id": "node_instance_id-or-random-boot-id",
  "validator_identity_hash": "hex-or-stable-hash",
  "term": 7,
  "fence": 7,
  "expiry": 1778320000000,
  "last_tip": {
    "height": 123,
    "hash": "optional-tip-hash"
  },
  "updated_at": 1778319990000
}
```

Field semantics:

- `owner_id`: unique process/node instance for this boot. It must change on restart unless the node can prove it still owns the lease.
- `validator_identity_hash`: key namespace and sanity check; mismatch between path and record is corruption.
- `term`: monotonically increasing ownership epoch. Increment on acquire after empty/expired record and conditional takeover; unchanged on renew.
- `fence`: monotonically increasing fencing token, persisted with the winning ownership epoch. For MVP it may equal `term`; keep it separate so a KV backend can map it to revision/mod_revision later.
- `expiry`: absolute backend timestamp in unix ms after which a different owner may attempt takeover.
- `last_tip`: last locally observed canonical tip by the current owner. At minimum store `height`; include `hash` when available to help diagnostics and stale-tip checks.
- `updated_at`: backend timestamp for last successful write; used for diagnostics, IO freshness checks and clock-skew warnings, not as the sole expiry condition.

## 3. Atomic Operations

Implement a narrow trait, e.g. `LeaseBackend`, returning both decision and latest record. The seal-loop should call it before every local seal attempt and fail closed on any uncertain result.

### `acquire(validator_identity_hash, owner_id, last_tip, now)`

Allowed when no record exists, the record is unreadable only as a handled corruption policy, or the existing record is expired past takeover guard. For file backend:

1. Open lock file in `lease_dir`.
2. Take exclusive OS file lock.
3. Read record if present.
4. Validate schema and `validator_identity_hash`.
5. If absent: write record with `term=1`, `fence=1`, `expiry=now+ttl`.
6. If present and not expired: return `held_by_peer`.
7. If present and expired enough for takeover: behave as conditional takeover below.
8. Write through temp file + flush + atomic rename while still holding the lock.

### `renew(expected_owner_id, expected_term, expected_fence, last_tip, now)`

CAS by `(owner_id, term, fence)`:

- Success only if the stored record still has the same `owner_id`, `term`, and `fence`, and `now <= expiry + allowed_clock_skew`.
- Update `expiry`, `last_tip`, `updated_at`.
- Do not increment `term`/`fence`.
- If the record is missing, owned by another process, has a different term/fence, or is already too far expired, return `lost` and suppress seal.

### `release(expected_owner_id, expected_term, expected_fence, reason)`

CAS by `(owner_id, term, fence)`:

- If the caller still owns the record, either delete it or write a tombstone with immediate `expiry=now`.
- If the record moved to another owner/term/fence, return idempotent `not_owner`; do not delete.
- Release is best-effort on graceful shutdown; correctness must not rely on it.

### `conditional_takeover(expected_term, expected_fence, observed_expiry, new_owner_id, last_tip, now)`

CAS by `term/fence/expiry`:

- The standby first observes a record, waits until `expiry + takeover_ms`, verifies its local tip is not stale beyond `max_tip_lag`, then attempts takeover.
- Success only if the record still has the observed `term`, `fence`, and `expiry`; this prevents two standbys from both winning after the same stale observation.
- On success, write `owner_id=new_owner_id`, `term=old.term+1`, `fence=old.fence+1`, `expiry=now+ttl`, `last_tip`, `updated_at`.
- On CAS miss, return latest record and remain standby.

For file backend the exclusive file lock serializes the compare/read/write critical section. For KV backend the same semantics map to `compare(mod_revision/value fields) -> put`.

## 4. Failure Semantics

**Process crash.** OS file locks are released when the process dies, but the JSON record remains. Other processes must not acquire until `expiry + takeover_ms` and stale-tip checks pass. Graceful `release` improves recovery time only; it is not required for safety.

**Stale lock / stale record.** Treat a held OS lock as transient backend contention and suppress seal while waiting. Treat a stale record as takeover-eligible only through `conditional_takeover`; never overwrite it without matching `term/fence/expiry`.

**Clock skew.** File backend has no server clock, so MVP must assume bounded skew for processes sharing a host/volume. Use conservative `ttl_ms`, `renew_interval <= ttl/3`, `takeover_ms >= max_clock_skew_ms + max_io_pause_ms`, and reject takeover if `now + skew_guard < expiry + takeover_ms`. Multi-host deployments with unbounded skew should move to KV/server-clock semantics before being supported.

**IO errors.** Fail closed: no acquire/renew means no seal. Emit structured reason (`lease_backend_io_error`, `lease_backend_corrupt_record`, `lease_backend_lock_timeout`) and increment lease reject/loss metrics. Do not keep sealing on cached ownership after a renew write fails.

**Backend unavailable.** Active node loses sealing permission after the first failed renew/check; standby remains standby. Operator-visible status should say `seal_suppressed_by_fence reason=lease_backend_unavailable`. Recovery is by successful renew/acquire/takeover only.

**Corrupt or mismatched record.** Fail closed by default and require operator repair. A future `--force-lease-repair` can be explicit, offline and out of MVP.

## 5. Security And Consistency Notes

The fencing token is the core stale-owner defense. The owner may seal only while it holds `(owner_id, term, fence)` in the backend; the seal loop must re-check before each block and stop immediately on renew/CAS loss. Heartbeat/hello already carry lease fields; peers should prefer the record-backed `term/fence` and reject or warn on same-validator messages from an older fence.

MVP consistency contract:

- Never accept local sealing authority from CLI role alone in `single_sealer`; role only decides whether the process tries to acquire or stays standby.
- Include current `fence` in operator logs, peer status and metrics. If product code can attach it to block/debug metadata without wire breakage, do so; otherwise keep it as a local/peer guard until a protocol bump slice formalizes block-level fencing.
- A stale owner that wakes after pause/crash must fail `renew` because `term/fence` no longer match. It must transition to standby and must not seal another block.
- Do not use peer gossip as the source of ownership. Gossip is observability; the external backend is the authority.

## 6. Migration From Current S2

1. Extract current in-memory `step_lease` behavior behind a backend-facing state machine: local process state remains `LeaseRuntime`, external authority moves behind `LeaseBackend`.
2. Add config:
   - `--seal-lease-backend process-local|file` (default can stay `process-local` for one compatibility slice, but testnet profile should require `file`).
   - `--seal-lease-dir <DIR>` / `PWM_SEAL_LEASE_DIR` for file backend.
   - optional `--seal-lease-clock-skew-ms`.
3. Preserve current defaults (`ttl_ms`, `takeover_ms`, `max_tip_lag`) but document that external backend is required for same-key multi-process failover.
4. Keep process-local backend as test/dev only and mark it unsafe for independent-process HA.
5. Wire `run_lease_gate()` to call backend `acquire/renew/takeover` instead of the static `OnceLock<HashMap<...>>`.
6. Update handshake/heartbeat lease fields from backend-backed runtime, not only process memory.
7. After file backend acceptance is green, change `single_sealer` testnet profiles/runbooks to require explicit external backend before allowing two same-key processes.

## 7. Acceptance Tests

Core unit tests:

- File backend `acquire` creates a valid record with expected `owner_id`, `validator_identity_hash`, `term=1`, `fence=1`, `expiry`, `last_tip`, `updated_at`.
- `renew` succeeds only with matching `(owner_id, term, fence)` and updates expiry/tip without changing term/fence.
- `release` deletes/tombstones only when owner/term/fence match; stale owner release cannot remove a newer owner.
- `conditional_takeover` succeeds after `expiry + takeover_ms` with matching observed `(term, fence, expiry)` and increments both `term` and `fence`.
- Two concurrent acquire/takeover attempts against the same file yield exactly one winner.
- Corrupt/mismatched record and IO-open failure return fail-closed decisions.

Two-process same-key acceptance:

1. Start node A and node B as separate OS processes with same validator key, same genesis, same `--deployment-profile single-sealer`, same `--seal-lease-backend file`, same `--seal-lease-dir`, distinct data dirs and ports.
2. Assert exactly one process logs `seal_lease_acquired`/`seal_lease_renewed` and produces blocks; the other logs `seal_suppressed_by_fence` or standby state and produces no local blocks.
3. Kill the active process with no graceful release. Assert standby waits until `expiry + takeover_ms`, then logs `seal_takeover_committed` with `term` and `fence` greater than the old record and begins sealing.
4. Restart the old active process. Assert it does not resume sealing with stale ownership; it either remains standby or reacquires only after the current owner expires and CAS succeeds.
5. Submit transactions during failover and verify canonical chain grows by a single active sealer at a time; no competing same-height local sealed blocks with the same validator identity appear across logs/state.
6. Repeat with induced backend unavailable/permission error and assert both processes fail closed rather than sealing on cached state.

Suggested command family for `pwm-testing`: host-mode `cq_process_ctl`/Git Bash orchestration with temp dirs under `tasks/20260509-s21-external-lease-*`, capturing node logs as artifacts rather than chat output.

## 8. Coding Checklist For `pwm-coding`

1. Add `LeaseBackend` abstraction and keep current process-local map as `ProcessLocalLeaseBackend` for unit compatibility.
2. Implement `FileLeaseBackend` with exclusive file lock, JSON record, temp-file write, flush and atomic rename.
3. Replace `step_lease` static map calls in `run_lease_gate()` with backend `acquire/renew/conditional_takeover` decisions while preserving `LeaseRuntime`/metrics/log events.
4. Add config/CLI/env for backend selection and lease dir; fail startup if `file` backend lacks a usable directory.
5. Make all backend errors fail closed and surface structured reasons in existing lease logs/metrics.
6. Ensure handshake/heartbeat exports backend-backed `owner_id`, `term`, `expiry`, `last_tip`, `fence`.
7. Add unit tests for file backend CAS/fencing and process-local compatibility.
8. Add two-process same-key integration harness: one active, one standby, crash active, standby takeover, stale old active blocked.

## Verdict

`PASS` for design gate. Recommended MVP path is file-lock JSON backend for current local/shared-volume testnet scope, with the backend trait shaped for a later KV implementation when multi-host HA becomes a supported target.

## Participation / token estimate

```yaml
agent: pwm-debug
result: PASS
verbosity_focus: seal:lease
instrumentation:
  files: []
  reverted: yes
repro:
  deterministic: null
  command: "design-only; no runtime repro"
artifacts:
  - docs/reviews/20260509-s21-external-lease-design.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 13000
  confidence: medium
```
