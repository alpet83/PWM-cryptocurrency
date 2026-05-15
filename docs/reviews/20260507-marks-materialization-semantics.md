# Marks materialization: Burn vs Stake/Unstake — semantics review

**Date:** 2026-05-07  
**Agent:** `pwm-review`  
**Question (owner):** После части транзакций (например burn) «marks» как будто не пересчитываются; после Stake/Unstake наблюдается всплеск marks (в т.ч. после дополнительного stake). Это по дизайну или рассинхрон? Нужен ли auto-claim/recalc на любые операции с адресом, включая burn?

---

## Current behavior (code refs)

### Где материализуются matured → `marks`

- **Единственные вызовы `apply_auto_claim`** в `apply_tx_with_ctx` стоят в ветках **`TxBody::Stake`** и **`TxBody::Unstake`**, перед изменением баланса/стейка и с последующим обновлением `last_claim_unix_time` / `last_stake_change_height` у этих же веток. Иных вызовов по репозиторию нет (кроме определения helper’а и тестов).
- **`apply_auto_claim`** добавляет к `acc.marks` значение `matured_units_available`, обновляет `last_claim_unix_time` и `last_claim_anchor_ref`; при `matured == 0` — no-op.

Ссылки (фрагменты):

- `Stake` / `Unstake`: вызов `apply_auto_claim`, затем движение PWM/staked и сброс claim-окна по времени/высоте стейка — см. `crates/pwm-core/src/state.rs` (ветки `TxBody::Stake`, `TxBody::Unstake`).
- **`BurnMark`:** только проверка `marks >= mark_amount`, дебет `marks`, инкремент `nonce` — **без** `apply_auto_claim` и без трогания полей зрелости/якоря claim — та же `state.rs`, ветка `TxBody::BurnMark`.
- **`Claim`:** отдельный явный путь: расчёт `matured`, режимы Free/Paid, лимит дня на free, обновление `marks` и claim-state — ветка `TxBody::Claim` в том же файле.
- **`Transfer` / `Export` / `Import`:** движение PWM, комиссии, реестры; **материализация marks не выполняется**.

### Всплеск marks после stake

Ожидаемо при ненулевом `matured_units_available` до stake: на шаге `Stake` сначала отрабатывает `apply_auto_claim` (добавляет накопленную по формуле зрелость к `marks`), затем меняется стейк; `last_claim_unix_time` выставляется в время блока, что сбрасывает «окно» часов для следующей зрелости. Тест на отсутствие эффекта при нулевой зрелости: `stake_autoclaim_zero_matured` в `state.rs`.

### Burn и «невидимая» зрелость

Накопленная по времени **не materialized** дельта живёт только в комбинации полей (`staked`, `last_claim_unix_time`, время блока) и **не увеличивает** `marks` до явного `ClaimTx` или auto-claim при Stake/Unstake. **Burn списывает только текущее поле `marks`**, не забирая «незрелое» напрямую (оно не в `marks`).

Итог по коду: наблюдение владельца **согласуется с реализацией** — это не случайный пропуск в одной ветке относительно другой; burn намеренно узкий.

---

## Design alignment

### Нормативные документы (V2 / RFC / white spec)

- **RFC 0012** (*Claim maturity and state model*, активный пакет V2-1) задаёт auto-claim явно: материализация неявно только в составе **«релевантной stake-management транзакции»** в рамках **stake/claim контура**, либо через явную `ClaimTx`. Free-day лимит относится к **явному** claim; auto-claim отдельный free-slot не потребляет.
- **WHITE_SPEC v0 / v0-en §9.3** дублирует продуктовый контракт: два пути — explicit `ClaimTx` и auto-claim как эффект **relevant stake-management transaction**; привязка к stake lifecycle (`STAKE`/`UNSTAKE`).
- **CHANGELOG / mvp_v2** формулирует implemented behavior как auto-claim при stake-management при ненулевой зрелости (согласовано с кодом).

**Вывод по согласованности:** текущий код **выровнен** с RFC 0012 и WHITE_SPEC по **узкой** модели auto-claim. `BurnMark` в этих источниках описан как списание **`marks`**, а не как триггер materialization — противоречия коду не выявлено.

### Замечание о формулировке RFC 0011

В **RFC 0011** auto-claim текстом отнесён к транзакции, «меняющей баланс монет или марок». Буквально burn **меняет марки**, что теоретически даёт двусмысленность; **устраняющая трактовка** — уточнение из RFC 0012 («stake-management», stake/claim контур), с которой **фактическая реализация совпадает**. При желании убрать путаницу — точечная редакторская правка RFC 0011 в пользу явного перечня tx kinds (не блокер для текущего кода).

---

## Options (A / B)

### A) Оставить как есть (рекомендуется как baseline)

**Плюсы**

- Соответствие активной спецификации и существующим тестам/ожиданиям экономики (явный claim с лимитом дня; auto-claim без free-slot только на stake ops).
- Предсказуемость: материализация привязана к **stake lifecycle** и сознательному **Claim**, а не к любому движению счёта.
- Меньше спорных edge-case’ов с порядком эффектов (burn после materialization в одном transition, взаимодействие с anchor / continuity).

**Минусы / UX**

- Оператор видит «скачок» marks после stake без отдельного claim — нужно понимание модели.
- Burn не «подтягивает» зрелость: пользователь может недооценивать доступный для сжигания запас, если смотрит только на текущие `marks` без учёта накопленной зрелости.

**Доработки без смены протокола**

- Подсказки в TUI/CLI: «Сжигание использует только уже materialized marks; накопл. зрелость — через Claim или изменение стейка».
- При необходимости — отображать **оценку matured** рядом с `marks` (read-only из той же формулы, что core), без изменения консенсуса.

### B) Расширить протокол: auto-claim на любые tx с адресом (включая burn)

**Плюсы**

- Единообразное ощущение «баланс marks всегда актуален» после любой операции.
- Burn в одной транзакции мог бы учитывать только что materialized дельту (если зафиксировать порядок: сначала materialization, затем debet).

**Минусы / риски**

- **Экономика и лимиты:** auto-claim **не потребляет** daily free-slot и **не требует** платы за явный paid claim — расширение триггера на Transfer и др. умножает пути materialization **в обход** явного claim-политики; потребуется явное решение: это намеренно или нужны новые ограничители/метрики.
- **Накладные расходы:** формула дешёвая, но частота вызовов растёт на каждый tx по аккаунту (профиль нагрузки / mempool всё же стоит оценить для целевого масштаба).
- **Семантика anchor / continuity:** чаще чем сейчас обновляются `last_claim_anchor_ref` / `last_claim_unix_time` без отдельной `ClaimTx` — кошельки и диагностика должны оставаться согласованными с RFC по monotonic anchor и preflight/apply parity (см. известные зазоры в обзорах RFC 0013/0014 для auto-claim reject surface).
- **Непредвиденный рост `marks`** при частых транзакциях — не «эмиссия», а перенос из time-accrued в поле `marks`; продуктово может выглядеть как «инфляция видимых marks» относительно привычки «только после F5 claim».

**Rollout (минимальный безопасный контур, если выбирать B)**

1. **RFC / ADR:** явно перечислить tx kinds с auto-claim; порядок под-эффектов в одном transition; влияние на free/paid claim narrative.
2. Реализация в одном месте (`apply_tx_with_ctx` или общий pre-hook) + **широкие golden-тесты**: порядно tx в блоке, burn с нулевыми/ненулевыми matured, сочетание со Stake в том же блоке.
3. Версионирование ноды / feature flag только если нужна поэтапная сеть; иначе hard fork правил state.

---

## Recommendation

- Зафиксировать для команды и тестеров: **наблюдение владельца — ожидаемое поведение по дизайну (RFC 0012 + WHITE_SPEC §9), не консенсус-рассинхрон** между burn и stake.
- **Практический путь A:** сохранить протокол, усилить **UX и доки** (опционально показ matured estimate), при необходимости слегка уточнить RFC 0011 против RFC 0012.
- Переход к **B** оправдан только при согласованном продуктовом решении «materialize on every touch» и пересмотре claim-политики/доков; иначе это **RFC-CHANGE** с полным циклом спецификации и регрессионным пакетом.

---

## Verdict

**INFO** — реализация согласована с основной нормативной моделью (auto-claim только stake-management + explicit Claim; burn не триггерит recalc). Расширение auto-claim на burn/transfer и т.д. — **не багфикс, а преднамеренное изменение протокола (RFC-CHANGE)**, если его примут.

---

## Participation / token estimate (orchestrator)

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/20260507-marks-materialization-semantics.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 9000
  confidence: low
```
