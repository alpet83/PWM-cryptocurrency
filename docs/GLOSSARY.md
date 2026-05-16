# Глоссарий PWM (для неспециалистов)

Этот файл объясняет слова и сокращения, которые встречаются в документации по кластерному согласованию клонов валидатора (RFC 16), в логах узла `pwmd` и в интеграционных тестах транспорта. Рассчитан на операторов, будущих контрибьюторов без глубокого Rust-фона и на читателей, которым незнаком жаргон распределённых систем.

**Как пользоваться:** сначала загляните в **тематические блоки** ниже — там связанные понятия собраны рядом. Если ищете конкретное слово, откройте **алфавитный указатель** в конце файла и перейдите по ссылке. Нормативные детали протокола всегда в [RFC 16](rfc/16-validator-clone-attestation.md); операционный чеклист спринта — в [чеклисте V2-9](reviews/20260509-v2-9-rfc16-sprint-checklist.md).

## Как читать этот документ

- **По темам** — разделы сгруппированы по смыслу (кластер, сеть, ошибки, тесты). Удобно, когда вы разбираете один сценарий целиком (например, «почему не seal-ится блок» или «почему follower отстаёт»).
- **По алфавиту** — внизу страницы указатель **латиница (A–Z)** и **кириллица (А–Я)** с якорными ссылками на определения в тематических разделах.
- **Связанные документы:** [RFC 16 — терминология](rfc/16-validator-clone-attestation.md) (раздел 2), [обзор слайсов B+C](reviews/20260510-v2-9-slice-bc-review.md), [wave notes Slice B](reviews/20260510-v2-9-slice-b-wave-notes.md), [wave notes Slice C](reviews/20260510-v2-9-slice-c-wave-notes.md).

---

## Кластер, кворум и «гейт» на seal {#thema-cluster}

### Attester (аттестер) {#term-attester}

- **Простыми словами:** «второй (или третий) экземпляр» того же валидатора, который **не собирает** свой альтернативный блок, а **проверяет и подписывает согласие** с предложением лидера, если оно проходит локальные правила.
- **Зачем в проекте:** в профиле кластера (Variant A, RFC 16) без достаточного числа таких подписей **seal** следующего блока не выполняется — защита от одиночной ошибочной или злонамеренной ноды-лидера.
- **Где встречается:** конфиг ролей (`ClusterRole::Attester`), доверенные пиры с `cluster_attest_enabled`, wire-сообщения `ClusterAttest`, обзоры в [slice-bc-review](reviews/20260510-v2-9-slice-bc-review.md).
- **Частая путаница:** attester **не** «голосует за свой mempool» как отдельный конкурирующий производитель блока в базовом Variant A — он проверяет **кандидата лидера** (см. RFC 16, раздел 2.1).

### Proposer (пропоузер / лидер раунда) {#term-proposer}

- **Простыми словами:** узел, который **формирует кандидат** на следующую высоту и рассылает предложение коллегам по кластеру.
- **Зачем в проекте:** только один такой коммитер обычно выполняет финальный `seal` после кворума; остальные клоны не должны seal-ить тот же `(высота, раунд)` (RFC 16, раздел 8).
- **Где встречается:** `cluster_2of2_gate_ok`, `cluster_2of3_gate_wire` в `crates/pwmd/src/transport/tests/production.rs`; поле `ClusterRole::Proposer`.

### k-of-n (кворум «k из n») {#term-k-of-n}

- **Простыми словами:** из **n** настроенных участников кластера для раунда нужно **k** независимых подтверждений (аттестаций), чтобы считать кворум достигнутым; конкретные **k** и **n** задаёт оператор в пределах поддерживаемых профилей (в спринте V2-9 — до 3 узлов, дорожки 2-of-2 и 2-of-3).
- **Зачем в проекте:** политика отказоустойчивости: потеря одного attester при 2-of-3 **не** даёт «тихо» обойти требование кворума (см. тест `cluster_2of3_one_ack_stuck`).
- **Где встречается:** `app.cluster_cfg.quorum_k`, `quorum_n`; RFC 16 раздел 7; чеклист [V2-9](reviews/20260509-v2-9-rfc16-sprint-checklist.md).

### `run_cluster_gate` (кластерный «гейт» перед seal) {#term-run-cluster-gate}

- **Простыми словами:** функция, которая **перед** локальным `Chain::seal` проверяет: включён ли кластер, есть ли состояние раунда, совпадает ли пропоузер с членами, и набралось ли **k** аттестаций от **других** членов (пропоузер в подсчёт k **не входит** — комментарий в коде ссылается на RFC 16 раздел 7). При нехватке ACK и истечении `attest_timeout_ms` в лог пишется причина **`quorum_timeout`**.
- **Зачем в проекте:** единая точка «можно ли печатать блок», когда включён профиль кластерной аттестации.
- **Где встречается:** `crates/pwmd/src/lifecycle.rs` (функция `run_cluster_gate` и цикл `spawn_seal_loop`).

### Seal (сейл / запечатывание блока) {#term-seal}

- **Простыми словами:** переход цепочки к следующему блоку с применением выбранных транзакций — «официальное» продвижение локального вида блокчейна.
- **Зачем в проекте:** в тестах после успешного гейта проверяют рост `tip_h` и смену `tip_hash` — признак, что блок действительно записан.

### `record_cluster_propose_originated` и «зеркало» propose {#term-record-cluster-propose-originated}

- **Простыми словами:** запись сведений о раунде в локальное состояние (`HandshakeState.cluster_attest`), синхронная с отправкой или приёмом **`ClusterPropose`**. Когда пропоузер **пишет** propose в исходящий TCP-поток, **входящий** обработчик на том же узле это сообщение **не видит**; зеркало нужно, чтобы **`run_cluster_gate`** на пропоузере видел ту же привязку раунда, что и аттестеры.
- **Зачем в проекте:** в прод-пути вызывается из **`send_cluster_prop`** после сборки **`ClusterProposeWire`**; в интеграционных тестах **`production.rs`** ту же запись иногда делают **явно** после ручной отправки фрейма — для прозрачности harness.
- **Где встречается:** `crates/pwmd/src/transport/peer_session/mod.rs` (**`record_cluster_propose_originated`**, **`send_cluster_prop`**), тест **`cluster_prop_mirror_send`**; вызовы в **`production.rs`**.
- **Частая путаница:** зеркало и кворум — разные вещи; аренда seal (S2) тоже отдельная ось (RFC 16 раздел 8.1).

### `send_cluster_prop` (исходящий ClusterPropose с пропоузера) {#term-send-cluster-prop}

- **Простыми словами:** перед записью фрейма проверяются роль пропоузера, состав кластера и то, что удалённый hello помечен как **аттестер**; затем формируется propose на **`tip_h+1`** и отправляется по уже установленной peer-сессии.
- **Где встречается:** после handshake в **`inbound.rs`** и на каждом цикле heartbeat для входящих пиров; в исходящем seed-пути — после **`initial_exchange`** и на каждом шаге **`steady_session`** (рядом с heartbeat и sync).
- **Тестовый жаргон:** **`cluster_prop_mirror_send`** в модуле **`peer_session`** проверяет зеркало состояния после вызова.

### Авто-аттестация по входящему `ClusterPropose` {#term-auto-cluster-attest}

- **Простыми словами:** если локальный узел — **аттестер**, при получении доверенного **`ClusterPropose`** узел **сам** строит **`ClusterAttest`** и **сразу отправляет** его обратно по той же TCP-сессии (входящий акцептор и устойчивый seed-цикл обрабатывают это одинаково).
- **Зачем в проекте:** убирает ручной второй шаг в лабораторном сценарии «пропоузер ↔ аттестер» и приближает поведение к описанию Variant A по соседскому wire.
- **Где встречается:** **`route_cluster_stub`** + **`mk_cluster_attest`** в **`peer_session/mod.rs`**; запись ответа в **`inbound.rs`** и **`steady_session.rs`**. Юнит-тест **`cluster_prop_auto_ack`**.

### Аренда seal «в процессе» (`process-local`) {#term-process-local-lease}

- **Простыми словами:** режим хранения lease **внутри процесса** `pwmd`, без общего файла аренды между процессами на одной машине — удобно для коротких локальных прогонов двух окон PowerShell.
- **Зачем в проекте:** в CY-лабе скрипты **`cy-cluster-proposer.ps1`** и **`cy-cluster-attester.ps1`** передают **`--seal-lease-backend process-local`**, чтобы не ловить **`seal_lease_cas_failed`** из-за общего каталога файловой аренды (см. запись **2026-05-11** в **`issues-report.md`**).
- **Частая путаница:** это **не** замена кворума RFC 16 и **не** модель HA между хостами; для боевого failover нужен согласованный внешний источник истины по lease (раздел 8 в RFC остаётся про ортогональность S2 и кворума).

### Validator clone (клон валидатора) {#term-validator-clone}

- **Простыми словами:** отдельный процесс `pwmd`, работающий от **того же** идентичностного ключа валидатора (в том же наборе валидаторов), что и другие клоны.
- **Зачем в проекте:** кластер Variant A договаривается **внутри** множества клонов одной логической валидаторской роли, а не с произвольными соседями по сети.

### Cluster membership (состав кластера) {#term-membership}

- **Простыми словами:** **явно** заданный список узлов/экземпляров, которым разрешено участвовать в аттестации; «любой пир шарды» таким участником **не** является (RFC 16, раздел 4).
- **`node_instance_id` и `--node-instance-id`:** в handshake в сеть уходит **идентификатор экземпляра**; им заполняется список **`--cluster-members`**. Если его не задавать, `pwmd` сам генерирует строку вида **`node_id`-PID-момент_запуска** — при **каждом** рестарте она другая, и статический `cluster-members` перестаёт совпадать с пирами. Для долгих смоков и скриптов вроде `cy-cluster-*.ps1` на каждом участнике кворума задают один и тот же стабильный id (флаги или `PWM_NODE_INSTANCE_ID`), который перечислен в `cluster-members`.

### Relay pool vs active quorum set {#term-relay-pool}

- **Простыми словами:** **relay pool** — более широкое кольцо узлов для ретрансляции; **active quorum set** — кто **обязан** дать аттестации в данном раунде. Это разные оси; не путать с «кто имеет право seal» (lease S2).
- **Где встречается:** RFC 16 разделы 2 (таблица терминов), 7.1, 8.1; обзор в [slice-bc-review Addendum](reviews/20260510-v2-9-slice-bc-review.md).

### S2 (lease) и ортогональность кворуму {#term-s2-lease}

- **Простыми словами:** **lease** отвечает на вопрос «**кому можно** выполнять seal» (эксклюзивность, fencing); **кворум** — на вопрос «**достаточно ли** согласий клонов перед seal». Одно не заменяет другое (RFC 16 раздел 8.1).

---

## Сеть, синхронизация, same-shard {#thema-network}

### Same-shard (одна шарда) {#term-same-shard}

- **Простыми словами:** пир смотрит на **тот же** кусок сети/цепочки (shard), что и локальный узел; обмен блоками и tip-ами имеет смысл без кросс-шардовой маршрутизации.
- **Где встречается:** чеклист [V2-9 Slice C](reviews/20260509-v2-9-rfc16-sprint-checklist.md); тесты `blk_fetch_apply_ok`, `same_shard_follower_tcp_tip`.

### Source и follower (источник и догоняющий) {#term-source-follower}

- **Простыми словами:** **источник** — узел с более продвинутым или эталонным для теста tip; **follower** — узел, который должен **подтянуть** высоту и хеш через peer-транспорт.
- **Зачем в проекте:** сценарий «кластерная нода + внешняя follower-нода той же шарды» в приёмке Slice C: follower **без** ролей кластера догоняет tip (см. [slice-c-wave-notes](reviews/20260510-v2-9-slice-c-wave-notes.md)).

### `SyncTipAnnounce` {#term-sync-tip-announce}

- **Простыми словами:** сообщение по peer-wire, которым сосед сообщает «какой сейчас видимый конец цепочки» (высота и идентификатор tip), чтобы запустить сравнение и при необходимости catch-up.
- **Где встречается:** обработка в `sync_live::on_tip` (`crates/pwmd/src/transport/peer_session/sync_live.rs`).

### Peer-behind (сосед «сзади») {#term-peer-behind}

- **Простыми словами:** ситуация, когда **у пира анонсированная высота меньше**, чем **ваша локальная** — он ещё не догнал вас (или вы опережаете).
- **Зачем в проекте:** такой случай **не должен** считаться «разводкой мостов» на разных высотах. После исправления в `on_tip` при `head_h < local_h` выполняется ранний выход **`Ok(None)`** без сравнения хешей на несопоставимых высотах ([slice-c-wave-notes](reviews/20260510-v2-9-slice-c-wave-notes.md)).

### `tip_h` и `tip_hash` {#term-tip-h-tip-hash}

- **Простыми словами:** **`tip_h`** — номер высоты головы цепочки; **`tip_hash`** — дайджест (хеш) блока на этой высоте. Вместе задают «где мы сейчас» для сравнения с пиром.
- **Где встречается:** проверки в `same_shard_follower_tcp_tip`, `blk_fetch_apply_ok`, пост-gate assert в `production.rs`.

### Двухузловой soak (нагрузочное «вымачивание» двух узлов) {#term-two-node-soak}

- **Простыми словами:** это **интеграционный** тестовый режим, где **два процесса-ноды** подключены по **реальному TCP** (`spawn_peer_listener_loop`, `spawn_stateful_transport_loop`), поддерживают **двусторонний** обмен и **ограниченное по времени ожидание**, пока **отстающий** догонит **высоту и `tip_hash`** источника. Транспортные циклы (hello, heartbeat, sync) работают как в бою, а не через моки.

- **Чем отличается от «трёх узлов кластера + отдельный follower в одном тесте»:** сценарий **2-of-3** и три кластерных роли требуют **тройной** wire-топологии и сложнее по времени и стабильности; **двухузловой soak** специально проверяет **сходимость follower** к источнику с **включённым кластером на источнике** и **выключенным кластером на follower** в одном бинарном тесте — этого достаточно для строки чеклиста про convergence; тяжёлый «три кластера + четвёртый follower» остаётся **опциональным** и **не обязателен** для закрытия gate ([slice-bc-review Addendum](reviews/20260510-v2-9-slice-bc-review.md)).

- **Где встречается:** тест **`same_shard_follower_tcp_tip`** в `crates/pwmd/src/tests/transport_peer.rs`; описание в [slice-c-wave-notes](reviews/20260510-v2-9-slice-c-wave-notes.md).

### Wire (транспортный «по проводу») {#term-wire}

- **Простыми словами:** **реальные** байтовые сообщения протокола поверх установленной TCP-сессии между пирами, а не внутренний вызов функций в одном процессе.
- **Зачем в проекте:** приёмка «wire E2E» значит: `ClusterPropose` / `ClusterAttest` проходят через тот же путь, что и в эксплуатации ([slice-b-wave-notes](reviews/20260510-v2-9-slice-b-wave-notes.md)).

---

## Ошибки, причины no-seal и счётчики {#thema-errors}

### `quorum_timeout` {#term-quorum-timeout}

- **Простыми словами:** к моменту проверки так и **не набралось k** валидных аттестаций; раунд **истёк** по времени ожидания.
- **Зачем в проекте:** детерминированный no-seal; в логах и негативных тестах ожидается строка вида `seal_suppressed_by_cluster` с **`reason=quorum_timeout`** ([slice-b-wave-notes](reviews/20260510-v2-9-slice-b-wave-notes.md), `cluster_timeout_no_seal`).

### `binding_mismatch` {#term-binding-mismatch}

- **Простыми словами:** пришёл attest, но **подписанный объект голосования** не совпадает с тем, что ожидалось для кандидата (`vote_object` и связанные поля) — аттест отвергнут, кворум не достигнут.
- **Где встречается:** негатив `cluster_bind_mismatch_no_seal`; логи с `cluster attest dropped` и **`reason=binding_mismatch`**.

### `quorum_pending` {#term-quorum-pending}

- **Простыми словами:** ещё **рано** считать отказ окончательным — раунд есть, но ACK всё ещё не хватает и **таймаут ещё не прошёл** (отличается от окончательного `quorum_timeout`).
- **Где встречается:** [slice-b-wave-notes (наблюдаемость, негативы)](reviews/20260510-v2-9-slice-b-wave-notes.md).

### `sync_tip_divergence` / счётчик `sync_tip_disconnect_total` {#term-sync-tip-divergence}

- **Простыми словами:** обнаружено **подозрение на рассинхрон**: на одной высоте tip-состояния не сходятся так, что это трактуется как **TipDivergence**, и сессия может быть разорвана. Счётчик в коде — `TransportSnapshot::sync_tip_disconnect_total`; в **JSON** снапшота ключ остаётся **`sync_tip_divergence_disconnect_total`** (`serde(rename)`).
- **Зачем в проекте:** в `same_shard_follower_tcp_tip` проверяют, что счётчик **не растёт** при **нормальном** отставании follower — иначе ложные disconnect мешают догонять tip ([slice-c-wave-notes](reviews/20260510-v2-9-slice-c-wave-notes.md)).

### `invalid_proposal` (в контексте гейта) {#term-invalid-proposal}

- **Простыми словами:** нет корректного связывания полей предложения (пустой `vote_object` или `candidate_hash`, пропоузер не из членов и т. п.) — seal невозможен до исправления состояния раунда.
- **Где встречается:** ветки `run_cluster_gate` в `lifecycle.rs`; RFC 16 раздел 9 для нормативной картины.

---

## Тесты, harness, «тестовый жаргон» {#thema-tests}

### `production.rs` (тестовый модуль) {#term-production-rs}

- **Простыми словами:** файл `crates/pwmd/src/transport/tests/production.rs` — набор **интеграционных** тестов, которые поднимают реальные TCP-сокеты и гоняют **`process_inbound_socket`** / полный цикл сообщений.
- **Зачем в проекте:** gate-сценарии кластера (`cluster_2of2_gate_ok`, `cluster_2of3_gate_wire`, негативы no-seal) помечены как wire/harness-проверки в обзорах слайсов; юнит-тесты **`cluster_prop_auto_ack`** и **`cluster_prop_mirror_send`** живут в **`peer_session/mod.rs`** (cfg test).

### Интеграционный тест {#term-integration-test}

- **Простыми словами:** тест, который склеивает **несколько подсистем** (транспорт, handshake, часть lifecycle), часто с сетевым I/O на `127.0.0.1`.
- **Частая путаница:** **не** то же самое, что модульные тесты одной функции без сокетов — дороже и ближе к «как в поле».

### `xfer_state_bidir_stable` **(тестовый жаргон)** {#term-xfer-state-bidir-stable}

- **Простыми словами:** два узла, взаимные `peer_seeds`, **двусторонние** stateful-транспортные циклы; проверка, что за ~секунды **нет лишних disconnect** и **нет churn** по счётчикам сессий.
- **Где встречается:** `crates/pwmd/src/tests/transport_peer.rs` — **имя теста**, не операторская команда.

### `tip_behind_no_divergence` **(тестовый жаргон)** {#term-tip-behind-no-divergence}

- **Простыми словами:** регрессия для ветки **peer-behind**: локальная цепочка уже выше, приходит announce с genesis-уровнем — сессия **не должна** рваться из-за ложного расхождения хешей ([slice-bc-review Addendum](reviews/20260510-v2-9-slice-bc-review.md)).
- **Где встречается:** модуль `transport::peer_session` tests; команда в [slice-c-wave-notes](reviews/20260510-v2-9-slice-c-wave-notes.md).

### Capability bits / `PWM_PROTOCOL_VERSION` {#term-capability-version}

- **Простыми словами:** согласование возможностей на handshake; обязательные поля кластерного wire требуют **либо** bump версии протокола, **либо** бита возможности (RFC 16 раздел 10).

### `attest_tx_lag` / `T_tx_catchup` (RFC 6.1) {#term-attest-tx-lag}

- **Простыми словами:** если attester **ещё не получил** все байты транзакций для проверки кандидата, политика MVP предлагает **короткое ожидание** `T_tx_catchup`; в логах может появиться маркер вроде **`attest_tx_lag`** — это **не обязательно ошибка**, а сигнал для аналитики ([RFC 16](rfc/16-validator-clone-attestation.md) раздел 6.1).

---

## MVP V3: foundation и public devnet {#thema-v3-foundation}

Короткие определения для закрытия **MVP V3** (документы и runbook, без прод-обещаний V4/V5). Подробнее: [план V3](plans/mvp_v3.md), [API v1](api-v1.md), [ADR index](adr/README.md).

### Public devnet (публичный devnet) {#term-public-devnet}

- **Простыми словами:** открытый для внешних тестеров сценарий: по инструкции из репозитория поднять несколько узлов и дернуть **публичный** HTTP API, не читая исходники приложения.
- **Важно:** это **не** обещание production-безопасности; passphrase и демо-ключи в runbook специально публичные и только для локальной лаборатории.

### Demo genesis (демо genesis) {#term-demo-genesis}

- **Простыми словами:** заранее согласованный пакет **genesis** (валидаторы, shard, начальные балансы), который скрипты собирают из репозитория для демо, чтобы все видели одну и ту же стартовую цепочку.

### Premine 21B PWM (21 миллиард, в raw) {#term-premine-21b}

- **Простыми словами:** целевая эмиссия в whitepaper — **21 000 000 000 PWM**; в коде и верификаторе её проверяют в **raw** единицах: **`21_000_000_000_000_000` raw** при масштабе `1 PWM = 1_000_000 raw` (см. runbook и `scripts/demo-genesis-verify.ps1`).

### API freeze (`/v1/*`, public stable) {#term-api-freeze-v1}

- **Простыми словами:** договорённость V3: перечисленные в `docs/api-v1.md` маршруты **`/v1/status`, `/v1/head`, `/v1/accounts`, `/v1/account/:id`, `/v1/tx`** считаются **публичным стабильным контуром** для devnet; ломать их контракт после V3 нужно осознанно (например отдельная версия пути). Операторские и dev-only маршруты в тот же freeze **не** входят.

### Epoch Snapshot (снимок эпох) {#term-epoch-snapshot-v3}

- **Простыми словами:** то, как `pwmd` **сейчас** хранит операционное состояние на диске: сводка `pwm-data.json` плюс каталог `epochs/` с файлами блоков и манифестом `pwm-epochs-manifest.json` (в V3 у манифеста свой **`schema_v`**, не путать с версией genesis или wire-снимка).

### Bootstrap Snapshot {#term-bootstrap-snapshot-v3}

- **Простыми словами:** **будущий** архивный формат «тяжёлого» снимка для долгого хранения и восстановления после pruning; в **V3** он описан в ADR как направление, **без** полной реализации в runtime.

### Cleanup-chain (цепочка архивных обязательств) {#term-cleanup-chain-v3}

- **Простыми словами:** задуманная **линейная цепочка** архивных commitments (каждый новый якорь ссылается на предыдущий), чтобы сопровождать будущую уборку истории; в V3 это **архитектурная рамка** в ADR 0004, не рабочий протокол для всех узлов.

### Replay determinism gate (гейт детерминизма replay) {#term-replay-det-gate-v3}

- **Простыми словами:** короткая команда тестов, которая дважды прогоняет один и тот же **replay** и проверяет, что итог (например state root / tip) **не разъехался** — страховка от скрытой недетерминированности в пути воспроизведения цепочки. В документации для V3 зафиксирована как `cargo test -p pwmd --lib v3_replay_det_gate_ok` (см. [руководство по storage](guide-node-storage-and-snapshot.md)); она **не заменяет** отдельные тесты на формат манифеста на диске.

### ADR (архитектурные решения V3) {#term-adr-v3-package}

- **Простыми словами:** короткие документы «как мы договорились думать дальше»: IPv4 claiming (0002), offchain scaling (0003), cleanup-chain и снимки (0004). Статусы **Draft (V3 foundation)** означают «направление зафиксировано», а не «всё уже реализовано в ноде».

---

## Алфавитный указатель

### Латиница (A–Z)

| Термин / токен | Ссылка |
|----------------|--------|
| A ADR (V3 foundation package) | [ADR V3](#term-adr-v3-package) |
| A API freeze `/v1/*` | [API freeze V3](#term-api-freeze-v1) |
| A attester | [Attester](#term-attester) |
| A auto-attest (incoming propose) | [Авто-ClusterAttest](#term-auto-cluster-attest) |
| B Bootstrap Snapshot (future) | [Bootstrap Snapshot](#term-bootstrap-snapshot-v3) |
| B `binding_mismatch` | [binding_mismatch](#term-binding-mismatch) |
| C capability / `PWM_PROTOCOL_VERSION` | [Capability / версия](#term-capability-version) |
| C cleanup-chain (V3 ADR) | [Cleanup-chain](#term-cleanup-chain-v3) |
| C ClusterPropose / ClusterAttest (wire) | [Wire](#term-wire), [кластер](#thema-cluster) |
| D demo genesis | [Demo genesis](#term-demo-genesis) |
| E Epoch Snapshot (V3) | [Epoch Snapshot V3](#term-epoch-snapshot-v3) |
| F follower | [Source и follower](#term-source-follower) |
| G `run_cluster_gate` / gate | [`run_cluster_gate`](#term-run-cluster-gate) |
| K k-of-n | [k-of-n](#term-k-of-n) |
| N `node_instance_id`, `--node-instance-id` | [membership](#term-membership) |
| P premine 21B (raw) | [Premine 21B](#term-premine-21b) |
| P proposer | [Proposer](#term-proposer) |
| P public devnet | [Public devnet](#term-public-devnet) |
| P `process-local` (lease backend) | [Аренда в процессе](#term-process-local-lease) |
| peer-behind | [Peer-behind](#term-peer-behind) |
| Q `quorum_pending`, `quorum_timeout` | [quorum_pending](#term-quorum-pending), [quorum_timeout](#term-quorum-timeout) |
| R replay determinism gate | [Replay gate V3](#term-replay-det-gate-v3) |
| R `record_cluster_propose_originated` | [зеркало propose](#term-record-cluster-propose-originated) |
| S `send_cluster_prop` | [send_cluster_prop](#term-send-cluster-prop) |
| S same-shard, seal, source | [same-shard](#term-same-shard), [seal](#term-seal), [follower](#term-source-follower) |
| S2 lease | [S2](#term-s2-lease) |
| T `tip_h`, `tip_hash`, tests `tip_behind_no_divergence` | [tip_h / tip_hash](#term-tip-h-tip-hash), [tip_behind](#term-tip-behind-no-divergence) |
| W wire | [Wire](#term-wire) |
| X `xfer_state_bidir_stable` | [тестовый жаргон](#term-xfer-state-bidir-stable) |

### Кириллица (А–Я)

| Термин | Ссылка |
|--------|--------|
| Активный набор кворума | [Relay pool vs quorum](#term-relay-pool) |
| API freeze `/v1/*` (V3) | [API freeze V3](#term-api-freeze-v1) |
| Архитектурные записи ADR (V3) | [ADR V3](#term-adr-v3-package) |
| Авто-аттестация (входящий propose) | [Авто-ClusterAttest](#term-auto-cluster-attest) |
| Аренда в процессе (lease) | [`process-local`](#term-process-local-lease) |
| Аттестер | [Attester](#term-attester) |
| Bootstrap Snapshot (будущий) | [Bootstrap Snapshot](#term-bootstrap-snapshot-v3) |
| Гейт (кластерный) | [`run_cluster_gate`](#term-run-cluster-gate) |
| Гейт replay (V3) | [Replay determinism gate](#term-replay-det-gate-v3) |
| Двухузловой soak | [Двухузловой soak](#term-two-node-soak) |
| Демо genesis | [Demo genesis](#term-demo-genesis) |
| Догоняющий узел (follower) | [Source и follower](#term-source-follower) |
| Зеркало propose | [`record_cluster_propose_originated`](#term-record-cluster-propose-originated) |
| Cleanup-chain (V3) | [Cleanup-chain](#term-cleanup-chain-v3) |
| Исходящий ClusterPropose | [`send_cluster_prop`](#term-send-cluster-prop) |
| Источник (source) | [Source и follower](#term-source-follower) |
| Клон валидатора | [Validator clone](#term-validator-clone) |
| Кворум | [k-of-n](#term-k-of-n) |
| Модель экземпляра (`node_instance_id`) | [membership](#term-membership) |
| Одна шарда (same-shard) | [same-shard](#term-same-shard) |
| Premine 21B (эмиссия в raw) | [Premine 21B](#term-premine-21b) |
| Пир «сзади» | [Peer-behind](#term-peer-behind) |
| Пропоузер | [Proposer](#term-proposer) |
| Публичный devnet | [Public devnet](#term-public-devnet) |
| Разводка / divergence | [`sync_tip_divergence`](#term-sync-tip-divergence) |
| Реле-пул | [Relay pool](#term-relay-pool) |
| Сейл | [Seal](#term-seal) |
| Снимок эпох (Epoch Snapshot, V3) | [Epoch Snapshot V3](#term-epoch-snapshot-v3) |
| Состав кластера (membership) | [membership](#term-membership) |
| Транспорт по проводу (wire) | [Wire](#term-wire) |

---

*Информация согласована с обзорами V2-9, RFC 16 и блоком MVP V3 foundation в этом файле по состоянию на 2026-05; при изменении протокола сверяйте первоисточник в `docs/rfc/16-validator-clone-attestation.md`.*
