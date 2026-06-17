# V6-4b: RFC16 §3 primary-miss failover — review (`pwm-review`)

**Ticket:** `tasks/20260606-v6-sprint4b-leader-failover-coding.json`  
**Branch reviewed:** `main` (local review, no bridge)  
**Commits:** `6d802b0` (feature), `b5acbc9` (build wrappers — spot-check)  
**Depends on:** V6-4 merge `fad86d8`  
**Normative:** `docs/rfc/addenda/v6-rfc16-multi-proposer-rotation.md` §3–§4  

---

## 1. Scope recap

Срез закрывает отложенный в V6-4 runtime **primary-miss → bounded skip → failover seal на `height+1`** в `pwmd` seal loop, плюс lib-harness `miss_skip_failover_seals`. План: `docs/plans/mvp_v6.md` (V6-4b), acceptance V6-4/V6-4b — «missed block → next leader ≤ 1 block» на CY profile **или** lib harness.

**Feature commit (`6d802b0`):** `crates/pwmd/src/lifecycle.rs` (+147/−7), запись в `issues-report.md`.  
**Chore commit (`b5acbc9`):** `.build.env`, `scripts/build_project.sh`, `scripts/test_project.sh`, упрощённые `build_project.cmd` / `test_project.cmd`, `target-codex/` в `.gitignore`.

---

## 2. Requirements fit

| Criterion (ticket / RFC §3) | Status | Notes |
|---|---|---|
| Miss detection (profile tick or quorum timeout) | **Partial** | Реализован только **profile tick**: non-primary cluster proposer ждёт `nominal_ms` (`seal_interval_ms(bph)`) и вызывает `skip_missed_h`. Второй триггер RFC §3 — **quorum timeout без seal primary** — в miss-path **не подключён** (attest timeout влияет на cluster gate позже, не на skip). |
| Failover seal at `height+1` | **Met** | `skip_missed_h` поднимает `canonical_h` на пропущенный `lead_h`; следующий `Chain::seal` даёт блок на `tip_h()+1` с proposer по `pick_prod_idx(h+1, active)`. Согласовано с формулой RFC §2 и тестом `failover_slot_is_next_height` (V6-4). |
| Harness: induced miss → ≤1 skipped block | **Met (lib)** | `miss_skip_failover_seals`: primary на h=1 — не local; skip h=1; failover seal h=2, `prod_idx=0`, подпись валидна. **Не** покрывает wall-clock путь `spawn_seal_loop`. |
| No competing multi-proposer rounds | **Met** | Non-primary: `continue` до cluster propose/seal. `mk_cluster_prop` (V6-4) уже отсекает non-leader по тому же `pick_prod_idx` → `cluster_cfg.members` mapping. |
| Optional `UnavailableProposer` evidence | **Absent** | RFC §5 / ADR 0010 — MAY; в diff нет append `EvidenceRecord`. Допустимо для среза. |
| `cargo fmt` / check / test | **Not verified here** | Review-only; `pwm-testing` должен прогнать matrix. |

**Mapping proposer ↔ cluster member:** `local_prod_for_h` повторяет контракт V6-4 (`peer_session::mk_cluster_prop`): `pick_prod_idx` → индекс в `cluster_cfg.members`. С `roll_epoch_if_needed` — согласовано с core.

**Известный продуктовый gap (зафиксирован coding):** skip через `Chain::set_canon_h` без тела блока создаёт **разрыв высот** относительно contiguous sync/snapshot trust — см. `issues-report.md` (2026-06-07). Для bounded cluster failover в harness — осознанный workaround; для live catch-up / snapshot trust — **follow-up medium+**, не блокер слайса при текущем scope.

---

## 3. Style and module shape

- **`check_entity_name_segments.py`** на `crates/pwmd/src/lifecycle.rs`: **violations: []** (prod ≤4, test ≤5).
- Новые символы: `local_prod_for_h`, `skip_missed_h` — 3–4 сегмента, в политике.
- Harness helper `mk_val` — локальный test-only, ≤2 сегмента.
- `lifecycle.rs` уже имеет `//!` banner; diff не ломает структуру.
- Miss-watch state (`miss_watch_h`, `miss_watch_at_ms`) — локальные переменные loop, без раздувания façade.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

---

## 4. Safety

- **`skip_missed_h` bounded:** skip только если `tip_h().saturating_add(1) == h` — не произвольный jump.
- **Scope:** miss-path только при `cluster_cfg.enabled` + `ClusterRole::Proposer`; attester по-прежнему не seal-ит.
- **Ordering:** miss skip выполняется **до** `run_lease_gate` — standby без lease может поднять `canonical_h` на skip, seal позже заблокируется lease. Операционный caveat для S2 multi-process; для single active proposer — приемлемо.
- **Race slow primary:** failover skip по локальному таймеру без cross-node подтверждения, что primary не seal-ит h. При двух активных proposer-процессах теоретический fork на h vs skip→h+1; mitigated lease/S2 в prod, не покрыто слайсом.
- **Panics / unwrap:** новый код — async guards, без новых hot-path `unwrap`.
- **Height gap:** sync/snapshot несовместимость документирована; severity **medium** для prod enablement, **low** для закрытия V6-4b harness scope.

---

## 5. Tests

**Present:**
- `miss_skip_failover_seals` — core contract skip + failover seal + sig verify.
- Опирается на V6-4 `pick_prod_idx` / active set (2 validators, members order).

**Gaps (for `pwm-testing`):**
1. **`test_project.sh` default matrix** (`pwm-core --lib`, `pwmd cluster_prop_`) **не включает** `miss_skip_failover_seals`. Нужен явный прогон: `cargo test -p pwmd miss_skip_failover_seals` (или расширить matrix).
2. Нет integration: induced miss через `spawn_seal_loop` + wall-clock.
3. Нет negative: double-skip, skip при `tip+1 != h`, attest-timeout-as-miss.
4. Quorum-timeout miss trigger — без теста (код отсутствует).

**Chore commit spot-check (`b5acbc9`):** thin `.cmd` → `scripts/*.sh`, shared `.build.env` (UCRT64 PATH, `dlltool` RUSTFLAGS, isolated `target-codex/`). Разумно; не влияет на протокол.

---

## 6. Verdict

**Approve with nits** — runtime failover и lib harness соответствуют RFC16 §3 acceptance для bounded cluster path; sync/snapshot gap и quorum-timeout trigger явно вынесены; evidence stub optional и отсутствует по AC.

### Prioritized nits

1. **Medium (testing):** добавить `miss_skip_failover_seals` в обязательный прогон `pwm-testing` / расширить `scripts/test_project.sh` default matrix.
2. **Medium (spec parity):** RFC §3 второй miss trigger (quorum timeout) — не реализован; зафиксировать defer или follow-up тикет.
3. **Medium (prod):** height gap vs sync/snapshot — follow-up из `issues-report.md` до prod catch-up enablement.
4. **Low:** harness не покрывает seal_loop timer path; integration defer OK если testing явно прогоняет unit harness.
5. **Low:** `UnavailableProposer` evidence — optional, отсутствует (OK).
6. **Low:** miss skip до lease gate — документировать для S2 standby ops.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260607-v6-4b-leader-failover-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 28000
  confidence: low
```

**Verdict (one line):** `PASS WITH NITS — RFC16 §3 failover + harness OK; test matrix must run miss_skip; quorum-timeout miss + sync gap are documented follow-ups.`
