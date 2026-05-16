---
name: MVP v3 Foundation Plan
overview: Roadmap MVP v3 — foundation-only версия перед V4/V5: versioned Epoch Snapshot manifest и migration contract, replay determinism gate, freeze публичного `/v1/*` API, demo genesis package для открытого devnet и ADR-пакет protocol-freeze; главный demo-ready результат — внешний тестер поднимает public devnet по runbook без чтения исходников.
todos:
  - id: v3-sprint-1-spec-adr-api
    content: "Sprint V3-1: spec freeze base — `/v1/*` API skeleton, ADR index/format, ADR IPv4/offchain/cleanup-chain drafts"
    status: completed
  - id: v3-sprint-2-snapshot-replay
    content: "Sprint V3-2: Epoch Snapshot schema versioning + compatibility gate + replay determinism gate"
    status: completed
  - id: v3-sprint-3-demo-genesis-devnet
    content: "Sprint V3-3: demo genesis package + public devnet one-command/near-one-command runbook"
    status: completed
  - id: v3-sprint-4-public-devnet-closeout
    content: "Sprint V3-4: end-to-end public devnet smoke, final review gate, V4/V5 backlog separation"
    status: completed
isProject: false
---

# MVP v3 Foundation Plan

## Цель и формат

- **Цель:** стабилизировать основание проекта перед следующими функциональными этажами. V3 не добавляет policy engine, IPv4 claiming runtime или production offchain API; она фиксирует контракты, без которых V4/V5/V7 будут опираться на плавающие допущения.
- **Главный demo-ready результат:** внешний тестер может поднять **public devnet** из чистого клона по понятному runbook, получить воспроизводимый demo genesis, проверить базовый `/v1/*` API curl-командами и иметь минимальный replay/snapshot gate для доверия к состоянию.
- **Scope:** foundation-only из [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md): Epoch Snapshot schema versioning, `/v1/*` API freeze, demo genesis package, replay determinism CI/local gate, ADR по IPv4 Claiming, Offchain Scaling Model, Cleanup-chain / Bootstrap Snapshot / external anchoring.
- **Out of scope для V3:** реализация policy engine, IPv4 allocation runtime, production offchain batch API, domain auctions, stake-based validator admission, `X-PWM-Mark` / `X-PWM-Auth` / media-anchor integration specs как обязательный gate. Эти темы допускаются только как backlog/notes после foundation closeout.
- **Формат:** слайсы V3 могут быть короткими и doc-first; порядок ниже отражает зависимости, а не календарную длительность.
- **Критерий завершения спринта:** каждый спринт оставляет воспроизводимый артефакт: спецификацию, тест/gate, runbook или review report.

**Приоритет плана:** V3 стартует после закрытия V2-9 по RFC 16. Legacy V2-8 Slice 6 wave-pack на multi-sealer пути не является блокером V3; цели V2-8 волн перекрыты V2-9 single proposer + attestation контрактом.

## Зависимости между спринтами

- **V3-1** фиксирует документы и решения, на которые будут ссылаться кодовые слайсы.
- **V3-2** опирается на терминологию V3-1: текущий `Epoch Snapshot` не равен будущему `Bootstrap Snapshot`; compatibility contract не должен обещать pruning semantics.
- **V3-3** опирается на API freeze из V3-1 и replay/schema gate из V3-2, чтобы devnet runbook был проверяемым.
- **V3-4** закрывает версию только после public-devnet smoke и независимого review.

```text
V3-1 -> V3-2 -> V3-3 -> V3-4
  |                 ^
  +-----------------+
```

Смысл: API/ADR база нужна уже для demo genesis runbook, но часть runbook можно готовить параллельно с уточнением snapshot gate, если слайсы не конфликтуют.

## Обязательный ритуал в начале каждого спринта

- Перед реализацией: создать/обновить `tasks/<id>.json` со статусом `in_progress`, scope, acceptance criteria и planned delegations.
- Если спринт широкий, сначала дать `pwm-info` на reuse-карту файлов и документов.
- Для кодовых слайсов держать конвейер **`pwm-coding` -> `pwm-testing` -> `pwm-review`**.
- Для doc-only слайсов допускается оркестраторская правка `docs/`, но финальный quality gate всё равно отдавать `pwm-review`, если документ становится контрактом версии.

## Обязанности оркестратора

- Держать фокус V3 на public devnet foundation, не втягивать V4/V5 feature scope.
- Разделять **публичный API-контракт** и **dev/operator endpoints**.
- Не править `crates/` напрямую: кодовые изменения делегируются `pwm-coding`.
- В каждом handoff для `pwm-*` субагентов явно требовать skill `colloquium-cqds-mcp` как primary runtime-guide для CQDS и запрет на широкий локальный grep/MCP-source mining.
- Вести `tasks/*.json`: delegations, token estimates, artifacts, review links, status.

## Базовые артефакты перед Sprint V3-1

- [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md) — секция **MVP V3 — Основание и стабилизация**, риски R1, R4, R5, R8, R10-R12.
- [DRAFT_WHITEPAPER-ru.md](../../DRAFT_WHITEPAPER-ru.md) — исходный продуктовый контекст; где расходится с roadmap, приоритет у актуального roadmap.
- [mvp_v2.md](mvp_v2.md) — закрытый контекст V2, особенно V2-8/V2-9 и public snapshot после RFC 16.
- [pwmd.md](../pwmd.md) — текущий операторский источник по `/v1/*`, CLI и storage.
- [guide-node-storage-and-snapshot.md](../guide-node-storage-and-snapshot.md), [runbook-store-snapshots.md](../runbook-store-snapshots.md) — текущие контракты `Epoch Snapshot`.
- [rfc/5-genesis-and-bootstrap.md](../rfc/5-genesis-and-bootstrap.md), [GENESIS_BLOCK.md](../GENESIS_BLOCK.md) — genesis/bootstrap контекст.
- [OFFCHAIN_STUB.md](../OFFCHAIN_STUB.md), [offchain-batch.md](../offchain-batch.md) — текущий offchain stub и batch model.
- [adr/0001-consensus-and-node-stack.md](../adr/0001-consensus-and-node-stack.md) — текущий ADR-формат и принятый PoA/devnet baseline.

## Текущее состояние кода и документов (ориентиры)

- **Epoch Snapshot manifest:** `crates/pwmd/src/snapshot/epoch.rs` содержит `schema_v`, `canonical_h`, `tip_hash`, `epoch_span` и список epoch-файлов; текущий путь фактически принимает только manifest schema `1`.
- **Snapshot load/replay:** `crates/pwmd/src/snapshot/io.rs` и `incremental.rs` различают trust-default load и full replay verification; `--snapshot-verify-chain` уже описан в docs.
- **Genesis schema:** `crates/pwmd/src/snapshot/genesis.rs` загружает legacy/current genesis schemas; `pwm-cli genesis-build` генерирует актуальный genesis.
- **`docs/api-v1.md`:** после Sprint V3-1 существует как draft baseline публичного `/v1/*` freeze и smoke; операторские детали и разрывы всё ещё сверяются с `docs/pwmd.md` и кодом до финального sprint closeout.
- **`docs/adr/`:** помимо принятого ADR 0001 добавлены draft ADR 0002–0004 по темам V3 foundation (IPv4 Claiming direction, Offchain Scaling, Cleanup-chain / Epoch vs Bootstrap Snapshot); статус см. индекс `docs/adr/README.md`.
- **Demo genesis gap:** CY lab и devnet runbooks существуют, но public clean-run package пока не оформлен как самостоятельный one-command/near-one-command путь.

---

## Sprint V3-1: Spec freeze base — API и ADR

**Цель:** создать документальную основу V3: публичный API skeleton, ADR-структуру и черновики/решения по трём protocol-freeze темам.

**Scope:**

- Создать [docs/api-v1.md](../api-v1.md) как будущий источник правды по публичному `/v1/*`.
- Разделить endpoint-классы:
  - public stable `/v1/*` для внешних тестеров и будущего `svcpool.io`;
  - operator endpoints для локального devnet/debug;
  - dev-only endpoints, которые не входят в freeze.
- Создать или обновить ADR index/readme в `docs/adr/`.
- Подготовить ADR:
  - IPv4 Claiming Design — решение уровня архитектуры, реализация defer к V5;
  - Offchain Scaling Model — centralized batch processing как текущий выбор против payment channels;
  - Cleanup-chain / Epoch Snapshot vs Bootstrap Snapshot / external anchoring — границы pruning, archive commitments, optional L1 anchors.

**Slices:**

- **Slice 0:** audit текущих источников (`pwmd.md`, review-доки, RFC 14) и список endpoint-классов.
- **Slice 1:** skeleton `docs/api-v1.md` с curl smoke для public devnet.
- **Slice 2:** ADR index + три ADR draft/accepted по решению владельца.
- **Slice 3:** `pwm-review` на отсутствие противоречий между roadmap, whitepaper, V2 plan и новыми docs.

**Acceptance criteria:**

- `docs/api-v1.md` существует и явно помечает, что входит в public `/v1` contract, а что является dev/operator surface.
- В `docs/adr/` есть индекс и три V3 ADR с понятным статусом.
- ADR не обещают реализацию V4/V5/V7 в V3, а фиксируют границы и выбранное направление.
- `tasks/<id>.json` содержит delegations и ссылки на созданные artifacts.

**Файлы/модули (ориентир):**

- `docs/api-v1.md`
- `docs/adr/README.md`
- `docs/adr/0002-ipv4-claiming-design.md`
- `docs/adr/0003-offchain-scaling-model.md`
- `docs/adr/0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md`
- `docs/pwmd.md`, `docs/rfc/14-claim-burn-api-error-contract.md`, `docs/CONCEPT_ROADMAP.md`

**Demo-ready output:** внешний тестер видит, какие `/v1` запросы можно считать стабильным devnet API, и какие ADR ограничивают будущие изменения.

---

## Sprint V3-2: Epoch Snapshot schema versioning и replay gate <!-- status: completed 2026-05-16 -->

**Цель:** превратить текущий `schema_v` manifest в явный compatibility contract и добавить focused gate против replay-path divergence.

**Scope:**

- Зафиксировать тип и семантику `schema_v` в `pwm-epochs-manifest.json` (не смешивать с genesis schema и wire snapshot version).
- Добавить compatibility entry point для manifest schema, начиная с текущей версии; полноценная multi-version migration table откладывается до появления schema v2.
- Добавить тесты совместимости текущего manifest и отказа для неподдерживаемой версии.
- Добавить replay determinism gate: local command и/или CI job, проверяющий demo genesis / fixture chain / snapshot replay.
- Обновить docs по storage/snapshot, если изменяется операторский контракт.
- Зафиксировать focused gate command `cargo test -p pwmd --lib v3_replay_det_gate_ok` как lightweight проверку replay path.

**Slices:**

- **Slice 0:** `pwm-info` map по snapshot/genesis/replay tests, если V3-1 карта устарела.
- **Slice 1:** schema/compatibility contract в `pwmd` snapshot subsystem.
- **Slice 2:** tests для current manifest compatibility, unsupported version rejection и replay determinism.
- **Slice 3:** docs + `pwm-testing` gate (`cargo test` target + explicit command).
- **Slice 4:** `pwm-review` на compatibility и отсутствие смешения `Epoch Snapshot`/`Bootstrap Snapshot`.

**Acceptance criteria:**

- Manifest schema versioning описан и покрыт тестом.
- Текущие snapshot-файлы версии 1 не ломаются.
- Есть воспроизводимая команда replay determinism gate.
- Изменения не обещают cleanup-chain/pruning semantics, зарезервированные для будущего Bootstrap Snapshot ADR/RFC.

**Файлы/модули (ориентир):**

- `crates/pwmd/src/snapshot/epoch.rs`
- `crates/pwmd/src/snapshot/incremental.rs`
- `crates/pwmd/src/snapshot/io.rs`
- `crates/pwmd/src/tests/*snapshot*`
- `docs/guide-node-storage-and-snapshot.md`
- `docs/runbook-store-snapshots.md`

**Demo-ready output:** оператор может проверить, что snapshot/replay devnet состояния не расходится после чистого запуска и повторной загрузки.

**Статус закрытия (2026-05-16):** Sprint V3-2 закрыт по тикету `tasks/20260516-v3-sprint2-snapshot-replay.json`. Реализован централизованный manifest schema contract (`EPOCH_MAN_SCHEMA_CUR`, `ensure_epoch_man_schema`), focused tests `epoch_man_v*`, lightweight replay gate `v3_replay_det_gate_ok`, и обновлены operator docs. Review gate: `docs/reviews/sprint-v3-2-snapshot-replay-review-20260516.md` — `PASS_WITH_NITS`; оставшийся fail-fast порядок для informational manifest read признан optional polish, не блокер V3-2.

---

## Sprint V3-3: Demo genesis package и public devnet runbook <!-- status: completed 2026-05-16 -->

**Цель:** собрать воспроизводимый public devnet package: genesis, validator set, роли, порты, команды запуска, smoke API.

**Scope:**

- Определить demo validator set и genesis package для public devnet.
- Убрать ручную зависимость от заранее созданного `tmp/genesis-custom.json` в основном demo path или явно автоматизировать его создание.
- Сделать one-command или near-one-command runbook для Windows PowerShell; при возможности зафиксировать Linux/Git Bash parity.
- Связать runbook с `docs/api-v1.md`: curl/API smoke после запуска.
- Добавить troubleshooting для частых ошибок: порты, target dir, passphrase, snapshot state root, peer seeds.

**Slices:**

- **Slice 0:** freeze demo topology: node count, roles, domains, ports, state roots, genesis passphrase policy.
- **Slice 1:** scripts/package path для генерации/запуска demo genesis.
- **Slice 2:** public devnet runbook и API smoke.
- **Slice 3:** `pwm-testing` clean-run на свежем state root.
- **Slice 4:** `pwm-review` на воспроизводимость и отсутствие скрытых локальных предпосылок.

**Acceptance criteria:**

- Внешний тестер может поднять demo devnet без чтения исходников.
- Genesis package воспроизводим и связан с validator set.
- `/v1/status` и минимальные account/tx endpoints проверяются curl/PowerShell командами.
- Runbook явно говорит, что является demo/devnet и не является production security posture.

**Файлы/модули (ориентир):**

- `scripts/*`
- `cy-cluster-*.ps1`
- `docs/tester-guide-devnet-smoke.md`
- `docs/runbooks/*`
- `docs/api-v1.md`
- `crates/pwm-cli/src/cmd_genesis.rs` (только через `pwm-coding`, если понадобится код)

**Demo-ready output:** clean clone -> documented command(s) -> running devnet -> API smoke pass.

**Статус закрытия (2026-05-16):**

- Добавлены скрипты `scripts/demo-genesis-build.ps1`, `scripts/demo-genesis-verify.ps1`, `scripts/demo-devnet-start.ps1` для near-one-command demo path.
- Добавлен runbook `docs/runbooks/demo-devnet-quickstart.md` с premine math (`21_000_000_000_000_000 raw`) и `/v1/*` smoke.
- CY launcher scripts остаются backward-compatible; demo launcher использует их как primary path после build/verify.
- `pwm-testing` подтвердил dry-run scripts, fail-fast verifier и реальную генерацию + проверку premine 21B PWM.
- Review gate: `docs/reviews/sprint-v3-3-demo-genesis-devnet-review-20260516.md` — `PASS_WITH_NITS`. Полный 3-node/API smoke на свежем demo genesis перенесён в Sprint V3-4 integrated closeout из-за уже запущенного операторского CY-кластера; это процессный residual risk, не блокер V3-3.

---

## Sprint V3-4: Public devnet closeout <!-- status: completed 2026-05-16 -->

**Цель:** закрыть V3 как demo-ready foundation и отделить backlog V4/V5.

**Scope:**

- End-to-end smoke public devnet package.
- Финальное ревью V3 artifacts: API docs, ADR, snapshot/replay gate, demo genesis runbook.
- Синхронизация `docs/CONCEPT_ROADMAP.md`, `docs/MVP-checklist.md`, `docs/GLOSSARY.md` при необходимости.
- Backlog: `X-PWM-Mark`, `X-PWM-Auth`, media-anchor concept, V4 policy engine, V5 IPv4/tokenomics/offchain.

**Slices:**

- **Slice 0:** integrated smoke matrix: fresh state, restart from snapshot, API curl, follower/attester topology if included.
- **Slice 1:** final docs consistency pass.
- **Slice 2:** `pwm-review` final V3 gate with sprint-final glossary check.
- **Slice 3:** close tickets and update changelog after accepted gates and owner control.

**Acceptance criteria:**

- V3 public devnet package passes documented smoke.
- Все V3 ADR/API/schema/runbook artifacts linked from the plan or checklist.
- Backlog items are explicitly not blockers.
- `tasks/*.json` for V3 slices are `done` or have explicit blockers.

**Статус закрытия (2026-05-16):** интегрированный smoke после retest — PASS (см. `tasks/20260516-v3-sprint4-public-devnet-smoke.md`, заметки в `tasks/20260516-v3-sprint4-public-devnet-closeout.json`). Финальный review gate: `docs/reviews/sprint-v3-4-public-devnet-closeout-review-20260516.md` — `PASS_WITH_NITS` (housekeeping чеклистов roadmap/MVP-checklist и опциональный smoke для `POST /v1/tx`).

**Файлы/модули (ориентир):**

- `docs/plans/mvp_v3.md`
- `docs/CONCEPT_ROADMAP.md`
- `docs/MVP-checklist.md`
- `docs/GLOSSARY.md`
- `CHANGELOG.md`
- `tasks/*.json`

**Demo-ready output:** V3 можно показать как foundation release для открытого devnet и начала внешнего тестирования.

**Статус закрытия (2026-05-16):** Sprint V3-4 закрыт по тикету `tasks/20260516-v3-sprint4-public-devnet-closeout.json`. Integrated smoke прошёл на свежем demo genesis: deterministic clean wallet/genesis path, premine verifier `21_000_000_000_000_000 raw`, CY 3-node, `/v1/status`, `/v1/head`, `/v1/accounts`, `/v1/account/:id`, cleanup `pwmd` count 0. Финальное ревью `docs/reviews/sprint-v3-4-public-devnet-closeout-review-20260516.md` — `PASS_WITH_NITS`; checklist/roadmap/glossary traceability обновлены. Не блокирующий follow-up: добавить smoke для `POST /v1/tx` при следующем расширении external integration сценариев.

---

## Межспринтовые гейты качества

- **Public Devnet Gate:** clean-run сценарий не требует знания исходников и не зависит от локальных `tmp/` файлов, не созданных runbook/script.
- **API Freeze Gate:** breaking change после V3 требует явного `/v2/*` или documented exception.
- **Snapshot Gate:** текущий `Epoch Snapshot` versioning и replay determinism проверяются автоматизированной командой.
- **ADR Gate:** V4/V5 темы имеют принятое направление, но не расширяют V3 runtime scope.
- **Regression Gate:** V3 не ломает закрытые V2-8/V2-9 инварианты same-shard sync и RFC16 attestation без отдельного тикета.

## Риски и контрмеры

- **Смешение Epoch Snapshot и Bootstrap Snapshot:** контрмера — ADR V3 явно фиксирует различие; V3 код трогает только текущий runtime `Epoch Snapshot`.
- **API freeze захватывает dev endpoints:** контрмера — `docs/api-v1.md` разделяет stable public, operator и dev-only routes.
- **Demo genesis становится локальным скриптом автора:** контрмера — clean-run test и явное описание prerequisites.
- **Replay gate слишком тяжёлый для CI:** контрмера — минимальный fixture/golden gate в CI, расширенный audit command локально.
- **ADR расползаются в реализацию V5:** контрмера — ADR фиксируют решения и deferred implementation boundaries.

## Декомпозиция на таски

- Sprint V3-1: `tasks/20260516-v3-sprint1-spec-adr-api.json`
- Sprint V3-2: `tasks/20260516-v3-sprint2-snapshot-replay.json`
- Sprint V3-3: `tasks/20260516-v3-sprint3-demo-genesis-devnet.json`
- Sprint V3-4: `tasks/20260516-v3-sprint4-public-devnet-closeout.json`

---

_Конец плана MVP v3._
