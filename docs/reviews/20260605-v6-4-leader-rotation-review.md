# Review: V6-4 Leader rotation (re-review)

**Branch:** `v6/20260603-v6-sprint4-leader-rotation-coding`  
**Commits reviewed:** `2f9aa94`, `a2622c0`, `4d68dcb`, `fff30f3` (vs `11f2713`)  
**Normative:** `docs/rfc/addenda/v6-rfc16-multi-proposer-rotation.md`  
**Prior verdict:** REQUEST_CHANGES (`docs/reviews/20260605-v6-4-leader-rotation-review.md` @ `17e07fc`)  
**Verdict:** **approve with nits**

## 1. Scope recap

Слайс V6-4 по `docs/plans/mvp_v6.md` (Sprint V6-4): детерминированная ротация proposer над **active set** (V6-3), интеграция в `Chain::seal`, pwmd snapshot/sync validation и cluster propose path. Acceptance плана: harness «primary miss → failover seal at `height+1` (≤ 1 skipped block)».

Доставлено в ветке:

| Commit | Содержание |
|--------|------------|
| `2f9aa94` | `pick_prod_idx` / `roll_epoch_if_needed` в seal, full replay, repair, `sync_live` inbound validation; unit-тесты slot math |
| `a2622c0` | Документация Windows `dlltool` blocker в `issues-report.md` |
| `4d68dcb` | **P0 fix:** `trust_tail_prod_idx`; **P1 partial:** `mk_cluster_prop` leader gate + `cluster_prop_skips_non_lead` |
| `fff30f3` | `build_project.cmd` — Windows PATH workaround для GNU toolchain |

## 2. Requirements fit

### P0 — trust vs full-replay proposer parity — **resolved**

До `4d68dcb` trust-path проверял `prod_idx` через `height % vals.set.len()`, расходясь с runtime при stake-gated active set.

`trust_tail_prod_idx` (io.rs) реплеит цепочку `1..=tip_h` с загрузкой блоков из epoch storage, на каждой высоте вызывает `roll_epoch_if_needed` + `pick_prod_idx` над `active_validator_indices`, применяет txs и producer rewards — та же семантика, что full replay в `validate_snapshot` и repair path. `validate_snapshot_trusted` сверяет tail `prod_idx` с предвычисленным вектором.

**Вывод:** gap trust vs full-replay по proposer selection **закрыт**. Остаточный nit (не P0): replay в `trust_tail_prod_idx` не сверяет финальный `replay_state` с `snapshot.state` (full replay делает); trust-path по-прежнему опирается на `last.hdr.state_root == digest(snapshot.state)` — приемлемо для текущей trust-модели.

### P1 — `mk_cluster_prop` leader gate — **достаточно для текущего coding-slice**

`mk_cluster_prop` после `roll_epoch_if_needed` + `pick_prod_idx` отсекает propose, если `node_instance_id` не совпадает с `cluster_cfg.members[prod_idx]` (индекс = genesis validator idx, согласован с существующими cluster-тестами).

Регрессия `cluster_prop_skips_non_lead`: при `active_validator_indices = [1]` node-a не открывает round — PASS по контракту gate.

**Ограничение (nit P2):** gate не покрывает сценарий «scheduled leader молчит, failover node должен propose» — это следующий пункт.

### P1 — miss / failover runtime — **не реализован; defer для sign-off, не блокер pwm-testing**

RFC16 addendum §3 (miss detection, quorum timeout → seal at `height+1` от failover proposer) и acceptance `mvp_v6.md` требуют runtime/harness. В diff:

- Есть только формула failover slot (`failover_slot_is_next_height` в `pwm-core`) — математика `pick_prod_idx(h+1, …)`, не поведение.
- Нет miss detection по profile tick window, нет переключения cluster propose/seal на failover leader, нет `UnavailableProposer` evidence hook.
- `lifecycle.rs` quorum/timeout логика не привязана к RFC16 §3 failover schedule.

**Рекомендация оркестратору:** **отложить** miss/failover runtime в follow-up coding slice (V6-4b) **до** закрытия sprint acceptance «done»; **не блокировать** конвейер `pwm-testing` на текущем diff — тестировать реализованную поверхность (unit + cluster gate + naming linter). pwm-testing должен явно зафиксировать acceptance-gap в отчёте.

### `build_project.cmd` — **ok**

Корень репозитория — правильное место для host-specific launcher (не меняет `.cargo/config.toml`). Скрипт prepend MSYS UCRT/MingW + rust self-contained GNU bins, проверяет `cargo`/`dlltool`, делегирует в `cargo build --workspace` или `cargo %*`. Снимает blocker из `issues-report.md` (стр. 753–758).

**Nit:** дефолт `CARGO_TARGET_DIR=F:\pwm-test\shared` — host-specific; на других машинах лучше задавать через env или убрать дефолт в follow-up chore.

## 3. Style and module shape

- Shared helpers `recompute_active_idxs`, `pick_prod_idx`, `roll_epoch_if_needed` в `pwm-core::chain` — DRY с V6-3, без дублирования в pwmd validation paths.
- `trust_tail_prod_idx` — 3 сегмента (prod budget ≤4) — OK.
- `check_entity_name_segments.py` на `chain.rs`, `snapshot/io.rs`, `peer_session/mod.rs` — **violations: []**.
- Module banners (`//!`) на затронутых файлах сохранены.
- Protocol semver: `PWM_PROTOCOL_VERSION` / wire structs не затронуты.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- Trust replay загружает блоки с диска по высоте — bounded работой `tip_h` (cold-start perf nit уже в `issues-report.md`).
- Leader gate предотвращает off-schedule cluster propose от non-leader proposer role — снижает риск лишних quorum rounds.
- Нет новых `unwrap` в hot paths в diff; `mk_cluster_prop` использует `ok()?` / early return.

## 5. Tests

**Покрыто:**

- `pwm-core`: `prod_rotation_uses_height_slot`, `failover_slot_is_next_height`, `stake_below_min_excluded`, seal regressions.
- `pwmd`: `cluster_prop_mirror_send`, `cluster_prop_skips_non_lead`.

**Пробелы (для pwm-testing / follow-up):**

- Нет unit/integration теста `trust_tail_prod_idx` / trust-load с filtered active set (parity assertion).
- Нет harness «induced primary miss → valid block from failover at height+1» (RFC16 §3 acceptance).
- Windows: `cargo test` blocked без `dlltool` — `build_project.cmd` + Linux CQDS для прогона.

## 6. Verdict

**approve with nits** — P0 снят; cluster leader gate достаточен для заявленного coding-slice; miss/failover runtime остаётся открытым acceptance-item, defer до follow-up, не блокирует pwm-testing.

**Prioritized nits:**

| Pri | Item | Action |
|-----|------|--------|
| P1 | Miss/failover runtime + harness (RFC16 §3, mvp_v6 acceptance) | Follow-up coding slice before sprint sign-off |
| P2 | Trust-path test: active-set subset parity on trust load | pwm-testing or next coding slice |
| P3 | `build_project.cmd` hardcoded `CARGO_TARGET_DIR` | Optional chore |
| P3 | `trust_tail_prod_idx` full-chain replay perf on long epochs | Monitor; checkpoint seed if needed |

## 7. pwm-testing guidance

Запускать (Linux CQDS или Windows через `build_project.cmd` после PATH fix):

- `cargo test -p pwm-core`
- `cargo test -p pwmd cluster_prop_`
- `python scripts/check_entity_name_segments.py` на touched paths (уже green)

Не заявлять PASS acceptance «miss → ≤1 block» до follow-up. Зафиксировать dlltool/Windows env в test report.

## 8. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260605-v6-4-leader-rotation-review.md
token_usage:
  source: estimate
  input: 12000
  output: 3500
  total: 15500
  confidence: medium
```
