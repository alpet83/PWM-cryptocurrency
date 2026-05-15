# Обзор: роли кластера (follower / attester / proposer), RFC16 и связка с кодом pwmd

## Scope

- Сверка **ожиданий оператора** с реализацией и черновиком RFC16 (`docs/rfc/16-validator-clone-attestation.md`): follower зеркалит пропозера и персистит; cluster-attester — то же плюс attest по peer wire; proposer — seal при прохождении lease/cluster gates и персистит.
- Проверка гипотезы **продуктового разрыва**: подавление локального seal-loop на attester должно следовать из **`ClusterRole::Attester`** (и политики standby), а не только из **`--debug-disable-seal-loop`**, когда cluster включён.
- Обязательное **дискавери логов** на диске репозитория Windows (`logs/…`), без утверждений «логов нет» без чтения файла.
- Prior art: `docs/reviews/20260513-attester-sync-persistence-review.md` — синхронизация и autosnapshot уже разобраны; здесь не повторяем доказательную базу про `route_sync_stub` и интервал 100 блоков, а дополняем ось **роль ↔ seal ↔ RFC**.

## Log path cookbook (операторы и будущие субагенты)

| Элемент | Факт |
|--------|------|
| Корень логов по умолчанию | `--log-dir` / `PWM_LOG_DIR`, значение по умолчанию **`logs`** (относительный путь от **cwd процесса**). Источник: `main.rs` (`log_dir`). |
| CY-лаунчеры | `Set-Location $PSScriptRoot` → файлы попадают в **`{repo}/logs/`**. |
| Основной файл | Шаблон по умолчанию `{date}/{log_name}-{node_id}-{time}.log`; `{date}` — **UTC**, подстановки в `logging.rs` (`expand_log_template_path`, `now_tokens`). |
| Peer-транспорт | Отдельный sink для таргета `pwmd::peer`; шаблон по умолчанию **`{date}/pwmd-peer-{node_id}-{time}.log`** (`LoggingConfig::peer_file_template` в `config.rs`). Консоль эти события режет (`is_peer_target` в `logging.rs`). |
| Поиск артефакта | Glob вида `logs/*/pwmd-peer-<node_id>-*.log`; для CY см. `--node-id` в `cy-cluster-*.ps1` (`cy-attester`, `cy-proposer`, `cy-follower`). |
| Пример из тикета | Прочитан хвост `logs/2026-05-12/pwmd-peer-cy-attester-173552.log` (~85 строк с конца); для сравнения — хвост `pwmd-peer-cy-proposer-173550.log`. |

## Наблюдения по логам (attester)

В **начале** того же файла attester видны строки синхронизации: `peer sync mode negotiated … mode=full_v1`, `peer sync catchup start …`, далее череда **`peer sync nack`** (`catchup_epoch`, `headers_range`) и переподключения к follower-порту (`127.0.0.1:33432`). Это подтверждает, что **sync-путь логируется в peer-файле**, а не в «тишину консоли» (согласуется с prior review).

В **хвосте** (~80 строк) доминируют **`cluster propose accepted`** от пропозера и **`peer sync nack … headers_range`**, попытки TCP к `33432`, без строк **`autosnapshot checkpoint`** / явного **`sync apply ok`** в выборке по этому файлу за пределами ранней фазы — что совместимо с prior отчётом: persist через autosnapshot редкий (каждые N блоков), а успешный apply может быть раньше по файлу или отсутствовать при застое/nack-only хвосте.

## Role matrix: ожидание vs лаунчеры vs код

Легенда: **SealRole** — что анонсируется в hello и деривируется локально; **seal-loop** — периодический `spawn_seal_loop` в `lifecycle.rs`; **cluster gate** — `run_cluster_gate` перед `Chain::seal` если loop активен.

| Роль (оператор) | Ожидание | `cy-cluster-*.ps1` | `ClusterRole` в конфиге | `derive_seal_role` (`lifecycle.rs`) | Seal-loop | Cluster gate на seal |
|-----------------|----------|-------------------|-------------------------|--------------------------------------|-----------|----------------------|
| Follower | Зеркало + persist; **не** локально seal в CY | `cy-cluster-follower.ps1`: **нет** `--cluster-enabled`; есть **`--debug-disable-seal-loop`** | `None` (дефолт) | `debug_disable_seal_loop` ⇒ **Standby** | Ранний `continue` — seal не выполняется | Не доходит до вызова при отключённом loop |
| Cluster attester | Как follower **+** RFC16 attest; **не** конкурирующий seal Variant A | `cy-cluster-attester.ps1`: `--cluster-enabled`, **`--cluster-role attester`**, **`--debug-disable-seal-loop`** | `Attester` | Только через **`debug_disable_seal_loop`** ⇒ Standby (`ClusterRole` **не участвует**) | Как follower — полностью отключён флагом | Не достигается |
| Proposer | Seal + persist + propose/quorum | `cy-cluster-proposer.ps1`: cluster on, роль proposer; **нет** debug-disable-seal-loop | `Proposer` | По умолчанию **Active** | Полный путь: lease → **`run_cluster_gate`** → seal | Да, когда cluster включён |

**Важная связка кода:** `derive_seal_role` учитывает `seal_role_override`, затем **исключительно** `debug_disable_seal_loop` → `Standby`; иначе `Active`. Поле **`cluster.role`** при этом **не читается**. Инициализация приложения выставляет `hs.local_seal_role = app.seal_role` (**не** производную от `ClusterRole`).

**`spawn_seal_loop`:** при `debug_disable_seal_loop` весь блок lease/cluster/`Chain::seal` обходится (комментарий в коде прямо называет режим follower/replay-only). Без этого флага узел с **`ClusterRole::Attester`** теоретически входит в полный seal-путь; **`run_cluster_gate` не проверяет локальную роль** (только quorum-состояние и членство). Это оставляет ответственность за «не быть вторым sealer» на **лаунчер / флаг / ручной `--seal-role`**, а не на инвариант «attester никогда не вызывает seal» на уровне роли cluster.

## RFC16: поддержка и противоречия

**Раздел 6 (валидность до attest):** нормативно проверки применяются к **кандидату лидера**, tip-доступность или catch-up перед подписью — в коде attest-путь отделён от локальной сборки блока в seal-loop; prior review уже установил отсутствие блокировки sync по `ClusterRole`. Здесь добавление: **ожидание оператора «attester как follower+catch-up»** согласуется с тем, что sync не отключён ролью.

Краткая опора RFC (tip / lag):

> «Tip consistency: parent hash matches expected head **for this clone’s view**, OR documented reconciliation rule if lagging (раздел 9.4).»

и раздел 9.4 про отстающий clone:

> «If attester’s tip is behind `H-1`, it MUST **reject** attest … **or** run catch-up … first — profile chooses …»

Это согласуется с необходимостью живого sync у attester; не противоречит текущей архитектуре peer-logов и steady-session.

**Раздел 8 (кто seal):**

> «Who performs **final seal**? **One** designated **committer** (often = leader) after quorum; **others MUST NOT seal** the same `(H,R)` candidate.»

Текущая **жёсткая гарантия** «attester не входит в seal path» в Variant A lab достигается **лаунчером** (`cy-cluster-common.ps1` строки про `--debug-disable-seal-loop`), а не условием `ClusterRole::Attester` в `derive_seal_role` / `spawn_seal_loop`. Это и есть подтверждённая **документально-продуктовая щель**: семантика RFC про единственного committer **не отражена** в автоматическом выводе seal-режима из cluster-роли.

**Раздел 9 (операции):** таблица реакций (нет quorum → нет seal и т.д.) хорошо стыкуется с логированием `seal_suppressed_by_cluster` в `run_cluster_gate`; для оператора полезно помнять различие **«нет seal из-за quorum»** (proposer) и **«нет seal-loop потому что debug-disable»** (attester/follower в CY).

## Gaps и предложения (для pwm-coding / доков), размер

- **S — документация / CLI help:** В `main.rs` пояснить, что для **`--cluster-role attester`** в профиле Variant A ожидается режим без локального seal (ссылка на RFC16 раздел 8); явно перечислить два поддерживаемых способа: **`--debug-disable-seal-loop`** или **`--seal-role standby`**, и что это **не случайный тест-only**, а лабораторный/операционный паттерн, пока нет автодеривации из роли.
- **S — cy-cluster-common.ps1:** Уже есть комментарий про seal-loop; можно добавить одну строку «RFC16 раздел 8: только proposer seal» для трассируемости без дублирования длинного RFC.
- **M — продуктовое правило:** При **`cluster.enabled && cluster.role == Attester`** автоматически выводить **`SealRole::Standby`** и/или пропускать seal-loop (с исключением только если явный override и предупреждение в логе). Альтернатива минимального риска: **fail-fast** при включённом cluster attester без standby/debug-disable с текстом ошибки.
- **M — инвариант на seal-path:** Явная проверка «не вызывать `Chain::seal` из periodic loop при `ClusterRole::Attester`» даже если конфиг ошибочно оставил Active — защита от гонки с пропозером при симметричном `run_cluster_gate`.
- **L — политика standby vs RFC «committer»:** Вынести в операторский гайд матрицу: S2 lease, `SealRole`, `ClusterRole`, cluster quorum — ортогональность как в RFC раздел 8.1; указать, что **`debug-disable-seal-loop`** сегодня — фактический мост между «follower replay» и «attester без seal».

## Requirements fit

- Ожидания оператора по **sync mirror + RFC16 attest на attester** и **persist через общий путь apply/autosnap** — **совместимы** с кодом и prior review (роль не режет sync).
- Ожидание **«suppression seal следует из ClusterRole::Attester без обязательного отдельного флага»** — **не выполнено**: код завязан на **`debug_disable_seal_loop`** / override seal role, **`ClusterRole` не участвует** в `derive_seal_role`.
- Соответствие RFC16 разделу 8 по запрету seal для non-committer clone — **обеспечивается конфигурацией лабораторных скриптов**, а не только типом узла в коде.

## Style / safety / tests (кратко)

- Именование production-fn для этого тикета не менялось; отдельный прогон `check_rust_fn_name_segments.py` не обязателен для объёма ревью «док + трассировка».
- **Safety:** при ошибочной конфигурации attester без standby/debug-disable возможен **конкурирующий локальный seal path** при симметричной логике gate — классифицировать как **средний+** риск до появления инварианта по роли (см. gaps **M**).
- **Tests:** имеет смысл добавить регрессионный тест: конфиг «cluster attester + Active seal без debug-disable» либо запрещён на старте, либо seal-loop no-op — по выбранной политике pwm-coding.

## Verdict

**PARTIAL (FAIL по формулировке gate «ясность документации роли vs флага»):** расхождение **RFC16 раздел 8 ↔ `derive_seal_role` / seal-loop** и зависимость CY-lab от **`--debug-disable-seal-loop`** объяснимы из исходников и комментария в `cy-cluster-common.ps1`, но **не доведены до операторского контракта** в CLI/RFC/runbook как единая матрица. После явной документации и (желательно) инварианта в коде gate можно закрыть как **PASS**.

## Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts:
  - docs/reviews/20260513-cluster-role-rfc-alignment-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 11000
  confidence: low
```

**GLOSSARY.md:** без изменений (нового жаргона не появилось; не финальное ревью спринта).

**Вердикт одной строкой для оркестратора:** `PARTIAL — RFC16 раздел 8 не закреплён в derive_seal_role/CLI; подтверждён coupling через --debug-disable-seal-loop и CY scripts; нужны доки ± инвариант по ClusterRole::Attester на seal-path.`
