---
name: MVP v1 Testnet Multi-Sprint Plan
overview: Построить многоэтапный roadmap для MVP SPEC v1 testnet (2+ shard, cross-shard transfer) с 2-недельными спринтами и demo-ready результатом после каждого спринта.
todos:
  - id: sprint-1-foundation
    content: "Подготовить Sprint 1 tasks: two-shard runtime foundation + demo checklist"
    status: pending
  - id: sprint-2-export
    content: "Подготовить Sprint 2 tasks: export path, commitment and deterministic export_id"
    status: pending
  - id: sprint-3-finality
    content: "Подготовить Sprint 3 tasks: minimal finality certificate profile"
    status: pending
  - id: sprint-4-import
    content: "Подготовить Sprint 4 tasks: import path and replay protection"
    status: pending
  - id: sprint-5-policy-routing
    content: "Подготовить Sprint 5 tasks: domain_hi routing and baseline policy enforcement"
    status: pending
  - id: sprint-6-pwmd-micro-optimization
    content: "Зафиксировать Sprint 6 tasks: pwmd micro-slice optimization conveyor and evidence policy"
    status: completed
  - id: sprint-7-lib-rs-decomposition
    content: "Подготовить Sprint 7 tasks: декомпозиция тяжелого pwmd lib.rs на субмодули с контролем внешних зависимостей"
    status: completed
  - id: sprint-8-burn-quota
    content: "Подготовить Sprint 8 tasks: marks_quota burn model and fee=0 baseline"
    status: completed
  - id: sprint-9-ux
    content: "Подготовить Sprint 9 tasks: CLI/TUI integration for two-shard operations"
    status: completed
  - id: sprint-10-hardening
    content: "Подготовить Sprint 10 tasks: hardening, conformance, MVP v1 cut"
    status: completed
  - id: sprint-11-domainhi-migration
    content: "Подготовить Sprint 11 tasks: миграция с fixed A/B shard на domain_hi + relay-by-default + deprecated shard alias"
    status: pending
  - id: sprint-12-final-optimization
    content: "Подготовить Sprint 12 tasks: финализирующий optimization sprint после domain_hi migration и глубокого pwm-optimus review"
    status: pending
  - id: sprint-13-intershard-mvp-cut
    content: "Подготовить Sprint 13 tasks: базовый inter-shard MVP с рабочим EXPORT/IMPORT path, replay guard и e2e acceptance"
    status: pending
  - id: sprint-14-multi-address-wallet
    content: "Sprint 14: wallet schema v3 (multi-address), RFC 10, миграция v2, CLI account*, TUI левая панель; конвейер coding→testing→review на код-слайсах"
    status: pending
  - id: sprint-15-cross-shard-consistency-and-state-storage
    content: "Sprint 15: окончательная отладка межшардовых транзакций/балансов с учетом genesis + optional snapshot storage backend (JSON/DB, базовый кандидат ClickHouse)"
    status: completed
isProject: false
---

# MVP v1 Testnet Multi-Sprint Plan

## Цель и формат
- Цель: эволюционно перейти от v0 devnet к v1 testnet (2+ независимых шарда + межшардовый перевод монет) без слома account-based ядра.
- Каденс: спринт = 2 недели.
- Критерий завершения каждого спринта: **внутренний demo-ready инкремент** (запуск, сценарий, краткий guide).

## Обязательный ритуал в начале каждого спринта
- Перед стартом реализации в каждом спринте запускать `pwm-coding` на ручную генерацию sprint-checklist:
  - scope текущего спринта,
  - критерии demo-ready,
  - минимальные негативные проверки,
  - риски и rollback-план.
- Чек-лист публикуется в docs/reviews (или tasks notes) до начала coding/delegation цикла.

## Дополнительные обязанности оркестратора (постоянно)
- Беречь и консолидировать контекст по ходу всего плана:
  - фиксировать принятые решения, развилки, компромиссы и открытые вопросы;
  - сводить результаты делегаций в единый сквозной narrative без потери причинно-следственных связей.
- Придерживаться промта оркестратора:
  - не подменять делегирование ручной реализацией там, где требуются `pwm-coding`/`pwm-testing`/`pwm-review`;
  - поддерживать прозрачную последовательность: coding -> testing -> review -> orchestration decision.
- Поддерживать эффективное делегирование:
  - давать субагентам узкие, проверяемые задачи с явными критериями готовности;
  - ограничивать scope, чтобы избежать расползания реализации.
- Эпизодически мониторить активность субагентов:
  - отслеживать признаки подвисания, зацикливания или ухода в «костыльные» решения;
  - при обнаружении — останавливать цикл, корректировать промпт и возвращать задачу в контролируемый scope.
- Накапливать проверяемые артефакты:
  - короткие промежуточные сводки, ссылки на отчёты, решения по блокерам;
  - чтобы финальная сводка спринта собиралась быстро и без потери деталей.

## Контроль связности roadmap после Sprint 1
- Сразу после завершения Sprint 1 запускать отдельный `pwm-review` на связность roadmap:
  - соответствие текущей реализации целям Sprint 2+,
  - проверка, что не возник скрытый архитектурный дрейф,
  - корректировка backlog с учётом фактических нюансов реализации.
- Выход: короткий review-отчёт с корректировками следующих спринтов (если нужны).

## Базовые артефакты перед стартом Sprint 1
- Зафиксировать спецификационную ось (уже синхронизирована):
  - [docs/WHITE_SPEC_v0.md](p:/opt/docker/PWM-cryptocurrency/docs/WHITE_SPEC_v0.md)
  - [docs/rfc/3-cross-domain-roaming.md](p:/opt/docker/PWM-cryptocurrency/docs/rfc/3-cross-domain-roaming.md)
  - [docs/rfc/7-tx-and-state-model.md](p:/opt/docker/PWM-cryptocurrency/docs/rfc/7-tx-and-state-model.md)
  - [docs/rfc/6-policy-engine.md](p:/opt/docker/PWM-cryptocurrency/docs/rfc/6-policy-engine.md)
  - [docs/DEPEDENCY_GRAPH.md](p:/opt/docker/PWM-cryptocurrency/docs/DEPEDENCY_GRAPH.md)
  - [docs/WHITEPAPER_COVERAGE_MATRIX.md](p:/opt/docker/PWM-cryptocurrency/docs/WHITEPAPER_COVERAGE_MATRIX.md)
- Принцип: routing определяется протокольно по `domain_hi(sender/receiver)`, не route-флагами клиента.

## Sprint 1 (Week 1-2): Two-Shard Runtime Foundation
- **Цель:** поднять 2 независимых shard runtime с account-core совместимостью.
- **Scope:**
  - shard-aware конфиг/инициализация ноды,
  - отдельные state/storage namespace per shard,
  - базовая маршрутизация локальных tx внутри shard,
  - единый demo-скрипт запуска 2 shard.
- **Файлы/модули (ориентир):**
  - [crates/pwmd/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/main.rs)
  - [crates/pwmd/src/lib.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/lib.rs)
  - [docs/tester-guide-devnet-smoke.md](p:/opt/docker/PWM-cryptocurrency/docs/tester-guide-devnet-smoke.md)
- **Demo-ready output:** оператор запускает Shard A/B и выполняет local tx на каждом отдельно.

## Sprint 2 (Week 3-4): Export Path (Source Shard)
- **Цель:** реализовать `EXPORT` как additive flow на source shard.
- **Scope:**
  - tx envelope/validation для export,
  - state transition: debit + export commitment,
  - deterministic export_id,
  - ошибки/диагностика для invalid export.
- **Файлы/модули:**
  - [crates/pwm-core/src/tx.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/tx.rs)
  - [crates/pwm-core/src/state.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/state.rs)
  - [crates/pwmd/src/lib.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/lib.rs)
- **Demo-ready output:** можно сформировать и зафиксировать экспорт на Shard A с валидным commitment.

## Sprint 3 (Week 5-6): Finality Proof Minimal Profile
- **Цель:** минимальный финалити-профиль для v1 testnet.
- **Scope:**
  - формирование и валидация finality certificate,
  - связка export -> certificate,
  - негативные кейсы weak/invalid proof.
- **Файлы/модули:**
  - [crates/pwmd/src/lib.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/lib.rs)
  - [crates/pwm-core/src/block.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/block.rs)
  - [docs/rfc/4-validators-and-finality.md](p:/opt/docker/PWM-cryptocurrency/docs/rfc/4-validators-and-finality.md)
- **Demo-ready output:** на source shard экспорт сопровождается проверяемым certificate.

## Sprint 4 (Week 7-8): Import Path + Replay Guard
- **Цель:** реализовать `IMPORT` на target shard с replay-защитой.
- **Scope:**
  - import validation against certificate,
  - `ImportedSet` (или эквивалент) и reject duplicate import,
  - credit recipient в target shard.
- **Файлы/модули:**
  - [crates/pwm-core/src/state.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/state.rs)
  - [crates/pwm-core/src/chain.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/chain.rs)
  - [crates/pwmd/src/lib.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/lib.rs)
- **Demo-ready output:** полный happy-path transfer A -> B + демонстрация duplicate import rejection.

## Sprint 5 (Week 9-10): Policy Baseline + Routing by domain_hi
- **Цель:** закрепить baseline policy и протокольный routing.
- **Scope:**
  - enforce `domain_hi` routing rule (`same` => local transfer, `different` => roaming required),
  - recipient policy reject (`reserve/witness/unknown`) в user flow,
  - единый error contract для policy failures.
- **Файлы/модули:**
  - [crates/pwm-core/src/types.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/types.rs)
  - [crates/pwm-core/src/state.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/state.rs)
  - [docs/rfc/6-policy-engine.md](p:/opt/docker/PWM-cryptocurrency/docs/rfc/6-policy-engine.md)
- **Demo-ready output:** демонстрация корректного авто-роутинга по доменам и policy reject кейсов.

## Sprint 6 (Week 11-12): `pwmd` Micro-Slice Optimization Conveyor (completed)
- **Цель:** снизить локальную сложность `crates/pwmd/src/lib.rs` через серию узких behavior-preserving micro-refactors без смены внешних контрактов.
- **Scope:**
  - transport/state helper extraction и DRY-pass по счётчикам, backoff, reconnect/status bookkeeping;
  - батчинг 3-4 micro-правок на slice с обязательными `pwm-coding -> pwm-testing -> pwm-review` gates;
  - строгая evidence policy: `scoped_diff_stat` фиксирует только product/tooling paths (`crates/**`, `tools/**`), без self-referential artifact шума;
  - подготовка почвы для последующей декомпозиции `lib.rs`, но без переноса кода между модулями.
- **Файлы/модули:**
  - [crates/pwmd/src/lib.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/lib.rs)
  - [tools/slice-artifacts.ps1](p:/opt/docker/PWM-cryptocurrency/tools/slice-artifacts.ps1)
  - [tools/slice-commit.ps1](p:/opt/docker/PWM-cryptocurrency/tools/slice-commit.ps1)
  - [docs/reviews/sprint-6-checklist.md](p:/opt/docker/PWM-cryptocurrency/docs/reviews/sprint-6-checklist.md)
  - [tasks/20260424-sprint6-optimization.json](p:/opt/docker/PWM-cryptocurrency/tasks/20260424-sprint6-optimization.json)
- **Demo-ready output:** зелёный `pwmd` regression suite (`cargo test -p pwmd`) после каждого slice и зафиксированный набор micro-refactor commits с review/test evidence.

## Sprint 7 (Week 13-14): Decompose Heavy `pwmd` `lib.rs` into Submodules
- **Цель:** аккуратно разрезать тяжёлый `crates/pwmd/src/lib.rs` на устойчивые private submodules, сохранив внешнее поведение и публичные зависимости.
- **Scope:**
  - стартовый deep review через `pwm-optimus`: карта responsibility zones, dependency graph, risky external touchpoints;
  - выделение субмодулей по естественным границам (`transport`, `http/api`, `snapshot/state`, `policy/tx guards`, `config/bootstrap`), с уточнением после review;
  - контроль и коррекция внешних зависимостей от `lib.rs`: imports, visibility, test access, public re-exports только при необходимости;
  - поэтапные slices с одним module-boundary move за раз и обязательным regression/review gate.
- **Файлы/модули:**
  - [crates/pwmd/src/lib.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/lib.rs)
  - [crates/pwmd/src/](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/)
  - [docs/DEPEDENCY_GRAPH.md](p:/opt/docker/PWM-cryptocurrency/docs/DEPEDENCY_GRAPH.md)
- **Demo-ready output:** `pwmd` собирается и проходит regression suite после декомпозиции; внешний API/CLI-facing поведение не изменено; есть карта новых модулей и зависимостей.

## Sprint 8 (Week 15-16): Burn-Quota Path (`marks_quota`) and Zero-Fee Baseline
- **Цель:** интегрировать agreed burn model для testnet.
- **Scope:**
  - `marks_quota` как burn-only ресурс,
  - `BURN_MARK` списывает quota, не `balance_pwm`,
  - baseline `fee=0` для mark-based flow,
  - cross-domain burn context source-only proof handling.
- **Файлы/модули:**
  - [crates/pwm-core/src/state.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/state.rs)
  - [crates/pwm-core/src/tx.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/tx.rs)
  - [docs/WHITE_SPEC_v0.md](p:/opt/docker/PWM-cryptocurrency/docs/WHITE_SPEC_v0.md)
- **Demo-ready output:** end-to-end burn demo (local + cross-domain context) по новой модели.

## Sprint 9 (Week 17-18): CLI/TUI Integration for Two-Shard Demo Ops
- **Цель:** сделать операционный UX для demo-команды.
- **Scope:**
  - CLI команды/параметры для export/import/burn quota flow,
  - TUI отображение shard context и history cross-shard операций,
  - demo scripts and operator checklists.
- **Файлы/модули:**
  - [crates/pwm-cli/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-cli/src/main.rs)
  - [crates/pwm-tui/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-tui/src/main.rs)
  - [docs/tester-guide-cli-tui-scenarios.md](p:/opt/docker/PWM-cryptocurrency/docs/tester-guide-cli-tui-scenarios.md)
- **Demo-ready output:** один оператор проходит scripted demo v1 testnet через CLI/TUI.

## Sprint 10 (Week 19-20): Hardening and MVP v1 Testnet Cut
- **Цель:** стабилизировать и зафиксировать v1 MVP срез.
- **Scope:**
  - reliability pass (negative cases, restart/recovery behaviors),
  - conformance checklist и release notes,
  - freeze API/errors for MVP baseline.
- **Файлы/модули:**
  - [docs/PHASE1_RELEASE_SUMMARY.md](p:/opt/docker/PWM-cryptocurrency/docs/PHASE1_RELEASE_SUMMARY.md)
  - [docs/reviews/v1-testnet-decision-options-20260423.md](p:/opt/docker/PWM-cryptocurrency/docs/reviews/v1-testnet-decision-options-20260423.md)
  - [docs/WHITEPAPER_COVERAGE_MATRIX.md](p:/opt/docker/PWM-cryptocurrency/docs/WHITEPAPER_COVERAGE_MATRIX.md)
- **Demo-ready output:** стабильный внутренний v1 testnet demo build + go/no-go документ.

## Sprint 11 (Week 21-22): DomainHi Migration + Relay-by-Default
- **Цель:** упразднить fixed two-shard модель (`A/B`) как primary runtime-конфигурацию и перейти на `domain_hi`-центричный режим с дефолтом `relay`.
- **Scope:**
  - default runtime mode: `relay` (без shard-enforced validation);
  - shard-support включается только при явной domain-конфигурации (`cluster_domain_hi`/mode);
  - операторский контракт без legacy `--shard` / `state/shard-*` (domain-first + neutral baseline);
  - storage namespace для shard-capable режима строится по `domain_hi` (`domain-hi-0xNN`);
  - обновление tx-policy guards: shard-enforced правила применяются только в соответствующем режиме;
  - синхронизация docs/operator guides и acceptance матрицы под новую модель.
- **Файлы/модули:**
  - [crates/pwmd/src/](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/)
  - [crates/pwm-cli/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-cli/src/main.rs)
  - [crates/pwm-tui/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-tui/src/main.rs)
  - [docs/reviews/](p:/opt/docker/PWM-cryptocurrency/docs/reviews/)
- **Demo-ready output:** `pwmd` стартует в relay-default; domain_hi-mode включает shard-enforced path; regression/conformance pack зелёный.

## Sprint 12 (Week 23-24): Final Optimization Sprint after `pwm-optimus` Review
- **Цель:** выполнить финальный, заранее ограниченный optimization pass после стабилизации domain_hi migration (Sprint 11).
- **Scope:**
  - фиксированный объём: 8 slices total (`0/8 ... 7/8`);
  - Slice 0: deep review через `pwm-optimus`, freeze backlog ровно на 6 execution slices и explicit out-of-scope list;
  - Slices 1-6: batched micro-refactor execution, только по утверждённому shortlist;
  - Slice 7: wrap-up, consolidated evidence, residual-risk review и post-MVP handoff;
  - фокус на hotspots, выявленных review: duplication, private helper boundaries, avoidable allocation/copy paths, test readability;
  - без новых feature semantics, без изменения API/errors/tx guards, без расширения роутов.
- **Файлы/модули:**
  - [crates/pwmd/src/](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/)
  - [crates/pwm-core/src/](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/)
  - [docs/reviews/](p:/opt/docker/PWM-cryptocurrency/docs/reviews/)
- **Demo-ready output:** итоговый optimization report, зелёный regression/conformance pack, и зафиксированный handoff/backlog на пост-MVP hardening.

## Sprint 13 (Week 25-26): Inter-Shard MVP Cut (EXPORT/IMPORT)
- **Цель:** реализовать минимально рабочий межшардовый путь `EXPORT/IMPORT` для MVP-testnet без расширения scope в advanced policy.
- **Scope:**
  - в `pwm-core` добавить tx-path для `EXPORT/IMPORT`, детерминированный `export_id` и replay-защиту (`ImportedSet` или эквивалент) с persistence в snapshot;
  - в `pwmd` добавить runtime/API wiring для export/import submit и сохранить явный guard: cross-domain `TRANSFER` -> маршрут через `EXPORT/IMPORT`;
  - в `pwm-cli` и `pwm-tui` добавить минимальные операторские действия/подсказки для export/import flow и согласованный UX ошибок;
  - зафиксировать обязательный e2e контур на 2 нодах (пример: CY -> DO) с негативными кейсами `duplicate import` и `invalid proof`.
- **Slices (fixed 8):**
  - Slice 0: design freeze + sprint-13 checklist + out-of-scope фиксация.
  - Slice 1: `pwm-core` EXPORT + `export_id` + unit tests.
  - Slice 2: `pwm-core` IMPORT + replay guard + snapshot restore tests.
  - Slice 3: `pwmd` API/runtime wiring для export/import + status/error contract.
  - Slice 4: `pwm-cli` minimal happy-path команды/операторские подсказки.
  - Slice 5: `pwm-tui` minimal UX для межшардового шага и статусов.
  - Slice 6: e2e smoke 2-node + negative suite (duplicate/invalid proof).
  - Slice 7: stabilization + consolidated test/review closeout.
- **Acceptance criteria:**
  - cross-domain `TRANSFER` не является рабочим local-path и даёт согласованный маршрут через `EXPORT/IMPORT`;
  - `EXPORT` фиксирует источник списания и детерминированный `export_id`;
  - `IMPORT` с валидным материалом зачисляет на target ровно один раз;
  - повторный `IMPORT` того же `export_id` стабильно отклоняется;
  - replay-защита сохраняется после рестарта/загрузки snapshot;
  - CLI/TUI/pwmd имеют согласованные операторские сообщения и воспроизводимый demo runbook.
- **Файлы/модули:**
  - [crates/pwm-core/src/tx.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/tx.rs)
  - [crates/pwm-core/src/state.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/state.rs)
  - [crates/pwmd/src/](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/)
  - [crates/pwm-cli/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-cli/src/main.rs)
  - [crates/pwm-tui/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-tui/src/main.rs)
  - [docs/reviews/](p:/opt/docker/PWM-cryptocurrency/docs/reviews/)
- **Demo-ready output:** оператор воспроизводит полный базовый межшардовый сценарий `EXPORT -> IMPORT` с доказуемой idempotency и негативными проверками.

## Sprint 14 (Week 27-28): Multi-Address Wallet (`schema_version` 3)
- **Цель:** эволюция файла кошелька до **multi-address** (несколько owned веток `m/0/N` в одном файле), без смешения с `address_book` (контакты / allow-list).
- **Spec-first:** до кодинга — аудит полей и RFC; номер **RFC 10** ([docs/rfc/10-wallet-file-format-v3.md](p:/opt/docker/PWM-cryptocurrency/docs/rfc/10-wallet-file-format-v3.md)) — `8` занят shard-runtime RFC.
- **Конвейер на код-слайсы:** каждый слайс с правками кода проходит `pwm-coding` → `pwm-testing` → `pwm-review` (см. [docs/reviews/sprint-14-checklist.md](p:/opt/docker/PWM-cryptocurrency/docs/reviews/sprint-14-checklist.md)).
- **Scope:**
  - аудит полей v2/v3 и семантика `created_at_unix_sec` / per-account времени — [docs/reviews/sprint-14-wallet-schema-audit.md](p:/opt/docker/PWM-cryptocurrency/docs/reviews/sprint-14-wallet-schema-audit.md);
  - нормативный YAML: **4 пробела на уровень вложенности**; owned в `accounts[]`; `id_pretty` вместо `account_id_human` на витрине v3 ([docs/CHANGELOG.md](p:/opt/docker/PWM-cryptocurrency/docs/CHANGELOG.md));
  - encrypted payload MVP **A**: в блобе master seed, ключи per-account деривировать при unlock;
  - `pwm-cli`: `wallet account list|add|use` (имена финализировать в RFC);
  - `pwm-tui`: **левая панель** — все адреса из `accounts[]`, выделение active;
  - миграция v2→v3 (явная команда или controlled auto — в RFC/ревью).
- **Slices (ориентир):**
  - Slice 0: schema audit + RFC 10 + changelog + roadmap (доки).
  - Slice 1: `pwm-core` + `pwm-cli` структуры, load, инварианты, миграция.
  - Slice 2: CLI операторские команды.
  - Slice 3: TUI левая панель + переключение active.
  - Slice 4: стабилизация, demo runbook, closeout review.
- **Файлы/модули (ориентир):**
  - [crates/pwm-cli/src/wallet.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-cli/src/wallet.rs)
  - [crates/pwm-cli/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-cli/src/main.rs)
  - [crates/pwm-core/src/wallet_read.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/wallet_read.rs)
  - [crates/pwm-tui/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-tui/src/main.rs)
  - [docs/rfc/10-wallet-file-format-v3.md](p:/opt/docker/PWM-cryptocurrency/docs/rfc/10-wallet-file-format-v3.md)
- **Demo-ready output:** один wallet v3 с двумя `accounts`, переключение `use`, успешный `tx-send` от каждого; TUI показывает оба адреса в левой панели.

## Sprint 15 (Week 29-30): Cross-Shard Consistency + State Storage Evolution
- **Цель:** закрыть системные нестыковки межшардовых переводов/балансов и заложить масштабируемый слой хранения состояния для runtime/observability.
- **Scope A: Cross-shard consistency hardening**
  - довести до прозрачного UX полный путь `EXPORT -> handoff/provenance -> IMPORT`, чтобы оператор ясно видел, где именно застревает перенос;
  - устранить двусмысленность foreign balance visibility (локальный view vs authoritative home-shard truth);
  - добавить source-side readiness preflight перед export (минимум: target recipient/init/import readiness contract);
  - синхронизировать поведение TUI/CLI/address book/history под реальный протокол (без скрытых фильтров и stale маркеров).
- **Scope B: Genesis/bootstrap consistency**
  - закрепить guardrails для одинакового genesis bundle/hash на всех подключаемых шардах;
  - добавить status/diagnostics, чтобы mismatch был очевиден до первых tx;
  - формализовать operator recovery path для expired/failed roaming intents.
- **Scope C: Optional state storage backend**
  - спроектировать и начать реализацию backend-абстракции snapshot/state: `JsonFile` (baseline) + `Db` (optional);
  - первичный кандидат DB backend: **ClickHouse** в Docker (высокая пропускная способность, удобство для будущего blockchain explorer);
  - провести сравнение альтернатив (например, PostgreSQL/TimescaleDB, RocksDB + indexer, Kafka+CH pipeline) с упором на write throughput, replay determinism, ops complexity.
- **Slices (ориентир):**
  - Slice 0: architecture freeze (consistency model + storage decision matrix).
  - Slice 1: cross-shard UX/protocol bugfix bundle (acceptance based on e2e CY->DO).
  - Slice 2: authoritative/foreign balance semantics in API/TUI.
  - Slice 3: genesis/hash guardrails and startup diagnostics.
  - Slice 3.1: TUI пошаговый cross-shard wizard (этапы preflight/export/handoff/import с явными статусами для тестера).
  - Slice 4: snapshot backend abstraction (`JsonFile` + interface for `Db`).
  - Slice 5: ClickHouse prototype backend in Docker + persistence smoke.
  - Slice 6: replay/consistency tests across backends.
  - Slice 6b (performance, желательно после Slice 4): ускорение загрузки блокчейна — **checkpoint-снимки** (материализованное состояние + якорь высоты/hash) и **lazy** подгрузка/материализация исторических блоков там, где полный replay не нужен сразу; сохранить детерминизм replay для validator-пути и совместимость с абстракцией snapshot/store из Slice 4.
  - Slice 7: closeout review, performance notes, explorer-readiness backlog.
  - **S15-S7 closeout:** зафиксировано в [docs/reviews/sprint-15-S7-closeout.md](p:/opt/docker/PWM-cryptocurrency/docs/reviews/sprint-15-S7-closeout.md) (decision gate, риски R1–R5, carry-over).
- **Acceptance criteria:**
  - перенос монет между шардами воспроизводим в e2e без скрытых операторских ловушек;
  - балансовые статусы foreign-адресов не вводят в заблуждение;
  - подключаемые шарды валидируют genesis-consistency до приема пользовательских tx;
  - snapshot persistence работает через JSON baseline и optional DB backend без потери replay deterministic behavior.
- **Файлы/модули (ориентир):**
  - [crates/pwmd/src/](p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/)
  - [crates/pwm-core/src/state.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-core/src/state.rs)
  - [crates/pwm-cli/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-cli/src/main.rs)
  - [crates/pwm-tui/src/main.rs](p:/opt/docker/PWM-cryptocurrency/crates/pwm-tui/src/main.rs)
  - [docs/reviews/sprint-14-slice31-genesis-balance-consistency-review.md](p:/opt/docker/PWM-cryptocurrency/docs/reviews/sprint-14-slice31-genesis-balance-consistency-review.md)
  - [docs/plans/sprint-15-architecture-genesis-consistency-and-db-snapshots.md](p:/opt/docker/PWM-cryptocurrency/docs/plans/sprint-15-architecture-genesis-consistency-and-db-snapshots.md)

## Межспринтовые гейты качества (каждый спринт)
- **Spec Gate:** `pwm-review` подтверждает отсутствие новых противоречий между WHITE/RFC/Matrix.
- **Demo Gate:** есть воспроизводимый запуск + 1 happy-path + 2 negative сценария.
- **Regression Gate:** существующие v0 local flows остаются работоспособными.
- **Manual Visual Gate (условный):**
  - если автотесты покрывают сценарий полно и стабильны, визуальный контроль владельца опционален;
  - визуальный/manual контроль подключается при сомнениях по стабильности UX/TUI (особенно производительность, лаги, деградация интерактивности).

## Риски и контрмеры
- Риск: скрытый дрейф к UTXO-ядру -> контрмера: explicit check в review gate.
- Риск: усложнение policy раньше времени -> контрмера: держать advanced policy за feature hooks.
- Риск: рост поверхности тестирования -> контрмера: fixed acceptance pack per sprint.

## Декомпозиция на таски
- Для каждого спринта открыть минимум 2 тикета:
  - `pwm-coding`: реализация + docs update,
  - `pwm-testing`: regression + targeted negatives + demo report.
- Сначала запускать coding тикет, затем testing тикет в том же спринте.
- В начале каждого спринта добавлять pre-task:
  - `pwm-coding`: manual sprint-checklist generation (обязательный шаг перед основными задачами).
