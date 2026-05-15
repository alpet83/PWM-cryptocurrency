# Single-sealer deployment profiles and same-key failover design

Дата: 2026-05-09  
Тикет: `tasks/20260509-single-sealer-failover-profiles.json`  
Роль: `pwm-debug`, design gate без product-code правок.

## Executive summary

Для MVP default должен быть **`single_sealer`**: на один validator identity/key допускается ровно один активный local sealer. Второй узел с тем же ключом может существовать как standby/sync node, но его local seal-loop обязан быть выключен или fenced lease-guard'ом. Это сохраняет текущий deterministic proposer contract: ожидаемый proposer выводится из validator set и height, а не из локальной топологии, wall-clock или порядка появления пиров.

**`multi_sealer_experimental`** допустим только как явно включённый профиль для исследовательских волн. Он не должен маскировать split-brain как норму: если два узла с одним proposer key одновременно seal'ят разные блоки на одной высоте, это не "повышенная доступность", а конкурирующие локальные истории с валидными подписями и разными `prev_hash` / `ts` / `tx_root` / `state_root`.

Ключевое добавление владельца принято: даже без явного cluster-mode два процесса с одинаковым validator identity/key должны иметь runtime-координацию coexistence/failover. Минимальный MVP-путь: duplicate detection через hello/status + строгая policy matrix + lightweight lease/fencing для same-key пары.

## Profile Contract

### `single_sealer` (default MVP)

Назначение: один активный proposer на validator identity, standby-ноды только синхронизируются и обслуживают read/RPC там, где это безопасно.

Обязательные свойства:

- Profile default для testnet/MVP и для обычного запуска `pwmd`.
- Local seal-loop включён только если узел владеет active lease для своего `validator_identity`.
- Same-key peer detection по handshake/status не является ошибкой само по себе: ошибка возникает, если оба узла заявляют `seal_role=active` или оба фактически seal'ят.
- Standby должен использовать существующий смысл `--debug-disable-seal-loop` как временный operator knob, но product policy не должна навсегда зависеть от debug-флага. S1/S2 должны ввести явный runtime/profile knob.
- При потере lease active node обязан перейти в `fenced_standby` до следующей успешной аренды; при восстановлении связи с более свежим active tip standby не seal'ит локальные "догоняющие" блоки.

Рекомендуемые дефолты:

- `profile=single_sealer`;
- `same_validator_policy=strict` для production/testnet;
- `takeover_timeout = 3 * seal_interval + jitter`, при текущем 2s interval стартовый ориентир 6-8s;
- `lease_ttl = takeover_timeout + 1 * seal_interval`, стартовый ориентир 8-10s;
- `standby_seal_loop=disabled_until_lease_acquired`.

### `multi_sealer_experimental` (guarded)

Назначение: controlled experiments по multi-proposer/fork-choice, chaos и будущему cluster consensus.

Guardrails:

- Включается только явным CLI/env/config: например `--deployment-profile multi-sealer-experimental`.
- Startup log обязан писать крупный warning: профиль не является deterministic single-proposer guarantee.
- Same-key duplicate active в этом профиле может быть `warn` вместо `reject`, но только при явном `allow_same_validator_active=true`.
- Требует включённых metrics/log signals по competing height, unexpected proposer, duplicate lease owner и fork-choice outcome.
- Не используется как default acceptance profile для MVP parity/e2e.

## Duplicate Proposer Detection

Нужно различать три уровня идентичности:

1. **Validator identity/key**: account/public key из genesis validator set, то есть то, чем подписывается block header.
2. **Node instance id**: ephemeral/persistent node id процесса, нужен для lease ownership и диагностики.
3. **Runtime profile/role**: `single_sealer.active`, `single_sealer.standby`, `multi_sealer_experimental.active`, `observer`.

Минимальные runtime signals:

- Handshake/status capability fields:
  - `validator_identity` или hash публичного validator key;
  - `node_instance_id`;
  - `deployment_profile`;
  - `seal_role`;
  - `lease_epoch`, `lease_owner`, `lease_expires_at_unix_ms` если lease включён.
- Peer/session event:
  - `duplicate_validator_seen`;
  - `same_validator_active_conflict`;
  - `same_validator_standby_seen`;
  - `same_validator_policy_action`.
- Seal-loop event:
  - `seal_lease_acquired`;
  - `seal_lease_renewed`;
  - `seal_lease_lost`;
  - `seal_suppressed_by_fence`;
  - `seal_takeover_started`;
  - `seal_takeover_committed`.

Detection sources:

- **Direct peer handshake**: fastest signal when same-key nodes are connected.
- **Local lease backend**: authoritative for active/standby decision in MVP.
- **Chain observation**: fallback evidence when a block signed by our validator key appears at unexpected height/hash while local node believes it is active.
- **Operator status endpoint**: exposes current role and last conflict reason for runbooks.

## Strict vs Warn Policy Matrix

| Profile | Same validator peer seen | Both claim standby | One active, one standby | Both claim active | Active block observed from duplicate |
|---|---|---|---|---|---|
| `single_sealer`, `strict` | Log + metric | OK | OK | Reject/fence local loser | Fence local node, require lease recheck |
| `single_sealer`, `warn` | Log + metric | OK | OK | Warn + prefer lease owner | Warn + trigger catch-up, no automatic local seal if lease stale |
| `multi_sealer_experimental`, guarded | Log + metric | OK | OK | Warn if explicitly allowed, else reject | Warn + fork-choice metrics |
| No profile / legacy | Treat as `single_sealer strict` after S1 | N/A | N/A | N/A | N/A |

Default recommendation: **`single_sealer strict`** once S1 guards exist. During S0/S1 rollout, a temporary `warn` mode may be useful to avoid breaking existing local harnesses, but it must be an explicit compatibility window with a removal date.

## Lease and Fencing Strategy

### MVP lightweight lease (S2)

Preferred initial approach: shared lease record in a simple backend already acceptable for the deployment environment:

- file lock / atomic lease file under a shared volume for two processes on one host or shared filesystem;
- HTTP/RPC lease coordinator endpoint only if there is already an operator-side supervisor;
- DB/KV backend later if ClickHouse or another storage layer becomes an accepted runtime dependency.

Lease record fields:

```text
validator_identity
genesis_hash
network_id
deployment_profile
owner_node_instance_id
owner_listen_addr
epoch
term
expires_at_unix_ms
last_tip_height
last_tip_hash
fencing_token
```

Fencing rule:

- Active may seal only when it holds a non-expired lease and includes the current in-memory `fencing_token` in its seal-loop guard state.
- Standby may attempt takeover only after `expires_at + takeover_grace`, then must write a higher `term` atomically.
- If an active node sees a higher term for the same validator identity, it immediately transitions to `fenced_standby` and suppresses local sealing.
- Blocks do not need to carry the token in MVP; the token is runtime fencing, not consensus data. Adding block-level term evidence belongs to S3/RFC work.

### Why not clock-only or mid-second

`--debug-align-seal-mid-second` can reduce accidental wall-clock drift, but it does not prove a single proposer. Different mempool batches, local snapshot state, scheduling, peer delivery order and `SystemTime` skew can still produce different valid headers signed by the same key. `SealTimeMode::DeterministicHeight` is useful for hash parity tests, but it changes timestamp semantics and is not a production failover mechanism.

## Failover State Machine

States for a two-node same-key setup:

1. **`booting`**: load config/genesis, compute `validator_identity`, create/read `node_instance_id`.
2. **`discovering`**: handshake/status with peers and lease backend probe.
3. **`standby_syncing`**: seal-loop suppressed, sync/catch-up active, local tip follows network.
4. **`active_sealing`**: lease held, seal-loop allowed.
5. **`suspect_active_lost`**: standby sees lease expired or active heartbeat stale; waits takeover timeout and verifies latest tip.
6. **`takeover_candidate`**: standby atomically writes higher lease term and fencing token.
7. **`active_after_takeover`**: new active starts sealing only after lease write succeeds and local tip is caught up to last known active tip.
8. **`fenced_standby`**: node lost lease or saw higher term; local sealing disabled until manual or automatic rejoin.
9. **`split_brain_detected`**: two active owners/terms or competing same-key block observed; strict mode fences local node and raises operator-visible error.
10. **`recovering`**: reconcile tip via sync/catch-up, clear conflict only after one active lease remains.

Transition rules:

- `booting -> discovering`: after genesis/network/profile validation.
- `discovering -> active_sealing`: no valid active lease exists and atomic acquire succeeds.
- `discovering -> standby_syncing`: valid active lease exists for same validator.
- `standby_syncing -> suspect_active_lost`: heartbeat/lease expiry exceeds `takeover_timeout`.
- `suspect_active_lost -> standby_syncing`: active renews lease or peer status catches up.
- `suspect_active_lost -> takeover_candidate`: lease remains expired and standby tip is at least lease `last_tip_height`.
- `takeover_candidate -> active_after_takeover`: compare-and-swap lease succeeds with higher term.
- `active_sealing -> fenced_standby`: lease renewal fails, higher term observed, or duplicate active strict conflict.
- `active_sealing/active_after_takeover -> split_brain_detected`: competing same-key active/block evidence appears.
- `split_brain_detected -> recovering`: local seal suppressed, operator/status records conflict, sync path chooses canonical tip according to existing fork-choice rules or refuses if ambiguous.

Split-brain prevention:

- No seal without lease in `single_sealer`.
- Takeover requires expired lease plus atomic higher term.
- Old active must self-fence on renewal failure or higher term.
- Standby must catch up before sealing; otherwise it can extend stale history after takeover.
- Strict mode treats ambiguous same-key active evidence as fail-closed.

Recovery:

- If old active returns after timeout, it must start as `fenced_standby`, sync to current active, and only later acquire lease after the current active expires.
- If both nodes sealed, operator must inspect competing block evidence; MVP should not silently merge same-key divergent histories. Runtime can expose rollback/catch-up guidance, but automatic conflict resolution belongs to S3.

## Protocol and Runtime Signals

Required for S1/S2:

- Handshake/status:
  - `deployment_profile`;
  - `seal_role`;
  - `validator_identity_hash`;
  - `node_instance_id`;
  - optional `lease_term` / `lease_owner`.
- Metrics:
  - `same_validator_peer_seen_total`;
  - `same_validator_active_conflict_total`;
  - `seal_suppressed_by_fence_total`;
  - `seal_lease_acquire_total{result}`;
  - `seal_takeover_total{result}`;
  - `split_brain_detected_total`.
- Logs:
  - startup profile summary;
  - lease acquire/renew/loss;
  - duplicate active conflict with local/remote instance ids;
  - takeover decision with timeout and last tip.
- Status/RPC:
  - current profile and role;
  - local validator identity hash;
  - lease owner/term/expiry;
  - last duplicate/conflict event;
  - whether local seal-loop is currently allowed.

## Implementation Slices

### S0: docs/profile freeze

Deliverables:

- This design report.
- Ticket fields `design_report`, `recommended_slices`, `mvp_default_profile`.
- Operator wording: default is `single_sealer`; multi-sealer is experimental only.

Acceptance:

- No product code changes.
- Backlog clearly separates guardrails from future consensus.

### S1: duplicate detection and guard policy

Scope for `pwm-coding`:

- Add deployment profile enum/config with default `single_sealer`.
- Add node instance id if missing.
- Publish profile/role/validator identity hash in handshake/status.
- Detect same-key peers and enforce policy matrix.
- Add startup/status logs and metrics.

Acceptance:

- Same-key active/active under default profile produces operator-visible strict action.
- Same-key active/standby is accepted.
- Existing single-node default remains unchanged except for profile log.

### S2: lease/failover for active/standby

Scope for `pwm-coding`:

- Add lightweight lease backend, initially file/atomic record or explicitly selected coordinator.
- Gate seal-loop on lease ownership.
- Implement state machine through `standby_syncing`, `active_sealing`, `suspect_active_lost`, `takeover_candidate`, `fenced_standby`.
- Add takeover timeout and renewal interval config.

Acceptance:

- Two same-key nodes start with one active and one standby.
- Killing active causes standby takeover after bounded timeout.
- Restarting old active does not create parallel sealing.
- A stale standby cannot seal before catching up to the last lease tip.

### S3: optional cluster consensus / multi-sealer

Scope:

- RFC-level decision on whether multi-sealer means real consensus, leader election, quorum leases, or only chaos profile.
- If real consensus: block-level term/epoch evidence, deterministic fork-choice, finalized height semantics and tests.
- If chaos-only: keep it out of default testnet acceptance.

Acceptance:

- No hidden migration from single-proposer MVP to multi-proposer semantics without RFC/protocol version decision.

## Mid-second Align Boundary

Mid-second align is supportive only:

- helps reduce accidental timestamp edge drift in dev runs;
- may make manual two-node observations less noisy;
- must not be used as proof of deterministic history;
- must not replace lease/fencing or single-proposer guarantees;
- should stay under debug/dev wording.

Deterministic/single-proposer guarantees come from:

- exactly one active local sealer per validator identity;
- deterministic proposer selection from validator set and height;
- lease/fencing for same-key clones;
- explicit conflict handling when evidence contradicts the expected single active owner.

## Ordered Backlog

1. **S1 config/profile guard:** add `deployment_profile=single_sealer` default, `multi_sealer_experimental` explicit guard, startup/status exposure.
2. **S1 same-key detection:** publish `validator_identity_hash`, `node_instance_id`, `seal_role`; detect same-key peers in handshake/status.
3. **S1 policy enforcement:** strict default for active/active same-key conflict; warn-only only by explicit compatibility flag.
4. **S2 lease backend:** implement atomic lightweight lease with owner, term, expiry, last tip and fencing token.
5. **S2 seal-loop fence:** suppress local sealing unless lease is held; self-fence on renewal failure or higher term.
6. **S2 failover state machine:** active/standby takeover timeout, catch-up-before-seal, old-active recovery as standby.
7. **S2 tests:** two-node same-key harness: normal active/standby, active kill/takeover, old active return, stale standby blocked.
8. **S3 RFC decision:** decide whether multi-sealer experimental graduates to real cluster consensus or remains chaos-only.

## Participation / token estimate

```json
{
  "agent": "pwm-debug",
  "result": "PASS",
  "verbosity_focus": "seal",
  "instrumentation": {
    "files": [],
    "reverted": "yes",
    "receiver_if_kept": null
  },
  "repro": {
    "deterministic": null,
    "flake_rate": null,
    "command": null
  },
  "artifacts": [
    "docs/reviews/20260509-single-sealer-failover-design.md"
  ],
  "commands": [
    "CQDS cq_help catalog: PASS",
    "CQDS start_grep context search: PASS after retry with project_registered"
  ],
  "cleanup": {
    "cleaned": "yes",
    "what": "No processes or instrumentation started"
  },
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 9500,
    "confidence": "medium"
  }
}
```

---

_End of design gate._
