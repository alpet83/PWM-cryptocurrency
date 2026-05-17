---
name: MVP v4 Policy Engine Plan
overview: Roadmap MVP v4 — runtime policy engine without VM: dedicated PolicyTx with embedded PolicyAction, per-account policy lifecycle, hybrid corporate INIT metadata, emergency routing with rescue-address cosign and irreversible finalization, structured policy rejects, and CLI/TUI inspection paths.
todos:
  - id: v4-sprint-1-rfc-freeze
    content: "Sprint V4-1: freeze policy tx, lifecycle, INIT metadata/rescue, emergency finalization, and policy reject RFCs"
    status: completed
  - id: v4-sprint-2-core-model
    content: "Sprint V4-2: implement core tx/account/snapshot serialization model for policies and INIT extension"
    status: completed
  - id: v4-sprint-3-policy-engine
    content: "Sprint V4-3: implement pure policy evaluator and structured reject integration"
    status: completed
  - id: v4-sprint-4-emergency-routing
    content: "Sprint V4-4: implement emergency routing, rescue cosign, and finalized account behavior"
    status: completed
  - id: v4-sprint-5-cli-tui
    content: "Sprint V4-5: add CLI/TUI/wallet operator paths for policy lifecycle and inspection"
    status: completed
  - id: v4-sprint-6-closeout
    content: "Sprint V4-6: integrated devnet smoke, docs/checklist/glossary/changelog, final review"
    status: completed
isProject: false
---

# MVP v4 Policy Engine Plan

## Цель и формат

- **Цель:** реализовать RFC 6 как bounded runtime policy engine: “dumb contracts” без VM, DSL, callbacks и недетерминированных side effects.
- **Главный demo-ready результат:** оператор может инициализировать аккаунт с V4 metadata/rescue context, установить policy через dedicated `PolicyTx`, активировать/деактивировать обычную policy, активировать emergency routing с rescue cosign и увидеть structured reject для нарушений.
- **Scope:** dedicated `PolicyTx` with embedded `PolicyAction`, `ActivationMode = Dormant | Immediately`, per-account policy state, hybrid corporate INIT metadata, rescue-address emergency routing, irreversible finalized account state, CLI/TUI inspection and minimal mutation flows.
- **Out of scope для V4:** policy DSL/VM, production domain lease auctions, full organization membership registry, governance plugins, V5 tokenomics/IPv4 distribution, V6 stake-based validator admission.
- **Критерий завершения спринта:** каждый спринт оставляет воспроизводимый артефакт: RFC/contract, model+tests, policy evaluator, emergency-routing demo, CLI/TUI path, or integrated review.

## Принятые решения V4

- **Policy update tx:** zero-PWM self-transfer не используется как policy carrier. Обычный self-transfer остаётся запрещённым; V4 вводит dedicated `PolicyTx` with embedded `PolicyAction`.
- **Metadata in INIT:** гибридная модель. Короткие публичные owner/company поля хранятся on-chain с canonical encoding и лимитами; длинные, приватные или изменяемые metadata фиксируются через commitment/hash и external verification/audit reference.
- **Policy lifecycle:** устанавливаемые policies имеют `activation = dormant | immediately`; отдельные action tx активируют/деактивируют policy. Не все policies обязаны быть обратимыми.
- **Emergency routing:** активация требует подпись target account и cosign rescue address; после активации account становится finalized, а обычные операции старым ключом отвергаются.
- **Pure evaluation:** `evaluate_policy(tx, &ReadOnlyState) -> PolicyDecision` не мутирует state и не вызывает внешние сервисы; apply path интерпретирует decision детерминированно.

## Зависимости между спринтами

```text
V4-1 -> V4-2 -> V4-3 -> V4-4 -> V4-5 -> V4-6
```

Смысл: wire/RFC freeze нужен до кодовых слайсов; core model должен появиться до evaluator; emergency routing опирается на evaluator и cosign envelope; CLI/TUI идут после стабильного API/tx shape.

## Обязательный ритуал в начале каждого спринта

- Перед реализацией: создать/обновить `tasks/<id>.json` со статусом `in_progress`, scope, acceptance criteria и planned delegations.
- Если спринт широкий, сначала дать `pwm-info` на reuse-карту файлов и документов.
- Для кодовых слайсов держать конвейер **`pwm-coding` -> `pwm-testing` -> `pwm-review`**.
- Для doc-only слайсов допускается оркестраторская правка `docs/`, но финальный quality gate отдавать `pwm-review`, если документ становится контрактом версии.

## Обязанности оркестратора

- Не размывать V4 в полноценные smart contracts: policy enum only, no VM/DSL/callbacks.
- Не править `crates/` напрямую: кодовые изменения делегируются `pwm-coding`.
- В каждом handoff для `pwm-*` субагентов явно требовать skill `colloquium-cqds-mcp` как primary runtime-guide для CQDS и запрет на широкий локальный grep/MCP-source mining.
- Вести `tasks/*.json`: delegations, token estimates, artifacts, review links, status.
- Автоматически закрывать mechanical `PASS_WITH_NITS` через conveyor, если нит не требует product/protocol/security решения владельца.

## Базовые артефакты перед Sprint V4-1

- [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md) — секция **MVP V4 — Policy Engine Runtime**, risks R2/R12 и use-case gates для V4.
- [DRAFT_WHITEPAPER-ru.md](../../DRAFT_WHITEPAPER-ru.md) — продуктовый контекст “глупых контрактов”; где расходится с roadmap/RFC, приоритет у актуального roadmap/RFC.
- [rfc/6-policy-engine.md](../rfc/6-policy-engine.md) — policy engine baseline and V4 corporate INIT profile.
- [rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md) — transaction/state model and `PolicyTx`.
- [rfc/10-wallet-file-format-v3.md](../rfc/10-wallet-file-format-v3.md) — multi-address wallet foundation for cosign UX.
- [rfc/14-claim-burn-api-error-contract.md](../rfc/14-claim-burn-api-error-contract.md) — structured reject baseline extended by V4 policy codes.
- [api-v1.md](../api-v1.md), [pwmd.md](../pwmd.md), [pwm-cli.md](../pwm-cli.md), [pwm-tui.md](../pwm-tui.md) — public/operator/client contracts to update as features land.

## Итоговое состояние кода и документов после V4 closeout

- `TxBody` включает dedicated `Policy` / `PolicyTx` shape with embedded `PolicyAction`; zero-PWM self-transfer не используется как policy carrier.
- `Account` хранит компактное policy state: active/dormant policies, `rescue_address`, finalized flag и V4 owner/corporate metadata fields.
- `State::apply_tx_with_ctx` и `pwmd` preflight/apply path используют общий deterministic policy verdict contract; V4 closeout smoke подтвердил согласованность policy filters и `pwmd --lib`.
- `SignedTx` поддерживает минимальный cosign envelope для V4 policy actions, включая rescue-address cosign для emergency activation, без general-purpose governance multisig.
- Snapshot/schema conversion включает V4 policy/rescue/finalized state deterministically; V4-2/V4-6 gates покрывают focused snapshot tests и snapshot bench compile.

**Дополнительно (демонстрационный срез, не backlog V4 Sprint):** тонкая живая матрица операторских политик на CY-кластере — **`scripts/cy_cluster_policy_matrix_e2e.ps1`**, **`docs/runbooks/cy-cluster-policy-matrix-e2e.md`**, тикет **`tasks/20260517-cy-cluster-policy-matrix-e2e-live.json`**; прогоны через **pwm-testing** + **`cq_process_ctl`**. Цель — **рабочий демонстратор** и воспроизводимость, а не многопользовательская симуляция: долговременная активность в кластере и расширение кейсов — отдельно по мере протокола.

## Черновик V4.x: отложенная активация (`Deferred`)

- **[ADR 0005](../adr/0005-policy-deferred-activation.md)** (Draft): третий **`ActivationMode`**, параметр **`activate_at_height`**, только чистое расширение evaluator/account state (**без** address flags и **без** delayed `Transfer`).
- Реализация в коде — отдельный тикет после нормализации RFC 0007/0006 ; см. **`tasks/20260517-v4x-deferred-activation-adr.json`**.

---

## Sprint V4-1: Policy RFC freeze and wire boundary

**Цель:** превратить RFC 6 gap в implementable V4 contract before code.

**Scope:**

- Freeze `PolicyTx` with embedded `PolicyAction`: `SetPolicy`, `ActivatePolicy`, `DeactivatePolicy`; emergency activation as special irreversible action.
- Freeze `ActivationMode = Dormant | Immediately` and policy identifiers/versioning.
- Freeze V4 policy enum set: `routing.same_domain_only`, `routing.emergency_redirect`, sender_filter, default_behavior, cosign_required.
- Freeze hybrid corporate INIT extension: public owner/company fields, `metadata_commitment`, `external_verification_ref`, `requested_domain_lo`, optional `rescue_address`, initial policies.
- Extend RFC 14 additively with policy reject classes and stable `E_POLICY_*` codes.

**Acceptance criteria:**

- RFC text states why `PolicyTx` is chosen over self-transfer.
- Emergency routing/finalization semantics are normative enough for implementation.
- No implementation slice starts with unresolved wire ambiguity.
- `tasks/20260517-v4-sprint1-policy-rfc-freeze.json` records review and artifacts.

**Файлы/модули (ориентир):**

- `docs/rfc/6-policy-engine.md`
- `docs/rfc/7-tx-and-state-model.md`
- `docs/rfc/14-claim-burn-api-error-contract.md`
- `docs/CONCEPT_ROADMAP.md`
- `docs/plans/mvp_v4.md`

**Demo-ready output:** команда видит implementable policy wire boundary before touching product Rust.

**Статус закрытия (2026-05-17):** RFC freeze завершён по тикету `tasks/20260517-v4-sprint1-policy-rfc-freeze.json`. Зафиксированы `PolicyTx` with embedded `PolicyAction`, `ActivationMode`, hybrid corporate `INIT`, `rescue_address`, emergency routing/finalization, additive `E_POLICY_*` rejects и JSON/u128 rule для `PolicyTx.fee`. Review gate: `docs/reviews/20260517-v4-sprint1-policy-rfc-freeze-review.md` — `PASS_WITH_NITS`; mechanical nits auto-closed in roadmap/RFC/plan follow-up edits.

---

## Sprint V4-2: Core data model and serialization

**Цель:** добавить минимальные consensus data structures без policy behavior explosion.

**Scope:**

- Extend `TxBody` with dedicated policy action variant(s), not `Transfer` overloading.
- Extend `Account` with compact policy state: active policies, dormant policies, finalized flag, rescue address from INIT where applicable.
- Extend INIT shape for V4 hybrid metadata and rescue address while preserving old minimal INIT compatibility.
- Extend snapshot/schema conversion paths and version gates.
- Add canonical serialization/signing coverage for policy actions and minimal cosign envelope.

**Acceptance criteria:**

- Old INIT and existing V3 devnet fixtures remain loadable or fail with explicit version gate.
- Policy data is deterministic in state root and snapshot replay.
- No dynamic dispatch/DSL/interpreter is introduced.

**Статус закрытия (2026-05-17):** Sprint V4-2 закрыт по тикету `tasks/20260517-v4-sprint2-core-model.json`. Реализованы `PolicyTx`/`PolicyAction`, optional `init_v4`, account policy/rescue/finalized fields, additive snapshot v2 conversion, policy reject mapping and focused tests. Testing PASS: `cargo check -p pwm-core`, `cargo check -p pwmd`, targeted policy/init/snapshot tests, `cargo test -p pwm-core --lib`; final review `docs/reviews/20260517-v4-sprint2-core-model-review.md` — `PASS`.

---

## Sprint V4-3: Pure policy evaluation and structured rejects

**Цель:** реализовать validation engine before complex UX.

**Scope:**

- Introduce read-only policy view and `evaluate_policy` returning enum `PolicyDecision`.
- Enforce baseline policies in apply path and align `pwmd` preflight with apply verdicts.
- Implement sender_filter, same_domain/routing restriction, V4-3 default-deny behavior, and cosign_required for selected actions.
- Add structured policy rejects for missing cosign, finalized account, sender filter, routing denied, emergency unavailable.

**Acceptance criteria:**

- Same input/pre-state returns same policy error in preflight and apply.
- Tests prove policy evaluation has no state mutation.
- Existing transfer/stake/burn/import tests remain compatible except where explicitly gated by new policies.

**V4-3 semantic boundary:** `sender_filter`, `default_behavior`, and generic `cosign_required` are minimal evaluator semantics only. `sender_filter` is a conservative deny placeholder until allow-list/member bindings exist; `default_behavior` means default-deny; generic cosign validation checks canonical intent signature but not corporate membership. Rescue-address cosign and emergency finalization are V4-4.

**Статус закрытия (2026-05-17):** Sprint V4-3 закрыт по тикету `tasks/20260517-v4-sprint3-policy-engine.json`. Реализован pure `evaluate_policy`, preflight/apply alignment through the shared apply path, structured `E_POLICY_*` rejects, same-domain routing deny, conservative sender/default deny, generic cosign-required validation and finalized-account reject scaffold. Testing PASS: `cargo check -p pwm-core`, `cargo check -p pwmd`, `cargo test -p pwm-core policy_` (11/11) plus focused tests. Final review `docs/reviews/20260517-v4-sprint3-policy-engine-review.md` — `PASS`.

---

## Sprint V4-4: Emergency routing, rescue cosign, and finalization

**Цель:** доставить главный V4 differentiator safely.

**Scope:**

- INIT may register `rescue_address`; emergency routing policy may be dormant or immediate.
- Emergency activation requires valid authorization from the account and cosign by rescue address.
- After activation, account becomes finalized: private-key ownership of the old account no longer authorizes ordinary spend/control operations.
- Incoming payments to finalized account route deterministically to rescue address; invalid cases reject with stable policy errors.
- V4-4 redirect applies to incoming `Transfer` only; `Import`/cross-shard ingress parity is backlog unless a later RFC extends emergency routing beyond same-shard transfer semantics.
- Deactivation is allowed for ordinary policies where specified; emergency finalization is irreversible in V4.

**Acceptance criteria:**

- One signature is insufficient for emergency activation.
- Missing rescue address makes emergency activation impossible with explicit error.
- Finalized account cannot spend/unstake/set arbitrary policies by old key.
- Incoming transfer to finalized account lands on rescue address or is rejected by documented routing rules.

**Статус закрытия (2026-05-17):** Sprint V4-4 закрыт по тикету `tasks/20260517-v4-sprint4-emergency-routing.json`. Реализованы rescue-address cosign activation, irreversible finalization, finalized old-key restrictions, same-shard incoming `Transfer` redirect to rescue, missing/uninitialized/cross-domain rescue rejects and no-mutation failure tests. Testing PASS: `cargo check -p pwm-core`, `cargo check -p pwmd`, `cargo test -p pwm-core policy_` (20/20), full `cargo test -p pwm-core --lib` (133 passed, 1 ignored). Final review `docs/reviews/20260517-v4-sprint4-emergency-routing-review.md` — `PASS`.

---

## Sprint V4-5: CLI/TUI/wallet operator path

**Цель:** сделать policies inspectable and usable in devnet.

**Scope:**

- CLI commands for init extension and policy lifecycle, e.g. `tx-policy-set`, `tx-policy-activate`, `tx-policy-deactivate`, `tx-init --owner-* --metadata-commitment --rescue-address`.
- Wallet v3 selection/cosign UX: choose owner account and rescue/cosigner account from one wallet or external signing path.
- TUI inspection first, mutation second: show active/dormant/finalized/rescue state; add minimal forms only after CLI is stable.
- API docs/examples for `POST /v1/tx` policy failures and success paths.

**Acceptance criteria:**

- CLI can set dormant sender_filter and activate it.
- CLI can execute emergency routing activation with rescue cosign.
- TUI displays finalized/rescue/policy state without implying unsupported governance features.

**Статус закрытия (2026-05-17):** Sprint V4-5 закрыт по тикету `tasks/20260517-v4-sprint5-cli-tui.json`. Реализованы CLI `tx-policy-set/activate/deactivate`, V4 `tx-init` flags, rescue cosign UX через wallet v3/external signer, TUI/API inspection fields and docs/examples. Testing PASS: CLI parse/help tests and `cargo check -p pwm-cli`, `cargo check -p pwm-tui`, `cargo check -p pwmd`. Final review `docs/reviews/20260517-v4-sprint5-cli-tui-review.md` — `PASS`.

---

## Sprint V4-6: Integrated devnet gate and closeout

**Цель:** закрыть V4 как coherent policy runtime release.

**Scope:**

- End-to-end smoke on demo devnet: extended INIT, set/activate/deactivate normal policy, emergency activation, finalization, structured rejects.
- Update `docs/MVP-checklist.md`, `docs/CONCEPT_ROADMAP.md`, `docs/GLOSSARY.md`, and `CHANGELOG.md` after accepted gates.
- Final `pwm-review` with sprint-final glossary check.
- Backlog separation for V5 tokenomics, domain leasing auctions, membership registries, full corporate governance, policy DSL.

**Acceptance criteria:**

- All V4 criteria from roadmap are covered or explicitly deferred with owner-approved rationale.
- Policy runtime bypass/cosign bypass bug bounty scope is documentable.
- No policy feature depends on non-deterministic callbacks or external services.

**Статус закрытия (2026-05-17):** Sprint V4-6 закрыт по тикету `tasks/20260517-v4-sprint6-closeout.json`. Integrated gate PASS после исправления `pwmd` JSON contract: `cargo fmt --check`, `cargo check --workspace`, `cargo test -p pwmd --lib`, `cargo test -p pwm-core --lib`, full `pwm-cli`, policy filters and snapshot bench compile. Документы `MVP-checklist`, `CONCEPT_ROADMAP`, `GLOSSARY`, `CHANGELOG` обновлены; full `cargo test --workspace`, ручной TUI smoke and long-running soak оставлены optional hardening. Final review `docs/reviews/20260517-v4-final-closeout-review.md` — `PASS`.

---

## Межспринтовые гейты качества

- **Simplicity Gate:** one compact action enum and one policy enum; no separate tx type per policy.
- **Purity Gate:** evaluator is read-only and deterministic.
- **Wire Gate:** policy rejects use stable `E_POLICY_*` codes and keep RFC 14 extensions additive.
- **Snapshot Gate:** policy state participates in deterministic state root and replay.
- **Security Gate:** emergency finalization is irreversible and requires rescue cosign.

## Риски и контрмеры

- **`PolicyTx` sprawl:** keep one compact action enum and one bounded policy enum.
- **Metadata bloat:** use hybrid metadata with strict on-chain byte limits and commitment for long text.
- **Cosign overgeneralization:** implement only the minimal reusable cosign envelope required by V4 policy actions.
- **Routing side effects inside evaluator:** evaluator returns decision only; apply path performs deterministic routing.
- **Compatibility drift:** preserve legacy INIT semantics and make schema/version changes explicit.

## Декомпозиция на таски

- Sprint V4-1: `tasks/20260517-v4-sprint1-policy-rfc-freeze.json`
- Sprint V4-2: `tasks/20260517-v4-sprint2-core-model.json`
- Sprint V4-3: `tasks/20260517-v4-sprint3-policy-engine.json`
- Sprint V4-4: `tasks/20260517-v4-sprint4-emergency-routing.json`
- Sprint V4-5: `tasks/20260517-v4-sprint5-cli-tui.json`
- Sprint V4-6: `tasks/20260517-v4-sprint6-closeout.json`

---

_Конец плана MVP v4._
