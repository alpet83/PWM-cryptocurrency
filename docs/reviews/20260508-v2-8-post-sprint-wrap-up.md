# V2-8 post-sprint wrap-up (Slices 0–5)

**Дата:** 2026-05-08  
**Агент:** pwm-review  
**Основание:** `docs/plans/mvp_v2.md` (Sprint V2-8), `docs/rfc/15-same-shard-sync-v1.md`, срезы ревью `20260508-v2-8-slice*`, топология мемпула и предложения по mesh, обзор конкуренции валидаторов.

---

## Executive summary

Спринт **V2-8 (same-shard chain sync v1)** по слайсам **0–5** доведён до закрытия по конвейеру: **RFC freeze**, **wire/capability-рамка**, **mempool gossip baseline**, **header-first live sync + apply**, **epoch catch-up fallback**, **наблюдаемость/chaos/docs + финальный hotfix теста** (`23a183f`). Качество по срезам в основном **PASS** или **PASS_WITH_NITS**; блокирующих «откатов спринта» в финальных артефактах не зафиксировано. Существенный **остаточный техдолг** — неполное соответствие букве **fork-choice §7.3** (кортеж с `finalized_height`) в live-sync, **операционная семантика `finalized_height` под PoA**, жизненный цикл **catch-up при `SyncNack`/ошибке записи**, **флаки/пробелы e2e**, и **риск амплификации mempool-трафика** в плотной mesh-топологии при текущем batch-push. Для **узкого testnet/demo** с малым числом пиров и осознанной топологией — **достаточно для следующей ветки работ**; для **плотного кластера / периферии NAT** — нужны RFC-guidance и стабилизация.  
**Вердикт спринта:** **READY_FOR_NEXT_BRANCH** (с явным списком стабилизации и RFC-follow-up ниже).

---

## Slice status matrix (0..5)

| Slice | Назначение (по плану) | Качество (ревью) | Краткий комментарий |
|------|------------------------|------------------|---------------------|
| **0** | RFC freeze, acceptance §11 | **PASS** / приёмка как freeze: **PASS-WITH-NITS** (ниты закрыты docs-правками) | Заморожен контракт v1; follow-up: статус **RFC 8** (Draft vs Frozen). |
| **1** | Wire skeleton + capability gates | **PASS_WITH_NITS** | Трассируемость §11/§5.1/§7 закрыта в `3d1422c`; nit: операторское правило **`finalized_height`**, кросс-refs **height → prod_idx**. |
| **2** | Mempool gossip (`SyncTx*`) | **PASS_WITH_NITS** | Baseline batch-push, dedup/caps/gating ок; ниты: тесты **rate_limit/outbound**, семантика **profile_mismatch** при shard, **флак** `slice20_dual_flow_ok`. |
| **3** | Header-first + block apply | **PASS_WITH_NITS** | Форма tip→headers→blocks и rollback-safe apply соблюдены; **§7.3 tuple** и **ingest `finalized_height`** — частично; multi-peer конфликт tips — без идеала. |
| **4** | Epoch catch-up | **PASS_WITH_NITS** | Объём §6.3/§8 в целом соблюдён; **nit:** `cup_active` может застрять при **`SyncNack`** / **write_wire_msg Err** без явного abort. |
| **5** | Observability + chaos + runbook + RFC | **PASS** (финальный re-gate) | Hotfix выравнивания **`tx_batch_profile_drop`** на **`23a183f`**; ранний nit про дубликат метрик без изменения статуса. |

Источники строк: `20260508-v2-8-slice0-review.md`, `20260508-v2-8-slice1-rereview.md`, `20260508-v2-8-slice2-review.md`, `20260508-v2-8-slice3-review.md`, `20260508-v2-8-slice4-review.md`, `20260508-v2-8-slice5-rereview-final.md`.

---

## Residual nits backlog (prioritized)

**P0 — согласованность канона и зависание режимов**

1. **Catch-up stuck state:** при активном CUP не снимать `cup_active` на **`SyncNack`** и при ошибке отправки запроса — риск «тихого» стопа догонки до переподключения (`20260508-v2-8-slice4-review.md`).
2. **`finalized_height` vs PoA:** в спецификации и между нодами нет единого нормативного правила, как поле в **`TipAnnounce`** соотносится с round-robin и локальным tip — риск **разъезда fork-choice** (`20260508-validator-competition-in-shard.md`, `20260508-v2-8-slice3-review.md`, `20260508-v2-8-slice1-rereview.md`).

**P1 — полнота v1 относительно RFC и тестового долга**

3. **Fork-choice §7.3:** сравнение кортежа **между пирами** и использование **`finalized_height` на ingress** — narrative/partial; outbound **`finalized_height = head`** не отражает «реальный» lag finalized (`slice3-review`).  
4. **`anchor_hash` в catch-up:** responder не интерпретирует — осознанный defer, но ветка/загрязнение запроса остаётся на apply (`slice4-review`).  
5. **Тесты:** нет **unit** на inbound **`rate_limit`** mempool-batch; нет автоматического **outbound** `send_sync_tx_batch`; нет сценария **SyncNack после выставления CUP**; **флак** `slice20_dual_flow_ok` — техдолг стабилизации (`slice2`/`slice3`/`slice4`).  
6. **Трассируемость pwm-testing:** ранее в `slice3-review` отмечался разрыв с git для отчёта тестирования; в текущем дереве **`docs/reviews/20260508-v2-8-slice3-testing.md`** присутствует — при аудите истории сверить SHA с коммитом реализации slice3.

**P2 — сеть и спецификация mempool**

7. **Топология:** фактически **периодический push полных тел** в batch; **Announce/Req** на приёме **unsupported** — расхождение с «полным» §4.1 RFC; в плотной mesh — **амплификация** (`20260508-mempool-gossip-topology.md`).  
8. **RFC / policy:** зональные лимиты, pull на периферии, anti-entropy — **NEED-RFC** (`20260508-mempool-mesh-anti-amplification-proposal.md`). RFC 15 дополнен §13 cluster storm guard — держать согласованность с операторскими профилями peering.  
9. **DoS / relay:** нет выделенного **per-peer rate limit** на частоту mempool-батчей; дедуп обновляется до тяжёлых проверок — приемлемо для baseline, но остаётся класс риска (`slice2-review`).  
10. **Slice 1 (исторический nit):** `SyncProfileAnnounce` vs handshake-only, расширение тестов shard gate — в backlog следующих итераций (`slice1-rereview`).

---

## Operational readiness assessment (testnet/demo)

| Критерий | Оценка | Комментарий |
|----------|--------|-------------|
| Два узла одной шарды, догон tip | **Хорошо для baseline** | Live + catch-up покрыты ревью и тестами в объёме слайсов; сценарии **многопирового** конфликта tips — слабее. |
| Mempool gossip между native peers | **Ок для малой степени** | Dedup и caps есть; в **full-mesh / L2-плотности** — риск шторма тел и egress (см. topology + §13 RFC 15). |
| Fallback epochs | **Условно** | Пока нет доказательства обработки **Nack** без зависания CUP (P0). |
| Legacy / negotiation | **Согласовано в тексте** | `sync_profile` как источник `full_v1`; smoke по review-цепочке. |
| Наблюдаемость и runbook | **Заявлено Slice 5** | Использовать метрики `sync_*` / `sync_tx_*` / CUP counters; алерты на рост `duplicate`/drops — см. proposal §6. |
| Операторская дисциплина PoA | **Обязательна** | Один активный sealer на высоту или явное расписание — иначе форки «выглядят как конкуренция» (`validator-competition`). |

**Итог для эксплуатации:** **demo-ready** при **ограниченной топологии** (низкая степень same-shard peering, контролируемый список сидов), **явном правиле `finalized_height`** на уровне конфига сети или временном RFC-патче, и **плане** на исправление **CUP+Nack**. Без этого — повышенный риск **недетерминизма tip** и **залипания догонки**.

---

## Recommended next branch (2–4 пункта)

1. **RFC amendment (узкий):** зафиксировать **`finalized_height`** для PoA-devnet/testnet (источник истины, монотонность, согласование с `TipAnnounce`) и выровнять **height → prod_idx** между WHITE и ядром; обновить **§7** при необходимости.  
2. **Стабилизация transport:** исправить жизненный цикл **`cup_active`** (Nack, write-fail) + юнит-тесты; довести **fork-choice** до заявленного кортежа или явно задокументировать **degenerate** режим для testnet.  
3. **Mempool v1.1 / policy:** принять **implementation guidance** или minor RFC по **зональным** режимам (dense vs bridge vs edge), лимитам **bytes/msg per peer**, и roadmap на **Announce/Req** или id-only межкластер.  
4. **Качество CI:** устранить **флаки** `slice20_dual_flow_ok`, закрыть пробелы **e2e** (два-три узла TCP) по согласованию с `pwm-testing`; коммитная трассируемость для всех `*-testing.md` слайсов.

---

## Slice 6 и перенос приёмки на Sprint V2-9 (дополнение 2026-05-09)

Планировавшийся **Slice 6** (автоматические wave A/B/C post-sprint) и связанный hotfix по **tip_hash** на практике уперлись в иной класс проблемы: при нескольких нодах одной шарды с **одной и той же identity валидатора** и конкурирующих seal наблюдались **расходящиеся цепочки** и **недетерминизм заголовков**; устранить это «только синхронизацией» без смены модели печати не удалось. **Стратегический пивот зафиксирован:** целевой режим — **один активный пропозер** и **аттесторы** ([RFC 16](p:/opt/docker/pwm-protocol/docs/rfc/16-validator-clone-attestation.md)), Sprint **V2-9**; функциональная приёмка многонодовых сценариев и ведомых вне кластера **переносится** туда (новые тесты под новый контракт). Same-shard sync **V2-8 (слайсы 0–5)** остаётся транспортной базой для подписчиков шарды.

**Нормативные ссылки:** [docs/plans/mvp_v2.md](p:/opt/docker/pwm-protocol/docs/plans/mvp_v2.md) — Sprint V2-8 «Статус и перенос приёмки», Sprint V2-9 (наследование от Slice 6); RFC 16 §16 (v0.4.6+); тикеты `tasks/20260508-v2-sprint8-slice6-automated-waves.json`, `tasks/20260508-v2-slice6-hotfix-tip-hash-divergence.json` (**blocked**); исполнительный трек — `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`.

---

## Verdict

**READY_FOR_NEXT_BRANCH** — спринтовые цели V2-8 (slices 0–5) по ревью-артефактам **закрыты**; переход к следующей ветке (**RFC-патч PoA/fork-choice**, **стабилизация CUP и flake**, **mempool policy**) **рекомендован** до масштабирования testnet на плотные топологии.

---

## Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/20260508-v2-8-post-sprint-wrap-up.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 11000
  confidence: medium
```

---

_End of wrap-up._
