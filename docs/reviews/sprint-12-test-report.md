# Sprint 12 Test Report (Slice 5 closeout coding-pass)

Дата: 2026-04-26  
Этап: Slice 5 closeout readiness and final targeted coding-pass checks

## Slice 5 Final Targeted Coding-Pass Check Set

- `cargo fmt --all -- --check` — PASS.
- `cargo check -p pwmd` — PASS.
- `cargo test -p pwmd transport` — PASS (11 passed, 0 failed; targeted transport pack).
- `cargo test -p pwmd tests::v1_status_reports_loading_and_head_returns_503 -- --exact` — PASS (1/1).
- `cargo test -p pwmd tests::v1_status_reports_ready_degraded_after_snapshot_error -- --exact` — PASS (1/1).
- Guardrail re-check (`fixed-volume`, `no scope expansion`, `no wire/API drift`, `no Sprint 11 migration reopen`) — PASS.

## Follow-up Validation: relay-neutral default contract

- `cargo check -p pwmd` — **PASS**.
- `cargo test -p pwmd tests::default_runtime_identity_neutral_is_relay_baseline_without_alias_affinity -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::v1_status_reports_neutral_relay_baseline_without_alias_shard -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::resolve_runtime_identity_uses_alias_mapping_for_shard -- --exact` — **PASS** (1/1, compat alias guard).
- `cargo test -p pwmd tests::v1_status_reports_alias_state_namespace_for_shard -- --exact` — **PASS** (1/1, compat alias namespace guard).

## Executed Checks (Slice 4)

- `cargo fmt` — PASS.
- `cargo check -p pwmd` — PASS.
- `cargo test -p pwmd transport` — PASS (11 passed, 0 failed; transport-focused filtered run).
- Проверка guardrails в изменении Slice 4 (`no scope expansion`, `no wire/API drift`, `no migration contract changes`) — PASS.

## Verdict

- Slice 5 final targeted coding-pass check set — **PASS**.
- Regression signals по затронутым transport/lifecycle status зонам не обнаружены.
- Sprint 12 coding-pass closeout готов к финальному независимому testing/review verdict.

## Planned Testing Policy for Execution Slices

- На каждом coding slice: минимум `cargo check -p pwmd` + targeted checks по затронутой optimization зоне.
- Без раздутия матрицы тестов на уровне coding-pass; полный независимый прогон остается за `pwm-testing`.
- Перед closeout: consolidated targeted+sanity verification против sprint-12 gates.

## Notes

- Sprint 11 migration baseline принят как входное состояние; Slice 1 выполнен без переоткрытия migration scope.

## Slice 4 Optimization Evidence

- В `crates/pwmd/src/lifecycle.rs` введен helper `runtime_mode_summary(...)` для единого формирования startup mode-строки.
- Удалено дублирование `match` блока в двух startup логах (`info!` и `eprintln!`) при сохранении того же output contract.
- Изменение behavior-preserving и не затрагивает публичные API/wire contracts.

## Independent Testing Pass (Slice 1)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent pass after coding-pass)

### Scope Under Test

- Зона регрессии: `pwmd` transport metrics/snapshot behavior вокруг `record_transport_attempt` / snapshot updates.
- Подтверждение behavior-preserving после micro-optimization (без изменения продуктовой логики).

### Commands and Results

- `cargo fmt --all -- --check` — **PASS**.
- `cargo test -p pwmd tests::v1_dev_peers_exposes_transport_snapshot -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_tick_connects_seed_and_accepts_handshake -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_tick_rejects_bad_signature_and_tracks_reason -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_runaway_guard_stops_then_resumes_attempts -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd transport` — **PASS** (11 passed, 0 failed; filtered transport smoke pack).

### Independent Verdict

- Targeted regression checks по transport metrics/snapshot — **PASS**.
- Regression signals в затронутой Slice 1 зоне не обнаружены.
- Product-code changes в independent testing-pass: **none**.

### Residual Risks

- Не выполнялся full-workspace/extended matrix прогон; confidence основан на targeted `pwmd` transport test-set.
- Не добавлялись новые тесты для внутренней структуры snapshot maps (`last_attempt_ms_by_class` / `last_result_by_class`) вне текущих endpoint/integration assertions.

## Independent Testing Pass (Slice 2)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent pass after coding-pass)

### Scope Under Test

- Зона регрессии: `crates/pwmd/src/transport.rs` (Slice 2 micro-optimization в helper class key без `String` аллокации).
- Цель: подтвердить behavior-preserving эффект без drift в transport metrics/snapshot semantics.

### Commands and Results

- `cargo fmt --all -- --check` — **PASS**.
- `cargo test -p pwmd tests::v1_dev_peers_exposes_transport_snapshot -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_tick_connects_seed_and_accepts_handshake -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_tick_rejects_bad_signature_and_tracks_reason -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_tick_uses_deterministic_seed_rotation_with_budget -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_runaway_guard_stops_then_resumes_attempts -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd transport` — **PASS** (11 passed, 0 failed; transport-targeted filtered pack).
- `cargo test -p pwmd tests::v1_status_reports_alias_state_namespace_for_shard -- --exact` — **PASS** (1/1, sanity вне transport).

### Independent Verdict

- Transport-targeted regression checks для Slice 2 — **PASS**.
- Признаков регресса в затронутой зоне (`transport` hot-path + snapshot counters) не обнаружено.
- Product-code changes в independent testing-pass: **none**.

### Residual Risks (Slice 2)

- Не выполнялся full-workspace прогон; confidence ограничен targeted `pwmd` набором + точечный sanity.
- Не добавлялись новые тест-кейсы на allocation/perf-метрики; проверялось только отсутствие функционального регресса.

## Independent Testing Pass (Slice 3)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent pass after coding-pass)

### Scope Under Test

- Зона регрессии: `crates/pwmd/src/transport.rs` (Slice 3: branch-first in-place update в snapshot maps).
- Цель: подтвердить behavior-preserving изменение без drift в transport metrics/snapshot semantics.

### Commands and Results

- `cargo fmt --all -- --check` — **PASS**.
- `cargo test -p pwmd tests::v1_dev_peers_exposes_transport_snapshot -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_tick_connects_seed_and_accepts_handshake -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_tick_rejects_bad_signature_and_tracks_reason -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_tick_uses_deterministic_seed_rotation_with_budget -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::real_transport_runaway_guard_stops_then_resumes_attempts -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd transport` — **PASS** (11 passed, 0 failed; transport-targeted filtered pack).
- `cargo test -p pwmd tests::v1_status_reports_alias_state_namespace_for_shard -- --exact` — **PASS** (1/1, sanity вне transport).

### Independent Verdict

- Targeted regression checks для Slice 3 — **PASS**.
- Признаков функционального регресса в transport metrics/snapshot зоне не обнаружено.
- Product-code changes в independent testing-pass: **none**.

### Residual Risks (Slice 3)

- Не выполнялся full-workspace/extended matrix прогон; confidence ограничен targeted `pwmd` набором + один sanity тест.
- Проверялась функциональная эквивалентность; performance/allocation эффект micro-optimization не бенчмаркался в этом проходе.

## Independent Testing Pass (Slice 4)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent pass after coding-pass)

### Scope Under Test

- Зона регрессии: `crates/pwmd/src/lifecycle.rs` (Slice 4 micro-optimization: dedup startup mode summary formatting).
- Цель: подтвердить behavior-preserving изменение в startup/runtime lifecycle path без functional drift.

### Commands and Results

- `cargo fmt --all -- --check` — **PASS**.
- `cargo test -p pwmd tests::v1_status_reports_loading_and_head_returns_503 -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::v1_status_reports_ready_degraded_after_snapshot_error -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::v1_status_reports_alias_state_namespace_for_shard -- --exact` — **PASS** (1/1, sanity вне lifecycle-ошибок snapshot).
- `cargo test -p pwmd transport` — **PASS** (11 passed, 0 failed; небольшой runtime sanity around transport loop interactions).

### Independent Verdict

- Targeted regression checks для Slice 4 startup/runtime lifecycle зоны — **PASS**.
- Признаков функционального регресса после dedup formatting helper не обнаружено.
- Product-code changes в independent testing-pass: **none**.

### Residual Risks (Slice 4)

- Не выполнялся полный `cargo test --workspace`; confidence основан на targeted `pwmd` lifecycle/status тестах + transport sanity.
- Проверялась только функциональная эквивалентность output behavior; micro-perf эффект не бенчмаркался в этом проходе.

## Independent Evidence: neutral relay-default follow-up

Дата: 2026-04-26  
Роль: `pwm-testing` (independent verification after neutral relay-default fix)

### Scope

- Проверить runtime behavior `pwmd` без identity/shard флагов.
- Проверить deprecated compat path `--shard A|B`.
- Проверить базовые sanity (`/v1/status`, `/v1/head`, namespace semantics) на отсутствие регрессий.

### Commands and Results

- `cargo run -p pwmd -- --listen 127.0.0.1:38080 --state-root tmp/s12-neutral` — **FAIL** относительно ожидаемого neutral-default runtime contract: стартовал с `state_ns=shard-a`, `shard=A`, `mode=relay_baseline(alias:A)` (нет neutral binding по умолчанию).
- `cargo run -p pwmd -- --shard A --listen 127.0.0.1:38081 --state-root tmp/s12-compat-a` — **PASS**: deprecated warning печатается, node поднимается, alias path работает (`state_ns=shard-a`, `mode=relay_baseline(alias:A)`).
- `cargo run -p pwmd -- --shard B --listen 127.0.0.1:38082 --state-root tmp/s12-compat-b` — **PASS**: deprecated warning печатается, node поднимается, alias path работает (`state_ns=shard-b`, `mode=relay_baseline(alias:B)`).
- `cargo test -p pwmd v1_status_reports_` — **PASS** (5 passed, 0 failed), включая:
  - `tests::v1_status_reports_neutral_relay_baseline_without_alias_shard` (neutral status semantics),
  - `tests::v1_status_reports_loading_and_head_returns_503`,
  - `tests::v1_status_reports_ready_degraded_after_snapshot_error`,
  - `tests::v1_status_reports_alias_state_namespace_for_shard`,
  - `tests::v1_status_reports_explicit_domain_state_namespace` (включает `/v1/head` = `200 OK` check).

### Independent Verdict

- Проверка (1) "CLI запуск без identity/shard параметров нейтрален" — **FAIL** по runtime evidence.
- Проверка (2) deprecated compat `--shard A|B` — **PASS**.
- Проверка (3) базовые status/head + namespace semantics — **PASS** на targeted test-pack.
- Product-code changes в этом independent pass: **none**.

### Residual Risks

- Есть расхождение между runtime CLI default path и in-memory neutral test contract (нейтральная семантика подтверждается тестом, но не runtime запуском `pwmd` без `--shard`).
- Прогон ограничен targeted `pwmd` checks; полный `cargo test --workspace` не выполнялся.

## Independent Retest Verdict: default pwmd incident (post-fix)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent retest after additional coding fix)

### Retest Scope

- Перепроверить спорный кейс default запуска `pwmd` без `--shard`/identity flags.
- Подтвердить, что compat path `--shard A|B` сохранен после фикса.
- Зафиксировать root-cause resolution статус на runtime evidence.

### Commands and Results

- `cargo run -p pwmd -- --listen 127.0.0.1:39080 --state-root tmp/s12-retest-neutral` — **PASS**: стартовал в neutral relay-default (`shard=neutral`, `state_ns=neutral`, `mode=relay_baseline(neutral-default)`), без alias affinity к shard A.
- `cargo run -p pwmd -- --shard A --listen 127.0.0.1:39081 --state-root tmp/s12-retest-a` — **PASS**: deprecated warning присутствует, compat alias path работает (`shard=A`, `state_ns=shard-a`, `mode=relay_baseline(alias:A)`).
- `cargo run -p pwmd -- --shard B --listen 127.0.0.1:39082 --state-root tmp/s12-retest-b` — **PASS**: deprecated warning присутствует, compat alias path работает (`shard=B`, `state_ns=shard-b`, `mode=relay_baseline(alias:B)`).
- `cargo test -p pwmd tests::default_runtime_identity_neutral_is_relay_baseline_without_alias_affinity -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::resolve_runtime_identity_uses_alias_mapping_for_shard -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::v1_status_reports_alias_state_namespace_for_shard -- --exact` — **PASS** (1/1).

### Retest Verdict

- Проверка (1) default запуск без `--shard`/identity flags как neutral relay-default — **PASS**.
- Проверка (2) compat `--shard A|B` — **PASS**.
- Root-cause resolution статус по инциденту default startup binding — **RESOLVED** (runtime evidence подтверждает исправление).
- Product-code changes в этом retest pass: **none**.

## Docs Addendum Sanity (post-closeout, docs-only)

- Проверен docs-only follow-up: добавлен `docs/DOMAINS.md` и ссылки на него из `README.md` / `docs/pwmd.md` в контексте запуска с `domain_hi`.
- Sanity result: PASS (структура документа целевая, ссылки валидны, терминология согласована с контрактом `domain-first + neutral default + alias compat`).
- Product-code changes в docs addendum: **none**.

## Independent Testing Pass: domain range edges + address bruteforce

Дата: 2026-04-26  
Роль: `pwm-testing` (targeted edge/bruteforce verification)

### Scope

- Проверка краев/поведения по domain-классам `country`, `sector`, `reserve`, `witness` (и boundary-adjacent policy behavior).
- Подтверждение практической работы индекса через `addr-bruteforce` для первой и последней country-меток в текущем indexed списке (`AD`, `ZW`).

### Commands and Results (edge/boundary tests)

- `cargo test -p pwm-core domain_index::tests:: -- --nocapture` — **PASS** (6/6), включая:
  - `country_list_has_195_entries_sorted`,
  - `sample_sector_lookup_works`,
  - `reserve_range_has_category_without_direct_label`,
  - `regulatory_lookup_by_hi_ignores_low_byte_noise`.
- `cargo test -p pwm-core tx::tests::validate_tx_shape_accepts_regulatory_init_lo_zero -- --exact` — **PASS** (country boundary behavior, `domain_lo=0`).
- `cargo test -p pwm-core tx::tests::validate_tx_shape_accepts_regulatory_init_lo_non_zero -- --exact` — **PASS** (country boundary behavior, `domain_lo!=0`).
- `cargo test -p pwm-core address_book::tests::validate_recipient_domain_policy_cli_vs_neutral_wording -- --exact` — **PASS** (reserve policy boundary wording).
- `cargo test -p pwmd tests::v1_tx_rejects_reserve_recipient_prefilter -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::v1_tx_rejects_witness_recipient_prefilter -- --exact` — **PASS** (1/1).
- `cargo test -p pwmd tests::v1_tx_rejects_unknown_recipient_prefilter -- --exact` — **PASS** (1/1; boundary-adjacent unknown-domain guard).
- `cargo test -p pwm-cli tests::label_only_accepts_sector_label -- --exact` — **PASS** (1/1).
- `cargo test -p pwm-cli tests::sector_label_is_not_regulatory -- --exact` — **PASS** (1/1).

### Commands and Results (address bruteforce)

- `cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master <hex> --domain AD --flags-mask 1023 --expected-flags 0 --max-try 800000 --wallet-out tmp/s12-edge-bruteforce-ad.yaml` — **PASS**.
  - `domain_label=AD`, `derivation_index=690542`, `benchmark_attempts=690543`, `benchmark_elapsed_ms=84760.198`, `benchmark_attempts_per_sec=8147.020`.
- `cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master <hex> --domain ZW --flags-mask 1023 --expected-flags 0 --max-try 800000 --wallet-out tmp/s12-edge-bruteforce-zw.yaml` — **PASS**.
  - `domain_label=ZW`, `derivation_index=156050`, `benchmark_attempts=156051`, `benchmark_elapsed_ms=18747.298`, `benchmark_attempts_per_sec=8323.919`.

### Anomalies / Mismatches (exact)

- **Tooling mismatch (PowerShell chaining)**  
  - Command: `cargo test ... && cargo test ...`  
  - Expected: sequential run in one line.  
  - Actual: PowerShell parser error (`&&` not supported in this shell session).  
  - Resolution: switched to separate commands / `;` chaining.

- **CLI contract mismatch #1 (`addr-bruteforce`)**  
  - Command: `... addr-bruteforce --domain AD --max-try 800000 --wallet-out ...`  
  - Expected: run with defaults for flags.  
  - Actual: FAIL, required args missing (`--flags-mask`, `--expected-flags`).

- **CLI contract mismatch #2 (flag format)**  
  - Command: `... --flags-mask 0x03FF --expected-flags 0 ...`  
  - Expected: hex accepted.  
  - Actual: FAIL, parse error `invalid digit found in string` (decimal required).

- **Runtime precondition mismatch (wallet encryption default)**  
  - Command: `... addr-bruteforce ... --wallet-out ...` without passphrase env/flag  
  - Expected: wallet file writes directly for evidence run.  
  - Actual: FAIL, `encrypted wallet mode requires passphrase`.  
  - Resolution: set `PWM_WALLET_PASSPHRASE` for successful evidence run.

### Evidence Artifacts

- `tmp/s12-edge-bruteforce-ad.yaml` (generated).
- `tmp/s12-edge-bruteforce-zw.yaml` (generated).

### Evidence Addendum (implementation pass, 2026-04-26)

- `cargo test -p pwm-cli tests::addr_bruteforce_cli_defaults_flags_mask_to_1023 -- --exact` — **PASS**.
- `cargo test -p pwm-cli tests::addr_bruteforce_cli_accepts_wallet_passphrase_flag_without_env -- --exact` — **PASS**.
- `cargo test -p pwm-cli tests::addr_bruteforce_preflight_rejects_empty_passphrase -- --exact` — **PASS**.
- `cargo test -p pwm-cli tests::addr_bruteforce_plaintext_fallback_when_passphrase_missing -- --exact` — **PASS**.
- `cargo test -p pwm-cli tests::addr_bruteforce_uses_cli_passphrase_for_encrypted_mode -- --exact` — **PASS**.
- `cargo check -p pwm-cli` — **PASS**.
- Contract deltas confirmed:
  - `--flags-mask` now optional with default `1023`;
  - `addr-bruteforce` preflights wallet protection before brute-force starts;
  - missing passphrase now falls back to `plaintext_dev` with explicit warning (encrypted path unchanged when passphrase is provided).

### Independent Verification (runtime/CLI, 2026-04-26)

- Scope: independent re-check of claimed fixes in `pwm-cli addr-bruteforce` without product-code edits.
- Test vector:
  - `master=0000000000000000000000000000000000000000000000000000000000000001`
  - `domain=AD`
  - `expected_flags=0`

#### Commands and PASS/FAIL

- **Scenario 1: preflight rejects empty passphrase before long brute-force**
  - Command:
    - `cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master 0000000000000000000000000000000000000000000000000000000000000001 --domain AD --expected-flags 0 --max-try 800000 --wallet-passphrase "" --wallet-out tmp/s12-iv-empty-passphrase.yaml`
  - Result: **PASS**.
  - Evidence: immediate user error `wallet passphrase must not be empty`, exit code `2`, runtime ~`3.7s` (no long brute-force phase).

- **Scenario 2: plaintext fallback + warning when passphrase is absent**
  - Command:
    - `cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master 0000000000000000000000000000000000000000000000000000000000000001 --domain AD --expected-flags 0 --max-try 1500000 --wallet-out tmp/s12-iv-fallback-defaultmask-success.yaml`
  - Result: **PASS**.
  - Evidence:
    - warning emitted before progress: `wallet will be saved in plaintext-dev mode`;
    - run completed successfully (exit code `0`);
    - output prints `flags_mask_u32 1023`;
    - wallet file saved with `mode: plaintext_dev`.

- **Scenario 3: passphrase wiring (`--wallet-passphrase` overrides empty env)**
  - Command:
    - `cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master 0000000000000000000000000000000000000000000000000000000000000001 --domain AD --expected-flags 0 --max-try 1500000 --wallet-passphrase cli-pass-123 --wallet-out tmp/s12-iv-cli-over-env-success.yaml` (with `PWM_WALLET_PASSPHRASE=""` in env)
  - Result: **PASS**.
  - Evidence:
    - no `empty passphrase` error;
    - run completed (exit code `0`);
    - resulting wallet has `mode: encrypted` and encrypted payload fields (`encrypted_payload_b64`, `kdf_salt_b64`, `aead_nonce_b64`).

- **Scenario 4: default `--flags-mask` = 1023 in practice**
  - Commands:
    - implicit default: `cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master 0000000000000000000000000000000000000000000000000000000000000001 --domain AD --expected-flags 0 --max-try 1500000 --wallet-passphrase cli-pass-123 --wallet-out tmp/s12-iv-cli-over-env-success.yaml`
    - explicit mask: `cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master 0000000000000000000000000000000000000000000000000000000000000001 --domain AD --flags-mask 1023 --expected-flags 0 --max-try 1500000 --wallet-passphrase cli-pass-123 --wallet-out tmp/s12-iv-explicit-mask-1023.yaml`
  - Result: **PASS**.
  - Evidence:
    - implicit run accepted without `--flags-mask` and printed `flags_mask_u32 1023`;
    - implicit and explicit runs converged to identical match (`derivation_index 127911`, same account id), confirming effective default mask behavior.

#### Final Verdict

- **RESOLVED**.
- Claimed fixes are reproducible in runtime/CLI checks: passphrase precedence, early preflight on empty passphrase, plaintext fallback with warning when passphrase is absent, and practical default `--flags-mask=1023`.

## Follow-up sanity: interactive launcher for addr-bruteforce

- Scope: smoke-check новых интерактивных оберток над `pwm-cli addr-bruteforce` (без изменений product-логики).
- Commands:
  - `cmd /c "(echo 0000000000000000000000000000000000000000000000000000000000000001& echo AD& echo tmp/s12-interactive-wallet.yaml& echo 1023& echo 0& echo 500000& echo.) | scripts\\addr-bruteforce-interactive.cmd --dry-run"`
- Result: **PASS**.
- Evidence:
  - `.cmd` launcher корректно передал `--dry-run` в `.sh`, принял интерактивные значения из stdin и вывел собранную команду `cargo run -p pwm-cli --bin pwm -- ... addr-bruteforce ...`;
  - реальный brute-force не запускался (`Dry run: command is prepared and not executed`).
