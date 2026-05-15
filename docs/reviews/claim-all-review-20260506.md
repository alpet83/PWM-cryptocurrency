# Review: CLAIM_ALL sentinel + TUI F5 marks modal (commit `35f02e8`)

**Verdict:** REQUEST CHANGES (см. §6 и таблицу AC).

## 1. Scope recap

Коммит вводит константу `CLAIM_ALL` (`u64::MAX as u128`) для `TxBody::Claim`, обработку на ноде через подстановку «все созревшие юниты», флаг `--all` / `claim_units == 0` в CLI, TUI: модалка F5 (claim all / burn), HTTP submit claim с `ClaimMode::Free`, тест в `pwm-core` и правки тестовых фикстур TUI.

## 2. Requirements fit

- **Протокол / core:** подстановка `effective_units` для сентинела согласована с целью «клиент не считает maturity»; начисление идёт по `effective_units`.
- **CLI:** маршрутизация `all || claim_default_zero → CLAIM_ALL` покрывает типичный UX.
- **TUI F5:** модалка и горячие клавиши присутствуют; **часовой guard заявлен, но на практике не сохраняется** (см. §4 Safety / checklist 3).

## 3. Style and module shape

- Новые модули с кратким `//!` на английском (`marks_modal.rs`, расширенный `tx_submit.rs`).
- Имена в затронутых путях укладываются в политику ≤4 сегментов для prod.

**Автоматизация:** `python scripts/check_rust_fn_name_segments.py` по перечисленным путям — **violations пустые.**

## 4. Safety

- **Сентинел и состояние:** при `CLAIM_ALL` после подстановки `effective_units == matured`, проверка `effective_units > matured` для этого случая недостижима; для явного `claim_units` — по-прежнему актуальна.
- **`effective_units == 0`:** возвращается `ClaimOverMatured` (то же имя, что и при «запросили больше созревшего»). Семантика перегружена: клиентам RPC/CLI может быть неочевидно отличить «нечего клеймить» от «перебор» — желательно отдельная ошибка или документированное соглашение.
- **`validate_tx_shape`:** ноль по-прежнему отклоняется (`ClaimDeltaInvalid`); `CLAIM_ALL` ненулевой — проходит ветку «> 0».
- **`anchor_ref` в TUI:** берётся как `parse_head_height(&ui.head).unwrap_or(0)`, где `ui.head` формируется из `/v1/head` как `height=… tip=…`. При нормальном опросе парсер находит высоту. Если строка head не в ожидаемом виде или RPC head недоступен (`…`, timeout/offline), подставляется **0**. Для аккаунта с прошлым claim состояние требует `anchor_ref >= last_claim_anchor_ref`; **0 обычно даст отказ ноды** (`ClaimAnchorRangeInvalid`), а не silent корректный claim. Это не дыра консенсуса, но **плохой UX и риск ложных отказов** при деградации head.

**Критично — часовой guard (1 h):**

- `MarksModal::can_claim` реализован как «≥ 3600 секунд с `last_claim_wall`» — локально корректно.
- После `RpcEvent::ClaimOk` в `last_claim_wall` пишется `Instant::now()` в соответствующую строку `ui.rows`, но при каждом `RpcEvent::PollDone` выполняется **`ui.rows = snapshot.rows`**, а `poll_snapshot` **всегда** задаёт `last_claim_wall: None` для всех аккаунтов. Опрос идёт ~раз в секунду. Итог: метка почти немедленно затирается следующим poll; повторное открытие F5 **не видит** часовой интервал. Заявленный «1-hour guard» в текущем виде **не работает**.

## 5. Tests

- **Core:** есть `claim_all_sentinel_all_matured` — покрывает успешный apply с `CLAIM_ALL`.
- **Пробелы:** нет регрессии на TUI merge `last_claim_wall` при poll; нет сценария «head недоступен → anchor_ref».

## 6. Verdict

**REQUEST CHANGES:** требуется починка сохранения `last_claim_wall` при обновлении снапшота (merge по `id` или поле из API), иначе поведение расходится с описанием фичи и чеклистом. Опционально: отдельная ошибка для «ноль созревшего» при `CLAIM_ALL`, и запрет/явное сообщение при `anchor_ref == 0` если известно, что аккаунт уже клеймил.

---

## AC table

| AC | Описание | Результат |
|----|-----------|-----------|
| AC1 | `effective_units == 0` обработан; `> matured` после сентинела невозможно; shape validation | **PASS** с nit: `ClaimOverMatured` для нуля — перегруз смысла |
| AC2 | `anchor_ref` в submit: не 0 при живом head; fallback 0 и монотонность | **PASS-WITH-NITS:** консенсус безопасен (отклонение), UX/edge при мёртвом head |
| AC3 | `can_claim` ≥3600s; обновление на `ClaimOk`; guard при переоткрытии F5 | **FAIL:** poll затирает `last_claim_wall` |
| AC4 | Free-claim daily limit vs wall 1h | **PASS** с nit: TUI 1h не синхронизирован с UTC-day лимитом ноды; возможен отказ после истечения 1h |
| AC5 | Wallet locked до nonce | **PASS:** `signing_material_for_sender` возвращает явное сообщение про lock |
| AC6 | `check_rust_fn_name_segments` | **PASS:** нарушений нет |

**Итог по воротам:** **FAIL** (блокер AC3).

---

## Participation / token estimate

```yaml
agent: pwm-review
result: FAIL
artifacts: docs/reviews/claim-all-review-20260506.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: low
```

## Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/claim-all-review-20260506.md'
git add 'tasks/20260506-claim-all-sentinel.json'
git commit -m 'docs: claim-all pwm-review report'
```

---

## Re-review after fix ae15b70

**Фикс-коммит:** `ae15b70` — `fix(claim): preserve last_claim_wall across polls + anchor_ref from head`.

### Scope recap (повторная проверка)

Патч добавляет `merge_poll_rows` перед заменой `ui.rows` при `PollDone`, поле `head_height` в `PollSnapshot`/`Ui`, `pick_claim_anchor` с fallback на последнюю успешную высоту, подсказку в `decorate_claim_error` при отклонении якоря, предупреждение в stderr при `anchor_ref == 0`, юнит-тест `keeps_claim_wall_two_polls`.

### Requirements fit (spot-checks)

1. **AC3 / merge:** `merge_poll_rows` для каждой строки нового снапшота ищет в `current_rows` запись с тем же `id` и переносит `last_claim_wall`. Сопоставление по идентификатору аккаунта корректно. Тест **не** прогоняет полный `RpcEvent::PollDone`, а изолированно проверяет два вызова `merge_poll_rows`: после первого merge обновлённый «текущий» ряд получает метку времени, после второго merge новый снапшот с `None` снова обогащается — `assert_eq!(second_poll_rows[0].last_claim_wall, Some(mark))`. Утверждение **non-None** после имитации «двух опросов» выполнено; регресс по затирачу poll устранён по смыслу фикса.

2. **`pick_claim_anchor`:** при ненулевом результате `parse_head_height(&ui.head)` возвращается он; иначе — `ui.head_height.unwrap_or(0)`. `head_height` в UI обновляется в `PollDone` только если `snapshot.head_height` — `Some`, то есть после успешного `/v1/head` в `poll_snapshot`; при ошибке head предыдущее значение **не** затирается — fallback логичен для кратковременной порчи строки `head` при сохранении последней известной высоты.

3. **Край `anchor_ref == 0`:** если строка head не парсится и `head_height` ещё ни разу не был успешно записан, возвращается 0; в лог попадает предупреждение, при отклонении клейма — расширенное сообщение с «Retry after next poll.» Согласуется с приемлемым UX при холодном старте / деградации RPC.

### Style

`python scripts/check_rust_fn_name_segments.py crates/pwm-tui/src/tui_loop.rs crates/pwm-tui/src/account_view.rs` — **violations пустые.** Имена в объёме проверки в пределах политики.

### Safety / тесты

Сохранение `last_claim_wall` при опросе устраняет описанный ранее сбой часового guard в TUI. Остаются **вне данного фикса** смежные замечания первого раунда (перегруз смысла `ClaimOverMatured` при нуле, расхождение wall 1h с дневным лимитом ноды), если продукт захочет их адресовать отдельно.

### AC table (после ae15b70)

| AC | Описание | Результат |
|----|-----------|-----------|
| AC1 | `effective_units` / shape / ноль | **PASS** с nit (как ранее): семантика `ClaimOverMatured` для нуля |
| AC2 | `anchor_ref`; fallback | **PASS** с nits: парсинг head + `head_height`; stale height не сбрасывается при сбое head |
| AC3 | guard ≥3600s, устойчивость к poll | **PASS:** merge по `id` + тест на два цикла merge |
| AC4 | Free-claim лимит vs 1h wall | **PASS** с nit (как ранее) |
| AC5 | Wallet locked | **PASS** |
| AC6 | `check_rust_fn_name_segments` | **PASS** |

### Verdict

**APPROVE WITH NITS** — блокер AC3 снят; остаточные nits не связаны с коммитом `ae15b70` и касаются ранее зафиксированных продуктовых нюансов.

---

## Participation / token estimate (re-review ae15b70)

```yaml
agent: pwm-review
result: PASS-WITH-NITS
artifacts: docs/reviews/claim-all-review-20260506.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 5500
  confidence: low
```

## Git handoff for orchestrator (re-review)

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/claim-all-review-20260506.md'
git add 'tasks/20260506-claim-all-sentinel.json'
git commit -m 'docs: claim-all re-review PASS'
```
