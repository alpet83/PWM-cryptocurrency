# PWM-cryptocurrency

**Языки:** [English — README.md](README.md) · Русский (этот файл)

PWM — **нативная криптовалюта с моделью matrixchain** (см. толкование v0 в [MATRIXCHAIN_SPEC_v0.md](docs/MATRIXCHAIN_SPEC_v0.md): «матрица» идентичности, одномерная цепь в коде, операционные geo-шарды). Реализация на **Rust**: PoA-нода (`pwmd`), CLI `pwm`, `pwm-tui`, доменно-первый операторский контур. Проект **утилитарный**, ориентирован **прежде всего на IT-безопасность** (аудируемые операции, явные границы доверия, шардинг и roaming), а не на розницу или универсальный DeFi.

![Интерфейс оператора pwm-tui (демо)](tui-demo-screenshot.png)

## Текущий статус (MVP v3 foundation закрыт)

- **Foundation-этап MVP v3 для public devnet закрыт** (спринты V3-1..V3-4).
- **Есть чистый public-devnet quickstart**: из clean clone документирован детерминированный demo genesis path с проверкой premine (`21_000_000_000 PWM` = `21_000_000_000_000_000 raw`).
- **`/v1` API freeze skeleton зафиксирован** в `docs/api-v1.md` как baseline публичного контракта.
- **Схема Epoch Snapshot и replay-determinism gate зафиксированы** как V3-база доверия для загрузки/реплея состояния.
- **ADR-пакет опубликован** в `docs/adr/` и задаёт архитектурные границы foundation-слоя.
- **Runtime log-control RPC относится к operator/debug поверхности** и явно не входит в стабильный public API.

**Что уже работает (после закрытия V3 foundation):**

- **Интегрированный public-devnet smoke покрывает read API:** `GET /v1/status`, `GET /v1/head`, `GET /v1/accounts`, `GET /v1/account/:id`.
- **`POST /v1/tx` остаётся в `/v1` API skeleton**, но не входил в интегрированный smoke Sprint V3-4 (отдельный smoke можно добить follow-up слайсом).
- **Operator runtime log-control endpoints** (`/v1/operator/log/override`) доступны как operator/debug surface, а не как стабильный клиентский public API.

- **Два spec-level geo-шарда** — два процесса `pwmd` с разным `domain_hi` (например `0x10` / `0x20`), отдельный `--state-root` у каждого и **отработанный** happy path для связи по **реальному транспорту** с взаимными `--transport-peer-seed`.
- **Внутришардовые** переводы и обычный жизненный цикл счёта (`INIT`, `TRANSFER`, задел под стейкинг) через RPC, CLI и TUI.
- **Единый баланс `marks` и `BURN_MARK`:** текущий MVP v2 использует `Account.marks` как единственный счётчик марок. Начисление марок на каждом `Chain::seal` удалено; usable marks приходят через genesis/claim-контур, а `BURN_MARK` списывает тот же счётчик. CLI `tx-burn-mark --amount N [--purpose P]` перед отправкой показывает текущие marks; в TUI есть колонка `Marks` и форма сжигания по F5 с заголовком `Current marks`; `--purpose` поддерживает плейсхолдеры `{utc_time}` / `{utc_timestamp}`.
- **Межшардовое перемещение стоимости** по явной цепочке **EXPORT → relay/handoff → IMPORT**: source и target согласуются через доверие к настроенным seed; `tx-send` / TUI и `tx-export` / `tx-import` соответствуют [контракту Sprint 13 как реализовано](docs/rfc/9-crossdomain-roaming.md). Подробнее: [ROAMING-SAMPLE.md](docs/ROAMING-SAMPLE.md), [ROAMING_COMPLETION.md](docs/ROAMING_COMPLETION.md).
- **Федеративный мост** (включая отказ в доверии и пути восстановления) — часть рантайм-контракта; см. [WHITE_SPEC_v0.md](docs/WHITE_SPEC_v0.md) §7.5 и операторские заметки в [docs/pwmd.md](docs/pwmd.md).
- **Персистентность:** основной путь — **JsonFile** (сводный `pwm-data.json`, `epochs/`, манифест; trust-default vs аудит-реплей). Опционально **`pwmd` может писать снапшоты в ClickHouse** (feature `clickhouse-snapshot`); семантика загрузки отличается от JsonFile (см. раздел **Backend хранения** ниже и [guide-node-storage-and-snapshot.md](docs/guide-node-storage-and-snapshot.md)).

Режим relay-baseline (neutral) остаётся для экспериментов; **демонстрации «как в проде» идут с явным доменным конфигом** (`--domain-hi`, `--cluster-id`, `--node-id`).

## Раскладка хранилища

- По умолчанию `--state-root` — `state`. Итоговый путь задаётся идентичностью рантайма:
  - **Neutral по умолчанию:** `state/neutral/<listen-addr>/pwm-data.json` (`:` → `+` в теге).
  - **Явный домен:** `state/domain-hi-0xNN/pwm-data.json`.
  - **Переопределение:** `--data-file <ABS_PATH>` (в скриптах лучше абсолютный путь).
- На диске могли остаться старые каталоги `state/shard-a` / `state/shard-b` от билдов до domain-first; текущий `pwmd` их не использует.
- Рядом со сводным файлом JsonFile хранит **`epochs/`** (`block_e*.json` в формате JSONL, `pwm-epochs-manifest.json`). При обычном старте доверяется чекпоинту и «хвосту»; полный реплей для аудита — `--snapshot-verify-chain` или `PWM_SNAPSHOT_VERIFY_CHAIN`. Детали и troubleshooting: [guide-node-storage-and-snapshot.md](docs/guide-node-storage-and-snapshot.md).
- Если остался старый плоский `state/pwm-data.json` от прежних схем — перенесите файл в нужное пространство имён или укажите `--data-file` явно.

## Backend хранения снапшотов (JsonFile и ClickHouse)

- **JsonFile** — путь по умолчанию: файлы под `--state-root`, рядом каталоги эпох, при необходимости полный реплей через `--snapshot-verify-chain` / `PWM_SNAPSHOT_VERIFY_CHAIN`.
- **ClickHouse** — опциональный backend: сборка `pwmd` с **`--features clickhouse-snapshot`**, выбор backend и HTTP endpoint через CLI / env (см. `pwmd --help` и [runbook-store-snapshots.md](docs/runbook-store-snapshots.md)). Загрузка из CH сейчас идёт по **full replay**; JsonFile и CH стоит рассматривать как разные операционные режимы (§ ClickHouse в [guide-node-storage-and-snapshot.md](docs/guide-node-storage-and-snapshot.md)).
- **Локальная проверка:** для быстрого подъёма ClickHouse удобно **Docker Desktop** (Windows/macOS) или Docker Engine (Linux) — пример compose: [`tools/docker/pwmd-clickhouse-compose.yaml`](tools/docker/pwmd-clickhouse-compose.yaml); подготовка схемы и операции: [runbook-store-snapshots.md](docs/runbook-store-snapshots.md).

## Быстрый старт оператора (domain-first)

Семантика доменов и словарь: [DOMAINS.md](docs/DOMAINS.md).

**Одна нода (явный домен):**

```powershell
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3030 --network-id devnet --domain-hi 0x10 --cluster-id local-cluster-a --node-id local-node-a
```

**Два шарда (два терминала)** — у процессов должны различаться **`--listen`** и **`--state-root`**. Готовые сценарии: `tools/demo-two-shard.ps1` (PowerShell) или `tools/demo-two-shard.sh` (bash). Минимальная ручная пара:

```powershell
# Шард A — domain 0x10, порт 3030
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3030 --state-root state-a --network-id devnet --domain-hi 0x10 --cluster-id local-cluster-a --node-id local-node-a

# Шард B — domain 0x20, порт 3031
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3031 --state-root state-b --network-id devnet --domain-hi 0x20 --cluster-id local-cluster-b --node-id local-node-b
```

**Связь пиров (real transport + seeds)** — перезапустите обе ноды с `--transport-real` и взаимными `--transport-peer-seed`:

```powershell
# A указывает seed на B
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3030 --state-root state-a --network-id devnet --domain-hi 0x10 --cluster-id local-cluster-a --node-id local-node-a --transport-real --transport-peer-seed 127.0.0.1:3031

# B указывает seed на A
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3031 --state-root state-b --network-id devnet --domain-hi 0x20 --cluster-id local-cluster-b --node-id local-node-b --transport-real --transport-peer-seed 127.0.0.1:3030
```

**Дымовые проверки:**

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/dev/peers"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/dev/peers"
```

Ожидается `phase=ready`, видимость пира на обеих сторонах и пространства имён `domain-hi-0x10` / `domain-hi-0x20`. Для **`pwm`** / **`pwm-tui`** задавайте нужную ноду через `--rpc` или `PWM_RPC`; кросс-шард и завершение импорта — в [tester-guide-cli-tui-scenarios.md](docs/tester-guide-cli-tui-scenarios.md) (§5–11).

## Сеть: охват и ограничения

- Топология — **PoA dev со списком seed**: явные peer-seeds, не публичный discovery mesh.
- **Межшардовые потоки реализованы** как выше; ограничения (например отсутствие протокольного **escrow на EXPORT до финализации IMPORT** — пока только дизайн в RFC) см. в [WHITE_SPEC_v0.md](docs/WHITE_SPEC_v0.md) и [rfc/9-crossdomain-roaming.md](docs/rfc/9-crossdomain-roaming.md).

## Ключевые документы

- README (English): [README.md](README.md)
- План foundation MVP v3: `docs/plans/mvp_v3.md`
- API freeze skeleton (`/v1/*`): `docs/api-v1.md`
- Quickstart public devnet: `docs/runbooks/demo-devnet-quickstart.md`
- Индекс ADR-пакета: `docs/adr/README.md`
- RFC runtime log-control RPC (operator/debug): `docs/rfc/17-runtime-log-control-rpc.md`
- Черновик whitepaper: `DRAFT_WHITEPAPER.md`
- Whitepaper (RU): `DRAFT_WHITEPAPER-ru.md`
- Matrixchain (термин и v0): `docs/MATRIXCHAIN_SPEC_v0.md`
- White spec: `docs/WHITE_SPEC_v0.md`
- Geo-sharding (простое объяснение): `docs/GEO-SHARDING-EXPLANATION.md`
- Runbook cross-domain roaming: `docs/ROAMING-SAMPLE.md`
- Заметки по завершению / стабилизации roaming: `docs/ROAMING_COMPLETION.md`
- Чеклист MVP: `docs/MVP-checklist.md`
- Dev smoke: `docs/tester-guide-devnet-smoke.md`
- Хранилище ноды и режимы снапшота: `docs/guide-node-storage-and-snapshot.md`
- Runbook снапшотов ClickHouse: `docs/runbook-store-snapshots.md`
- CLI/TUI: два шарда и кросс-шард: `docs/tester-guide-cli-tui-scenarios.md`
- Словарь доменных кластеров: `docs/DOMAINS.md`
- Phase 1 checklist: `docs/PHASE1_CHECKLIST.md`
- Phase 1 release summary: `docs/PHASE1_RELEASE_SUMMARY.md`
