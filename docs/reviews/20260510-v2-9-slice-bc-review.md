# Combined gate: Sprint V2-9 Slice B + Slice C (короткий ревью, 2026-05-10)

**Скоуп ревью:** правки pwm-coding в `record_cluster_propose_originated` (`crates/pwmd/src/transport/peer_session/mod.rs`), TCP cluster-тесты в `crates/pwmd/src/transport/tests/production.rs`, заметки `docs/reviews/20260510-v2-9-slice-b-wave-notes.md` и `docs/reviews/20260510-v2-9-slice-c-wave-notes.md`; **delta (commit-scale):** `sync_live::on_tip` — ранний **`Ok(None)`** при **`head_h < local_h`** и расчёт `lag` только при **`head_h >= local_h`**; регрессия **`tip_behind_no_divergence`** в `peer_session/mod.rs`.  
**Чеклист:** `docs/reviews/20260509-v2-9-rfc16-sprint-checklist.md` §3 (Slice B/C).

---

## 1. Scope recap

- **Хелпер** `record_cluster_propose_originated` зеркалирует исходящий `ClusterPropose` в `HandshakeState.cluster_attest` на стороне proposer (повторно использует `record_cluster_prop`), чтобы gate на proposer совпадал с сценарием «proposer → inbound attester», где предложение на proposer по TCP само не попадает.
- **Позитивы:** wire E2E `cluster_2of2_gate_ok` (proposer зеркало + attest на proposer через отдельный inbound), `cluster_2of3_gate_wire`, деградация `cluster_2of3_one_ack_stuck`.
- **Негативы в production.rs:** `cluster_timeout_no_seal`, `cluster_bind_mismatch_no_seal`.
- **Доки:** Slice B wave notes под wire; Slice C wave notes — same-shard + cluster gate, затем обновление с **peer-behind** root/fix и фильтром `tip_behind_no_divergence`; multi-node TCP soak для same-shard follower помечен **landed** (`same_shard_follower_tcp_tip` в `crates/pwmd/src/tests/transport_peer.rs`, см. §5–§6 и wave notes).

---

## 2. Requirements fit (vs checklist)

**Slice B**

| Строка чеклиста | Оценка |
|-----------------|--------|
| Happy 2-of-2 seal + воспроизводимая публикация | **Delta:** в `cluster_2of2_gate_ok` и в **2-of-3** happy (`cluster_2of3_gate_wire`) после `run_cluster_gate` добавлены тот же паттерн: локальный `seal` и assert на инкремент `tip_h` и смену `tip_hash` (снимок до/после seal). Wire-кворум и binding по-прежнему проверены; **зазор по parity 2-of-2 vs 2-of-3 на этом пункте снят.** |
| Negative: нет кворума, reason в логах | **Repeat review:** топология `…_no_seal` выровнена с happy-path (propose **A→B**, `record_cluster_propose_originated` на A); кворум по времени без attest согласуется с закрытым `run_cluster_gate`. **Delta (second addendum):** для `cluster_timeout_no_seal` и `cluster_bind_mismatch_no_seal` добавлен WARN-capture и assert на ожидаемые строки (`quorum_timeout` / `binding_mismatch`) — мягкий nit по логам **снят** для этих двух негативов. |
| Fault inject §11 | **Repeat review:** после фикса harness негативы ближе к заявленным путям; детальное сверение с каждым полем wave notes в этом проходе не повторялось. |
| Артефакт воспроизведения | Да: команды `cargo test` в wave notes; позитивы дают ценный repro. |

**Slice C**

| Строка чеклиста | Оценка |
|-----------------|--------|
| Happy 2-of-3 после кворума | Закрыто: TCP + `run_cluster_gate` + тот же post-gate `seal` и assert `tip_h` / `tip_hash`, что и для 2-of-2 (`cluster_2of3_gate_wire`). |
| Degradation (один ack не bypass-ит k=2) | Закрыто (`cluster_2of3_one_ack_stuck`). |
| Cluster + non-cluster follower, convergence | **Закрыто:** `blk_fetch_apply_ok` (non-cluster контур, **`tip_hash` после apply**) + peer-behind fix в **`sync_live::on_tip`** с **`tip_behind_no_divergence`**. **TCP soak (landed):** `same_shard_follower_tcp_tip` — источник с **`cluster_cfg.enabled`**, follower с cluster off, стабильный bidirectional TCP, bounded ожидание **`tip_h` + `tip_hash`**, счётчики **`sync_tip_divergence_disconnect_total`** не растут (см. `transport_peer.rs` и `20260510-v2-9-slice-c-wave-notes.md`). |

---

## 3. Style и форма модулей

- Имена символов: `python scripts/check_rust_fn_name_segments.py` на затронутых путях — **нарушений нет**.
- `record_cluster_propose_originated` помечена `#[allow(dead_code)]`: на не-test сборках это честное подавление, реальные интеграционные callers вне `cfg(test)` по поиску **отсутствуют**. Для финального прод-пути проposer всё равно нужно будет записывать раунд при **исходящем** propose (или иной канонической точке), не только через тестовый помощник — иначе риск расхождения интеграции и harness.
- Wire / версия протокола: изменений сообщений или handshake в этом слайсе не видно; bump `PWM_PROTOCOL_VERSION` не вызывается.

---

## 4. Safety

- **Trust boundary:** `record_cluster_prop` уже не валидирует подпись propose (семантика доверительного транспортного канала сохранена). Отдельное зеркало на proposer в тестах не создаёт доверенный удалённый ввод без вызова — но в прод-коде появление публичного сходного входа должно быть только на границе «мы сами отправили по защищённой сессии».
- **Паники unwrap:** только внутри harness/test путей, не считалось регрессом для gate.

---

## 5. Tests

- Прогнан локально фильтром: `cargo test -p pwmd cluster_` — **21 пройдено**, включая пять производственных harness-тестов в `production.rs`. Для peer-behind: **`transport::peer_session::tests::tip_behind_no_divergence`** (см. wave notes для команды фильтром). Для Slice C follower TCP convergence: **`tests::transport_peer::same_shard_follower_tcp_tip`** в `crates/pwmd/src/tests/transport_peer.rs` (фильтр `cargo test -p pwmd same_shard_follower_tcp_tip --lib`).
- Покрытие **позитивов** считается **существенным** для RFC §10 поверх установленной peer-сессии.
- **Негативы** зелёные; после **delta** явно проверяются WARN-строки с `quorum_timeout` и `binding_mismatch` (см. Addendum второй).
- **Repeat review:** описанный ранее «идеальный минимальный фикс» направления propose (как в happy-path 2-of-2) **применён** в `production.rs` для `cluster_timeout_no_seal` и `cluster_bind_mismatch_no_seal`.

---

## 6. Verdict

**PASS** (финальный gate после pwm-coding): позитивные 2-of-2 / 2-of-3 (включая одинаковый post-gate seal + tip) и деградация k=2 консистентны с wire и `run_cluster_gate`; хелпер зеркалирования закрывает разрыв «исходящий propose не виден proposer inbound». Slice C: peer-behind + bounded **TCP** сходимость follower к tip источника с cluster on зафиксированы тестами; **material** остаточных нитов по прежнему открытому defer soak **нет** (см. nit 4).

**Ниты / условия полного чеклиста**

1. ~~**P0 (тесты):** перестроить `cluster_timeout_no_seal` и `cluster_bind_mismatch_no_seal`…~~ **Снято (repeat review):** см. Addendum.  
2. ~~**P1 soft (логи негативов):** reason `quorum_timeout` / `binding_mismatch` в WARN…~~ **Снято (second addendum):** WARN-capture + assert в `production.rs`.  
3. ~~**P1 (приёмка B, остаток):** для **2-of-3** happy (`cluster_2of3_gate_wire`) по желанию строгой трактовки — те же классы assert на seal/tip, что уже есть в `cluster_2of2_gate_ok`, или lab runbook.~~ **Снято:** `cluster_2of3_gate_wire` зеркалирует post-gate `seal` + assert `tip_h` / `tip_hash`, как `cluster_2of2_gate_ok` (`production.rs`).  
4. ~~**P1 (приёмка C, остаток после peer-behind fix):** ложный disconnect при peer-behind~~ — **снято:** `sync_live::on_tip` + **`tip_behind_no_divergence`**. ~~Defer полного TCP soak~~ — **снято (wave notes + код):** multi-node TCP soak для same-shard follower **landed** — **`same_shard_follower_tcp_tip`** (`transport_peer.rs`): cluster-enabled **source**, cluster-off **follower**, bidirectional loops, converge по **`tip_h`/`tip_hash`**, divergence disconnect не растится; это соответствует намерению строки чеклиста §3 Slice C («cluster + non-cluster follower», convergence height/hash) без необходимости отдельного трёх-приложенческого defer из старого текста wave notes.

---

## Addendum — repeat review (2026-05-10)

**P0 (топология `cluster_timeout_no_seal` / `cluster_bind_mismatch_no_seal`):** **снято.** В `production.rs` негативы выровнены с `cluster_2of2_gate_ok`: propose идёт по `handshake_ib_client(app_b, &app_a)` как **A→B** (proposer→attester), затем `record_cluster_propose_originated` на стороне A; для bind-mismatch добавлены канал **B→A** и `trust_attester`, attest с несовпадающим `vote_object` — целевая ветка disagreement/таймаута больше не блокируется ранним drop по ролям.

**RFC 16 v0.4.7, §9.5 (+ §12 item 4, Appendix B.2):** **согласовано** с §7.1 (active quorum / relay pool, выбор подмножества) и §8.1 (слоты кворума и S2 seal lease — разные оси); §9.5 явно откладывает нормативные алгоритмы к §12 item 4; строка B.2 про overload / `quorum_timeout` и future demotion ссылается на §9.5.

**Итоговый вердикт (после repeat review топологии):** исторически **PASS_WITH_NITS** до закрытия soak; финальный gate — **PASS** (см. §6).

---

## Addendum — delta after pwm-coding PARTIAL (2026-05-10)

**P1 soft (WARN / checklist «reason в логах»):** в `crates/pwmd/src/transport/tests/production.rs` для `cluster_timeout_no_seal` и `cluster_bind_mismatch_no_seal` добавлены `warn_log_scope()` и assert на наличие ожидаемых подстрок (`seal_suppressed_by_cluster` + `reason=quorum_timeout`; `cluster attest dropped` + `reason=binding_mismatch`). Это закрывает ранний мягкий nit про отсутствие явной проверки reason в логах для негативов.

**P1 (seal / tip, happy 2-of-2 и 2-of-3):** в `cluster_2of2_gate_ok` и `cluster_2of3_gate_wire` после успешного `run_cluster_gate` выполняется `seal` и проверяются инкремент высоты и смена `tip_hash` относительно снимка до seal — приёмка «локальный seal после открытия гейта» для обоих harness-путей выровнена.

**Slice C:** в `crates/pwmd/src/transport/peer_session/mod.rs` тест `blk_fetch_apply_ok` явно требует `cluster_cfg.enabled == false` и сравнивает `tip_hash` с хешем применённого удалённого блока. **Обновление 2026-05-11 (final gate):** peer-behind false divergence закрыто `on_tip` + **`tip_behind_no_divergence`**; **`same_shard_follower_tcp_tip`** в `transport_peer.rs` закрывает TCP soak follower ↔ cluster-enabled источник (см. wave notes «landed»).

---

## Addendum — Slice C peer-behind delta (2026-05-11)

**Bug / fix:** до правки кейс **`head_h < local_h`** уходил в ветку с **`lag == 0`** после `saturating_sub` и сравнивал tip-хэши на **разных высотах**, что давало ложный **`TipDivergence`**. Сейчас после обновления peer sync state в **`on_tip`** выполняется ранний **`Ok(None)`** при **`head_h < local_h`**, **`lag`** считается только для **`head_h >= local_h`**.

**Регрессия:** `tip_behind_no_divergence` — локальный **seal** до **tip_h == 1**, входящий **SyncTipAnnounce** с genesis (**head_height == 0**), ожидание продолжения сессии и **нулевой** счётчик disconnect по divergence.

**Скоуп soak (обновление final gate):** для приёмочной строки «cluster + non-cluster follower / TCP convergence» **`same_shard_follower_tcp_tip` признан достаточным** (bounded wait, совпадение tip, отсутствие ложной divergence disconnect); более тяжёлый сценарий «три кластера-участника + follower в одном harness» при необходимости остаётся вне scope этого короткого гейта, но **не блокирует** PASS по текущему чеклисту и закрывает прежний nit 4.

---

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": ["docs/reviews/20260510-v2-9-slice-bc-review.md"],
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 14000,
    "confidence": "low"
  }
}
```

**PASS:** прежний nit 4 закрыт тестом **`same_shard_follower_tcp_tip`** (сходимость по высоте и `tip_hash`, TCP, cluster on/off); peer-behind, топология негативов B, WARN-reason и parity happy 2-of-2 / 2-of-3 отражены в отчёте и addenda.

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260510-v2-9-slice-bc-review.md'
git commit -m 'docs(v2-9): Slice B+C gate review final PASS (Slice C TCP soak landed)'
```
