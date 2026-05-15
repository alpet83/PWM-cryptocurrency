# Раследование: F5 → `E_CLAIM_OVER_MATURED` при `CLAIM_ALL` (V2-7 marks)

## Заключение

NEED-FIX — `CLAIM_ALL` при нуле созревших единиц отклоняется как `ClaimOverMatured` из-за явной проверки `effective_units == 0` в `State::apply_tx` (ветка `Claim`); подстановка `CLAIM_ALL → matured` выполняется до этой проверки, поэтому гипотеза «`u32::MAX` не заменяется» не подтверждается.

## Root cause(s)

1. **Главная причина симптома.** В `crates/pwm-core/src/state.rs`, обработчик `TxBody::Claim`: после вычисления `matured` поле `effective_units` для `CLAIM_ALL` равно `matured`. Если созревших единиц нет (`matured == 0`), код сразу возвращает `Err(TxError::ClaimOverMatured)` по ветке `if effective_units == 0` (строки ~224–231). Это воспроизводит HTTP/RPC код `E_CLAIM_OVER_MATURED` через маппинг в `crates/pwmd/src/api/common.rs`. По смыслу это не «переклейм», а отсутствие созревшего объёма при том, что sentinel уже сведён к нулю.
2. **`CLAIM_ALL` обрабатывается до проверки «over».** Последовательность: вычислить `matured` → если `claim_units == CLAIM_ALL`, то `effective_units = matured` → затем ошибка при `effective_units == 0` или `effective_units > matured`. Гипотеза про «`CLAIM_ALL` не перехвачен и сравнивают `u32::MAX` с `matured`» для текущего кода не актуальна.
3. **Почему `matured` часто 0 после стейка, хотя «много блоков».** Функция `matured_units_available` (`state.rs`, ~408–415) возвращает 0, если `staked == 0`, если время блока не строго больше `last_claim_unix_time`, или если с момента `last_claim_unix_time` прошло меньше одного полного часа (`delta_seconds / 3_600` даёт 0 часов). При `Stake` состояние сбрасывает «окно»: после `apply_auto_claim` выполняется `a.last_claim_unix_time = block_unix_time` (~170). Пока с этого момента не набралось ≥1 часа wall-clock между последним клеймом/стейком и временем включающего блока клейма, созревший объём остаётся 0 при ненулевом стейке.
4. **TUI усиливает восприятие как ошибку.** В `crates/pwm-tui/src/tui_loop.rs` (~576–582) F5 вызывает `submit_claim(..., CLAIM_ALL, ...)`. `submit_claim` (`crates/pwm-tui/src/tx_submit.rs`, ~152–157) при не-2xx возвращает `Err` с текстом, включающим подсказку от `summarize_tx_reject_json`; при успехе — только `Ok(())` без сообщения «Claimed N marks», поэтому желаемая подсказка «0 марок — норма» отсутствует на успешном пути.

## Scope fix (что надо поправить)

1. **`crates/pwm-core/src/state.rs` — ветка `TxBody::Claim` (~224–257).** При согласованном с владельцем поведении разрешить **успешное** применение транзакции при `effective_units == 0` (особенно при `CLAIM_ALL`), не отдавая `ClaimOverMatured`; оставить `ClaimOverMatured` для явного запроса `claim_units ∉ {CLAIM_ALL}` с `effective_units > matured` (строго больше созревшего). Отдельно зафиксировать продуктовые правила: при нулевом клейме нужно ли увеличивать `nonce`, обновлять `last_claim_anchor_ref` / `last_claim_unix_time`, списывать ли платный сбор и **не тратит ли Free-режим дневной лимит** (`last_free_claim_utc_day`), если марок добавлено 0 — иначе F5/auto-claim может заблокировать бесплатный клейм на день.
2. **`crates/pwm-core/src/state.rs` — модульные тесты.** Расширить `claim_all_sentinel` или добавить тест: `CLAIM_ALL` при `matured == 0` ожидается успех с нулевым приростом `marks` (и согласованными полями якоря/времени по выбранной семантике).
3. **`crates/pwm-tui/src/tx_submit.rs` и/или `tui_loop.rs`.** Если RPC начинает возвращать тело с числом заклеймленных марок — отобразить `Claimed {N}`; если RPC остаётся без тела при успехе, можно показывать нейтральное «claim ok (0 marks)» после успешного HTTP, чтобы F5 не выглядел как сбой при нулевом созревании.

## Вердикт по accrue_marks

**Удалён из пер-поколения печати блока.** В `Chain::seal` (`crates/pwm-core/src/chain.rs`, ~85–118) нет вызовов `accrue_marks` / `accrue_marks_v2`; комментарий явно указывает, что марки в seal не копятся, а из genesis «посеяны». Методы `State::accrue_marks` и `accrue_marks_v2` в `state.rs` по-прежнему существуют как API состояния, но не вызываются из `seal`. **Genesis:** в `GenCfg::state0` (`crates/pwm-core/src/genesis.rs`, ~90–95) `marks = bal / PWM_RAW_SCALE` (с насыщением до `u32::MAX`), что соответствует описанию спринта.

## Дополнительные наблюдения

- **`crates/pwmd/src/tx_policy.rs`:** по поиску по символам claim в файле логики клейма не обнаружено; отклонение идёт из ядра при `apply_tx_with_ctx` (см. `lifecycle.rs` и снапшот-пути).
- **Тест `claim_all_sentinel`:** проверяет только сценарий с ненулевым `matured` (время 7200 с, ожидание +4 к `marks`); регрессии «ноль созревших при `CLAIM_ALL`» нет.
- **Именование ошибки:** для нулевого созревания при `CLAIM_ALL` использование `ClaimOverMatured` вводит в заблуждение; при фиксе можно оставить код только для реального переклейма `N > matured`.

---

## Участие / оценка токенов (для тикета)

- `agent`: `pwm-review`
- `result`: `FAIL` (поведение не соответствует согласованной цели «0 — успех»)
- `artifacts`: `docs/reviews/v2-marks-claim-investigation-20260507.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 8000, "confidence": "low" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/v2-marks-claim-investigation-20260507.md'
git commit -m 'docs(review): v2 marks claim F5 E_CLAIM_OVER_MATURED investigation'
```

**Verdict (one line for orchestrator):** NEED-FIX — remove or narrow `effective_units == 0` → `ClaimOverMatured` for `CLAIM_ALL`; align Free-claim side effects and TUI success messaging.
