# MVP checklist (PWM)

Подробный чеклист по плану MVP. Легенда: `[x]` сделано, `[ ]` не сделано, `[~]` частично / заглушка. Отдельные строки `[ ]` со ссылкой на **§12** — намеренный defer/backlog, не «пропуск».

Обновляйте этот файл **по мере реализации** (дата в скобках по желанию).

Общий пользовательский итог нулевой фазы: [MVP_PHASE0_SUMMARY.md](MVP_PHASE0_SUMMARY.md).

Экономический и консенсусный трек **MVP v2** ведётся отдельно в [plans/mvp_v2.md](plans/mvp_v2.md); этот файл остаётся базовым чеклистом v0/v1 testnet и cross-shard MVP.

Foundation-трек **MVP v3** ведётся отдельно в [plans/mvp_v3.md](plans/mvp_v3.md); ниже добавлен короткий traceability-блок, чтобы общий чеклист ссылался на закрытый public-devnet gate.

Policy runtime-трек **MVP v4** ведётся отдельно в [plans/mvp_v4.md](plans/mvp_v4.md); ниже добавлен короткий traceability-блок по закрытому V4 integrated gate.

Tokenomics-трек **MVP v5** ведётся отдельно в [plans/mvp_v5.md](plans/mvp_v5.md); ниже добавлен traceability-блок по спринтам V5 (обновляется по мере закрытия gate).

---

## 0v3. MVP v3 foundation closeout

| Статус | Пункт |
|--------|--------|
| [x] | `/v1/*` API freeze skeleton и ADR package: [api-v1.md](api-v1.md), [adr/README.md](adr/README.md), ticket [20260516-v3-sprint1-spec-adr-api.json](../tasks/20260516-v3-sprint1-spec-adr-api.json) |
| [x] | Epoch Snapshot manifest `schema_v` compatibility contract и replay determinism gate: [guide-node-storage-and-snapshot.md](guide-node-storage-and-snapshot.md), ticket [20260516-v3-sprint2-snapshot-replay.json](../tasks/20260516-v3-sprint2-snapshot-replay.json) |
| [x] | Demo genesis package с premine 21B PWM и public devnet quickstart: [runbooks/demo-devnet-quickstart.md](runbooks/demo-devnet-quickstart.md), ticket [20260516-v3-sprint3-demo-genesis-devnet.json](../tasks/20260516-v3-sprint3-demo-genesis-devnet.json) |
| [x] | Integrated V3 public-devnet smoke: clean genesis build/verify, CY 3-node, `/v1/status`, `/v1/head`, `/v1/accounts`, `/v1/account/:id`; финальный review [sprint-v3-4-public-devnet-closeout-review-20260516.md](reviews/sprint-v3-4-public-devnet-closeout-review-20260516.md), ticket [20260516-v3-sprint4-public-devnet-closeout.json](../tasks/20260516-v3-sprint4-public-devnet-closeout.json) |
| [~] | `POST /v1/tx` остаётся в API freeze skeleton, но не входил в V3-4 integrated smoke; покрыть отдельной smoke-строкой при расширении external integration сценариев. |

---

## 0v4. MVP v4 policy runtime closeout

| Статус | Пункт |
|--------|--------|
| [x] | RFC/wire freeze: dedicated `PolicyTx` with embedded `PolicyAction`, lifecycle `Dormant/Immediately`, hybrid corporate `INIT`, rescue address, emergency finalization semantics and additive `E_POLICY_*` rejects; ticket [20260517-v4-sprint1-policy-rfc-freeze.json](../tasks/20260517-v4-sprint1-policy-rfc-freeze.json), review [20260517-v4-sprint1-policy-rfc-freeze-review.md](reviews/20260517-v4-sprint1-policy-rfc-freeze-review.md). |
| [x] | Core model: `PolicyTx`, `init_v4`, account policy/rescue/finalized fields, snapshot conversion, cosign wire preservation and deterministic signing/serde; ticket [20260517-v4-sprint2-core-model.json](../tasks/20260517-v4-sprint2-core-model.json), review [20260517-v4-sprint2-core-model-review.md](reviews/20260517-v4-sprint2-core-model-review.md). |
| [x] | Pure evaluator: `evaluate_policy`, structured policy rejects, same-domain routing deny, conservative sender/default deny, generic cosign gate and finalized reject scaffold; ticket [20260517-v4-sprint3-policy-engine.json](../tasks/20260517-v4-sprint3-policy-engine.json), review [20260517-v4-sprint3-policy-engine-review.md](reviews/20260517-v4-sprint3-policy-engine-review.md). |
| [x] | Emergency routing: rescue-address cosign activation, irreversible finalization, finalized old-key blocks, same-shard incoming `Transfer` redirect to rescue and reject/no-mutation tests; ticket [20260517-v4-sprint4-emergency-routing.json](../tasks/20260517-v4-sprint4-emergency-routing.json), review [20260517-v4-sprint4-emergency-routing-review.md](reviews/20260517-v4-sprint4-emergency-routing-review.md). |
| [x] | Operator path: CLI `tx-policy-*`, V4 `tx-init` flags, rescue cosign UX, TUI/API inspection fields and docs; ticket [20260517-v4-sprint5-cli-tui.json](../tasks/20260517-v4-sprint5-cli-tui.json), review [20260517-v4-sprint5-cli-tui-review.md](reviews/20260517-v4-sprint5-cli-tui-review.md). |
| [x] | Integrated V4 gate: `cargo fmt --check`, `cargo check --workspace`, `cargo test -p pwmd --lib`, `cargo test -p pwm-core --lib`, full `pwm-cli`, policy filters and snapshot bench compile; smoke report [20260517-v4-integrated-smoke.md](reviews/20260517-v4-integrated-smoke.md), ticket [20260517-v4-sprint6-closeout.json](../tasks/20260517-v4-sprint6-closeout.json). |
| [x] | **Demo publication slice (thin operator harness):** live CY policy-matrix smoke **`scripts/cy_cluster_policy_matrix_e2e.ps1`** exercised via **`pwm-testing`** + **`cq_process_ctl`** (**PASS**, **exit 0**); runbook [runbooks/cy-cluster-policy-matrix-e2e.md](runbooks/cy-cluster-policy-matrix-e2e.md), ticket [20260517-cy-cluster-policy-matrix-e2e-live.json](../tasks/20260517-cy-cluster-policy-matrix-e2e-live.json); commit после фиксации harness. Явно **не** многопользовательский/regression/soak-слой: долговременную симуляцию активности и расширенные policy-кейсы оставить на рост протокола / отдельные тикеты. |
| [x] | **V4.x minimal path (spec only):** ADR 0005 deferred activation — **Accepted** в V5-1 ([adr/0005-policy-deferred-activation.md](adr/0005-policy-deferred-activation.md)); runtime — V5-4. Address flags / `conservation` — ADR 0006 (spec only, enforcement V6). |
| [~] | Full `cargo test --workspace`, manual TUI operation and long-running devnet soak were not part of V4-6; keep them as optional hardening gates before a public testnet announcement. |

---

## 0v5. MVP v5 tokenomics hardening (CY E2E PASS; sprint-final closeout PASS — owner sign-off pending)

| Статус | Пункт |
|--------|--------|
| [x] | **V5-1 spec/RFC/ADR freeze:** RFC 0012 v2 lazy marks (staked-only, hours, saturation, touch semantics), RFC 0011/0013/0014 ClaimTx retirement addenda, RFC 0019 float inflation, ADR 0005 → Accepted, ADR 0006 address flags (spec only), ADR 0007 domain lease governance; review-fixes + rereview gate PASS; tickets [20260523-v5-sprint1-spec-adr-freeze.json](../tasks/20260523-v5-sprint1-spec-adr-freeze.json), [20260523-v5-sprint1-review-fixes.json](../tasks/done/20260523-v5-sprint1-review-fixes.json), [20260523-v5-sprint1-review-rerun.json](../tasks/20260523-v5-sprint1-review-rerun.json); reviews [20260523-v5-sprint1-spec-adr-freeze-review.md](reviews/20260523-v5-sprint1-spec-adr-freeze-review.md), [20260523-v5-sprint1-spec-adr-freeze-rereview.md](reviews/20260523-v5-sprint1-spec-adr-freeze-rereview.md) (2026-05-23). |
| [x] | **V5-2 core model:** GenCfg + ClaimPhaseConfig; Account V5 fields (`marks_last_block` height cursor, `deferred_policies`, `ipv4_claimed_phase`); legacy ClaimTx retired; `ClaimIPv4Batch` tx shape; snapshot schema v3; review-fixes + rereview + integrated testing PASS; tickets [20260524-v5-sprint2-core-model.json](../tasks/done/20260524-v5-sprint2-core-model.json), slices `20260524-v5-s2-slice1`…`slice5`, [20260524-v5-s2-review-fixes.json](../tasks/done/20260524-v5-s2-review-fixes.json), [20260524-v5-s2-review-fixes-rereview.json](../tasks/done/20260524-v5-s2-review-fixes-rereview.json), [20260524-v5-s2-review-fixes-testing.json](../tasks/done/20260524-v5-s2-review-fixes-testing.json); reviews [20260524-v5-s2-review-fixes-rereview.md](reviews/20260524-v5-s2-review-fixes-rereview.md) (2026-05-24). |
| [x] | **V5-3 lazy marks + float inflation engine:** `compute_lazy_marks` / `compute_block_reward`; RFC 0012 v2 touch matrix in state; seal float reward; slices 1–3 review+testing PASS; umbrella [20260524-v5-sprint3-lazy-marks-inflation.json](../tasks/done/20260524-v5-sprint3-lazy-marks-inflation.json); intro [tasks/introductory/20260524-v5-s3-lazy-marks-inflation.md](../tasks/introductory/20260524-v5-s3-lazy-marks-inflation.md); reviews [20260524-v5-s3-slice3-chain-seal-review.md](reviews/20260524-v5-s3-slice3-chain-seal-review.md) (2026-05-24). |
| [x] | **V5-4 deferred activation runtime:** `ActivationMode::Deferred { activate_at_height }`, height-gated `evaluate_policy`, Activate/Deactivate rejects, RFC 6/7 normative, snapshot `deferred:<height>` wire; slices 1–3 review+testing PASS; umbrella [20260524-v5-sprint4-deferred-activation.json](../tasks/done/20260524-v5-sprint4-deferred-activation.json); intro [tasks/introductory/20260524-v5-s4-deferred-activation.md](../tasks/introductory/20260524-v5-s4-deferred-activation.md); reviews [20260524-v5-s4-slice3-spec-tests-review.md](reviews/20260524-v5-s4-slice3-spec-tests-review.md) (2026-05-24). |
| [x] | **V5-5 IPv4 Claim on-chain:** `ClaimIPv4Batch` validate/apply, registry sig, double-claim reject; slices 1–2 review+testing PASS; umbrella [20260524-v5-sprint5-ipv4-claim-onchain.json](../tasks/done/20260524-v5-sprint5-ipv4-claim-onchain.json); intro [tasks/introductory/20260524-v5-s5-ipv4-claim-onchain.md](../tasks/introductory/20260524-v5-s5-ipv4-claim-onchain.md); reviews [20260524-v5-s5-slice2-reject-fixture-review.md](reviews/20260524-v5-s5-slice2-reject-fixture-review.md) (2026-05-24). |
| [x] | **V5-6 TUI marks saturation:** effective_marks at head height, marks_last_block API, saturation column; slices 1–2 review+testing PASS; umbrella [20260524-v5-sprint6-tui-marks-saturation.json](../tasks/done/20260524-v5-s6-tui-marks-saturation.json); intro [tasks/introductory/20260524-v5-s6-tui-marks-saturation.md](../tasks/introductory/20260524-v5-s6-tui-marks-saturation.md); reviews [20260524-v5-s6-slice2-ui-saturation-column-review.md](reviews/20260524-v5-s6-slice2-ui-saturation-column-review.md) (2026-05-24). |
| [x] | **V5-7 CLI + genesis doc:** `account-info` marks detail, `tx-policy-set --activation deferred --activate-at-height`, `docs/genesis-21b-design.md`; slices 1–3 review+testing PASS (slice2 rerun); umbrella [20260524-v5-sprint7-cli-genesis-doc.json](../tasks/done/20260524-v5-sprint7-cli-genesis-doc.json); intro [tasks/introductory/20260524-v5-s7-cli-genesis-doc.md](../tasks/introductory/20260524-v5-s7-cli-genesis-doc.md) (2026-05-24). |
| [x] | **V5-8 integrated gate + closeout:** operator smoke harness `scripts/devnet_v5_operator_smoke.ps1` covers marks/inflation (slice1 PASS: `tmp/devnet_v5_operator_smoke_20260524_192234.md`), deferred activation (slice2 PASS: `tmp/devnet_v5_operator_smoke_20260525_143518.md`), ClaimIPv4Batch (slice3 PASS: `tmp/devnet_v5_operator_smoke_20260528_080852.md`), account-info CLI marks (slice4 PASS: `tmp/devnet_v5_operator_smoke_20260528_085451.md`); commits fd94191, c930024, f5d4535, f21f243; umbrella [20260524-v5-sprint8-operator-closeout.json](../tasks/20260524-v5-sprint8-operator-closeout.json); review [20260524-v5-sprint8-closeout-review.md](reviews/20260524-v5-sprint8-closeout-review.md) (2026-05-28). |
| [x] | **V5-9 pre-closeout CY E2E:** live CY cluster multi-hour soak — s1 bootstrap/stability PASS, s2-rerun marks saturation soak PASS (PARTIAL: 2 staked), s3 mass burn batches PASS; umbrella [20260529-v5-precloseout-cy-e2e-umbrella.json](../tasks/done/20260529-v5-precloseout-cy-e2e-umbrella.json) done; reports `tmp/cy-e2e-s1-20260528_220256.md`, `tmp/cy-e2e-s2-20260530_082418.md`, `tmp/cy-e2e-s3-20260530_141317.md`; doc alignment [20260530-v5-precloseout-cy-e2e-docs-version-review.md](reviews/20260530-v5-precloseout-cy-e2e-docs-version-review.md) PASS_WITH_NITS (nиты закрыты 2026-05-30); sprint-final [20260530-v5-sprint-final-closeout-review.md](reviews/20260530-v5-sprint-final-closeout-review.md) PASS. |

---

## 1. Спецификация и решения

| Статус | Пункт |
|--------|--------|
| [x] | `docs/WHITE_SPEC_v0.md` — цели v0, AccountId, tx-типы, упрощения |
| [x] | `docs/OFFCHAIN_STUB.md` — роль заглушки batch-burn |
| [x] | `docs/adr/0001-consensus-and-node-stack.md` — выбор своего узла (PoA dev), не CometBFT в v0 |
| [x] | Привести `WHITE_SPEC` и код к одному виду: human-адрес `PWMv0-…` в CLI/TUI; подпись/хэш tx — §3 WHITE_SPEC согласовано с `signing_message()` (см. [reviews/pwm-mvp-20260418.md](reviews/pwm-mvp-20260418.md) §2) |
| [x] | Термин **matrixchain**: [MATRIXCHAIN_SPEC_v0.md](MATRIXCHAIN_SPEC_v0.md) (сравнение с whitepaper + ось v0) |
| [x] | `docs/rfc/9-crossdomain-roaming.md` — as-implemented контракт Sprint 13 для cross-domain roaming MVP (baseline vs out-of-scope) |
| [x] | Операторские docs по Sprint 13 roaming: [ROAMING-SAMPLE.md](ROAMING-SAMPLE.md) + [GEO-SHARDING-EXPLANATION.md](GEO-SHARDING-EXPLANATION.md) |
| [x] | S15 closeout отладки межшарда: [ROAMING_COMPLETION.md](ROAMING_COMPLETION.md) + [sprint-15-s3-17-closeout.md](reviews/sprint-15-s3-17-closeout.md) (2026-05-01) |
| [x] | Stabilization slicing (2026-05-03): [cross-shard-stabilization-slicing-20260503.md](reviews/cross-shard-stabilization-slicing-20260503.md) — Slice A..E с test gates/risks/acceptance для `pwm-coding -> pwm-testing -> pwm-review` |
| [x] | RFC delta для межшардовой стабилизации MVP: deterministic target provenance, automatic reimport/backfill, offline repair + crash-fast, future settlement-chain note; **доп. направление (только RFC, без кода):** source-side lock / conditional finalize — [rfc/9-crossdomain-roaming.md](rfc/9-crossdomain-roaming.md) Appendix A.5 |
| [ ] | Протокольная блокировка UTXO/стоимости на `EXPORT` до финализации `IMPORT` — отложено до отдельной спеки (см. RFC 0009 §A.5 и §12); в MVP не реализовывать до согласования proof/finality, таймаута и fork-правил |
| [x] | Формализация **отказа в федеративном доверии** при расхождении **мостового** (level-2) учёта и **закрытие one-window** для клиентов: `WHITE_SPEC_v0.md` §7.5, `rfc/9-crossdomain-roaming.md` Appendix A.6, `GEO-SHARDING-EXPLANATION.md` §8 |
| [x] | Реализация в `pwmd`: bridge commitment, обнаружение расхождения, readiness + отключение one-window/foreign observability при bridge trust refusal (2026-05-04; same/cross-shard hello — см. RFC A.6, `POST /v1/bridge-federation/reset`) |
| [x] | **Slice F** (2026-05-04): адаптация §7.5/A.6 в рантайм + лог `pwmd-peer-*.log` без консоли + снижение шума reconnect — [slice-f-bridge-trust-peer-logging-20260504.md](reviews/slice-f-bridge-trust-peer-logging-20260504.md), `tasks/slice-f.json` |

---

## 2. Репозиторий и инфраструктура

| Статус | Пункт |
|--------|--------|
| [x] | Git: `git init`, ветка `main`, коммиты с кодом |
| [x] | Workspace: `pwm-core`, `pwmd`, `pwm-cli`, `pwm-tui` |
| [x] | `cargo check` / `cargo test -p pwm-core` |
| [x] | Push на `local`/Gitea по необходимости (выполнен после фиксации полного MVP-чеклиста) |
| [x] | `PWM-cryptocurrency` в `git.local/setup-repos.sh` |
| [x] | MCP-корни для multi-root (`cqds-cursor.code-workspace`) |
| [x] | Промпты для агентов: [AGENT_PROMPTS.md](AGENT_PROMPTS.md) |
| [x] | `.gitattributes`: есть `* text=auto`; добавлены явные LF-паттерны (`*.rs`, `*.toml`, `docs/**/*.md`) (2026-05-06) |
| [x] | Низкий приоритет: единый язык комментариев в `pwm-core` (ревью §3 — смесь RU/EN) |

---

## 3. Ядро `pwm-core`

| Статус | Пункт |
|--------|--------|
| [x] | `crypto`: blake3, Ed25519 sign/verify, hash для заголовка блока |
| [x] | `hd`: SLIP-0010 `m/0'/i`, brute по `domain_code`, `account_id_from_parts` |
| [x] | `types`: `AccountId`, `Account` (+ `genesis_funded`) |
| [x] | `tx` + `ser_bin`: `TxBody`, `SignedTx`, подпись, serde для sig |
| [x] | `state.apply_tx`: ветки tx; **INIT** создаёт stub-счёт при отсутствии записи |
| [x] | `state::digest` — корень состояния (bincode+blake3) |
| [x] | `block`: `BlockHdr`, `Block`, `txs_root`, `hdr_hash` |
| [x] | `mempool`: `Mpool` FIFO, cap |
| [x] | `genesis`: `GenCfg`, `GRow`, `dev_net()` (исторический legacy-вектор: seed **`[99;32]`**, `m/0'/0'`; активный genesis flow для `pwmd --genesis-file` — schema v4 через `pwm genesis-build`) |
| [x] | `chain`: `Chain::boot`, `seal`, `prev_gen`, PoA ротация по индексу |
| [x] | `offchain`: `merkle_root`, `sign_batch`, `batch_preimage` |
| [x] | Юнит-тесты: `hd::tests`, `chain::tests::seal_empty_block` |
| [x] | Расширить тесты `state` / `validate_tx_shape`: INIT→TRANSFER, неверный nonce, insufficient, domain mismatch (`state::tests`) |
| [x] | Тесты сценария **seal + mempool**: `SealAbort` + `Mpool::prepend_block`; регрессия в `chain::tests`, `mempool::tests` (см. [reviews/pwm-mvp-20260418.md](reviews/pwm-mvp-20260418.md) §4) |
| [x] | Интеграционный smoke `pwmd`: `crates/pwmd` как `lib` + тесты `oneshot` (`/v1/head`, `POST /v1/tx`, лимит тела) |

---

## 4. Нода `pwmd`

| Статус | Пункт |
|--------|--------|
| [x] | Крейт + axum/tokio/tower-http CORS |
| [x] | REST: `GET /v1/head`, `GET /v1/accounts`, `GET /v1/account/:hex`, `POST /v1/tx` |
| [x] | Фон: `seal` до 64 tx из пула (пустой блок тоже) с cadence из genesis `blocks_per_hour`: `seal_interval_ms = 3_600_000 / blocks_per_hour` |
| [x] | **Приоритет (ревью):** при ошибке `seal` не терять изъятые из пула tx — `Chain::seal` → `SealAbort`, `pwmd` вызывает `Mpool::prepend_block` |
| [x] | `POST /v1/tx`: ранняя `validate_tx_shape` до пула; `DefaultBodyLimit` **256 KiB** на роутере (в т.ч. localhost) |
| [x] | CORS: permissive только на **loopback** bind; иначе обязателен **`PWM_CORS_ORIGINS`** (список через запятую) |
| [x] | Persist цепи на диск: JSON-снапшот `--data-file` (по умолчанию `pwm-data.json`) с `blocks` + `state`, загрузка при старте с проверкой совместимости genesis |
| [x] | Флаги CLI: **`--listen`** (по умолчанию `127.0.0.1:3030`), **`--genesis-file`** (schema v4 JSON: `gen_cfg.funding.accounts` + `gen_cfg.validators.set` + `validator_keys[*].enc_seed`; plaintext `validator_seeds_hex` не поддерживается) |
| [x] | Cross-shard stabilization MVP: `handoff_register` не мутирует replay-critical `State.exported_registry` вне блока; provenance входит в deterministic block path (`Import`/эквивалент) |
| [x] | Automatic reimport/backfill после cleanup/rollback target: trusted peer (`network_id`/`genesis_hash`), idempotent inclusion, replay validate после восстановления |
| [x] | Offline repair path: rollback до последней воспроизводимой высоты + безопасная перезапись epoch/manifest/summary + validate-after-write |

---

## 5. CLI `pwm` (`pwm-cli`, бинарник `pwm`)

| Статус | Пункт |
|--------|--------|
| [x] | `key-gen`, `addr-derive`, `tx-init`, `tx-send` (nonce с RPC) |
| [x] | `off-demo` — демо Merkle + подпись batch (JSON в stdout) |
| [x] | `tx-stake` / `tx-unstake` / `tx-burn-mark` |
| [x] | Единый `--rpc` / env `PWM_RPC` в CLI (`pwm`; глобальный флаг для `tx-init` / `tx-send`; TUI уже читал `PWM_RPC`) |

---

## 6. TUI `pwm-tui`

Целевой дизайн и профили **Public / Debug**: [TUI_SPEC_v0.md](TUI_SPEC_v0.md).

| Статус | Пункт |
|--------|--------|
| [x] | Публичный вид: **таблица** счетов сети (PWM / Staked / Marks / Init), короткий id |
| [x] | Строка «selected» с полным hex и nonce |
| [x] | Опционально **Debug**: `PWM_TUI_DEBUG=1` — нижняя панель с JSON (`GET /v1/account/...`) |
| [x] | Опрос `PWM_RPC`, ~1s; `q` / **F10** — выход; стрелки — выбор строки |
| [x] | Панели «владелец / получатели», Tab-фокус, **F5/F6** модалки — по TUI_SPEC §2 (F6: рабочая send-форма с полями, валидацией, submit/status; F5 остаётся в рамках текущего MVP-сценария) |
| [x] | Нижняя строка с подсказками F1–F10 (как mc): отражают текущие действия F5/F6 согласно [TUI_SPEC_v0.md](TUI_SPEC_v0.md) §2 |

---

## 6b. Phase 1 (bech32DX + wallet/TUI) — текущий статус

| Статус | Пункт |
|--------|--------|
| [x] | `cargo test --workspace` зелёный (актуальная проверка после Sprint 1C обновлений) |
| [x] | TUI send-flow по F6 реализован: from/to/amount/fee/confirm, локальные валидации, submit в `POST /v1/tx`, отображение статуса/ошибки |
| [x] | CLI smoke `wallet + send` проходит, включая pretty recipient |
| [x] | Ручной операторский TUI smoke (2026-05-04): happy-path; негативы — пересылка при заблокированном кошельке, на неинициализированный адрес, TUI при остановленном `pwmd`; расширенные негативы и RPC-параллели — [tester-guide-cli-tui-scenarios.md](tester-guide-cli-tui-scenarios.md) §«Негативные сценарии» |

---

## 7. Вне скоупа MVP (напоминание)

- Политики «глупых контрактов», арбитры, шардинг консенсуса, PQC, IPv4-клайминг, полная инфляция.
- Подробная **документация кодовой базы** по каждому компоненту — отдельные TODO в трекере, после стабилизации API.
- Масштабируемое межшардовое **read-наблюдение** без перегрузки сети (global explorer, подписки клиента по адресам) — не цель текущего MVP-паттерна «одного окна»; см. [reviews/sprint-15-s3-12-9-closeout.md](reviews/sprint-15-s3-12-9-closeout.md).
- **S15 слайс O** (оптимизация / decomposition): [CODEBASE_REFACTORING.md](CODEBASE_REFACTORING.md), [reviews/sprint-15-slice-O-checklist.md](reviews/sprint-15-slice-O-checklist.md) — post-MVP backlog, не блокирует объявление текущего MVP baseline.

---

## 8. Отложено: подробная документация по компонентам (dev-related)

- [x] Док: крейт `pwm-core` — [pwm-core.md](pwm-core.md).
- [x] Док: нода `pwmd` — [pwmd.md](pwmd.md).
- [x] Док: `pwm-cli` — [pwm-cli.md](pwm-cli.md).
- [x] Док: `pwm-tui` — [pwm-tui.md](pwm-tui.md).
- [x] Док: оффчейн batch — [offchain-batch.md](offchain-batch.md).

---

## 8b. Отложено: эксплуатационная документация (end-user / tester)

- [x] Tester guide: установка/запуск devnet и базовые smoke-сценарии (без деталей внутренней реализации) — [tester-guide-devnet-smoke.md](./tester-guide-devnet-smoke.md).
- [x] Tester guide: сценарии CLI/TUI проверки и ожидаемые результаты — [tester-guide-cli-tui-scenarios.md](./tester-guide-cli-tui-scenarios.md).
- [x] Tester guide: типовые ошибки окружения и быстрые шаги восстановления — [tester-guide-env-errors-recovery.md](./tester-guide-env-errors-recovery.md).

---

## 9. Запуск (devnet) [x]

1. [x] Терминал A: `cargo run -p pwmd --bin pwmd` (опционально `--listen 127.0.0.1:3030`, `--genesis-file path.json`, `--data-file pwm-data.json`). CORS: на loopback — permissive; иначе задайте **`PWM_CORS_ORIGINS`** (через запятую).
2. [x] Терминал B: `cargo run -p pwm-tui --bin pwm-tui` (опционально `PWM_RPC`, для JSON-режима — `PWM_TUI_DEBUG=1`).
3. [x] Кошелёк: `cargo run -p pwm-cli --bin pwm -- key-gen` → hex seed; `addr-derive --master <hex> --domain <hex u16>`.
4. [x] Валидатор в genesis — отдельный master: `dev_net()` использует **`[99u8;32]`** (в hex — 32 повтора байта `99`, всего 64 hex-символа; см. `genesis::dev_net`).
5. [x] `tx-init` / `tx-send` шлют JSON в `POST /v1/tx` (база URL: глобальный `pwm --rpc <url> …` или `PWM_RPC`, по умолчанию `http://127.0.0.1:3030`).

---

## 10. Отчёты ревью (бэклог) [x]

- [x] [reviews/pwm-mvp-20260418.md](reviews/pwm-mvp-20260418.md) — статический обзор + `cargo test`; пункты **Request changes** перенесены в §3–§6 выше (мемпул/seal — высокий приоритет).
- [x] Multi-sprint closeout — граница **CQDS/MCP vs продуктовая нода**: [reviews/mvp-closeout-operator-notes.md](reviews/mvp-closeout-operator-notes.md).

---

## 11. Веха: параллельная валидация (ручная + агенты) [x]

**Цель:** можно независимо гонять сценарии из §9 и доверять регрессии по `cargo test`.

| Поток | Делает | Промпт / субагент |
|--------|--------|--------|
| Оркестратор | План, тикеты [`tasks/`](../tasks/README.md), локальные коммиты, вызовы субагентов | [AGENT_PROMPT_orchestrator.md](AGENT_PROMPT_orchestrator.md) |
| Код | Реализация по чеклисту | [AGENT_PROMPT_coding.md](AGENT_PROMPT_coding.md) → **`pwm-coding`** |
| Тесты | §3–§6 тесты и галочки | [AGENT_PROMPT_testing.md](AGENT_PROMPT_testing.md) → **`pwm-testing`** |
| Ревью | Отчёт без правок кода | [AGENT_PROMPT_review.md](AGENT_PROMPT_review.md) → **`pwm-review`** |

**Критерий «первая тестовая версия CLI» для внешней проверки:** `cargo test -p pwm-core` зелёный; сценарий §9 (локальный `pwmd` + `pwm key-gen` / `addr-derive` / `tx-init` / `tx-send`) воспроизводим; единый `PWM_RPC` / `--rpc` в CLI — §5 `[x]`; дальше по желанию: smoke `pwmd`, тест mempool+seal после фикса ноды.

---

## 12. Явные defer / backlog (не блокируют baseline)

| Статус | Пункт |
|--------|--------|
| [x] | §1: «Протокольная блокировка UTXO/стоимости на `EXPORT` до финализации `IMPORT`» формально вынесена в defer/backlog и отложена до отдельной спеки (RFC 0009 §A.5); пункт в §1 сохранён как `[ ]` напоминание |
