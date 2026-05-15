# V2-8 Slice 6 — независимое ревью (Wave A / Phase A)

**Объект:** коммит `e74dbf3` (test-only `debug-stop-height`, harness `scripts/wave_a_same_shard_stop.py`, дополнения runbook).  
**Тикет:** `tasks/20260508-v2-sprint8-slice6-automated-waves.json`.  
**Вход testing:** заявлен `docs/reviews/20260508-v2-8-slice6-testing.md` — **в репозитории на момент ревью файл не обнаружен**; выводы по прогону опираются на код harness/runbook и статический анализ критериев PASS/FAIL.

---

## 1. Scope recap

Тикет формулирует для **всего слайса** минимум Wave A/B/C. Делегация **pwm-coding** закрывает **Phase A / Wave A** только:

- test-only останов по высоте (`debug_stop_height` в config/state/bootstrap, ветка в `lifecycle` после успешного seal);
- автоматический двухнодовый прогон с детерминированной остановью и проверками согласованности snapshot/epoch-manifest и ключевых аккаунтов;
- операторская секция Wave A в `docs/runbook-same-shard-sync-v1.md`.

Соответствие заявленному **Wave A** из `context.required_minimum[0]`: по смыслу **да** — две ноды одной шарды, транзакции, останов после минимум двух checkpoint-интервалов (через `max(stop_height, 2 * SNAP_CHK_BLK_IV)`), сравнение `canonical_h`, `checkpoint_height`, полей manifest, хеш последнего epoch-файла и полей sender/receiver.

**Узкое место формулировки тикета:** «синхронность мемпула» в harness выражена **косвенно** (отправка tx, лимит «pre-stop lag» по высотам головы), без явного сравнения содержимого мемпула или метрик gossip — это осознанно минималистичный smoke, не полный mempool-инвариант.

---

## 2. Requirements fit (достаточность Phase A как baseline)

**Для маркера «baseline Wave A complete» (только фаза A):** реализация **достаточна**, если независимый прогон pwm-testing подтверждён артефактом. Отсутствие в ветке файла `docs/reviews/20260508-v2-8-slice6-testing.md` **снижает доказательную базу** до обзора кода; оркестратору имеет смысл закоммитить/прикрепить отчёт тестирования.

**Wave B/C** в коммите **не заявлены** — для полного закрытия слайса по `required_minimum` они остаются впереди; это не дефект Phase A.

Итог по достаточности: **Phase A как автоматизированная база для двухнодового same-shard sync — принимаема с нитом про трассируемость testing-md и про неявную проверку мемпула.**

---

## 3. Style and module shape

- Запущен `python scripts/check_rust_fn_name_segments.py` по путям из артефактов (`main.rs`, `config.rs`, `state.rs`, `bootstrap.rs`, `lifecycle.rs`): **нарушений политики имён нет** (`violations: []`).
- Поверхностно: изменения в `lifecycle` локализованы вокруг seal/snapshot; test-only флаг снабжён предупреждением в лог — согласуется с ожидаемым testnet-паттерном.
- Детальный обзор «микромодульности» и баннеров для всех затронутых файлов не выявил блокеров по целям слайса.

---

## 4. Safety

- `debug_stop_height` — test-only путь остановки после успешного seal; публичная атака через флаг возможна только если оператор сам передаёт CLI — **приемлемо** для testnet harness.
- Harness поднимает локальные процессы и RPC; секреты только фиксированный Genesis-pass для тестового сценария — ок для smoke.
- Иных сетевых доверенных границ в диффе не добавлено.

---

## 5. Tests (наблюдения по покрытию)

- Логика критериев зашита в `wave_a_same_shard_stop.py`: жёсткий FAIL при расхождении `canonical_h`, manifest metadata, `checkpoint_height`, последнего epoch-файла (имя и sha256), ключевых полей аккаунтов; контроль pre-stop lag и кодов выхода `pwmd`.
- **`tip_hash_equal` не входит в условие FAIL** — только stderr-note и поле JSON-отчёта; с runbook это согласовано.
- Без приложенного `docs/reviews/20260508-v2-8-slice6-testing.md` **нет** независимого подтверждения фактического PASS на CI/хосте ревьюера.

---

## 6. Значение `tip_hash_equal=false` для приёмки same-shard sync

Контекст: после остановки сравниваются поля `tip_hash` из `pwm-epochs-manifest.json` на двух нодах.

- **По принятой модели приёмки Wave A** (runbook §6): успех опирается на совпадение `canonical_h`, инвариантов manifest, `checkpoint_height`, эффектов аккаунтов и байтово‑совпадающего последнего epoch-файла. **`tip_hash` выделен как отдельный диагностический индикатор** и **не отменяет PASS**, если перечисленные инварианты выполнены.
- **Инженерная смысловая нагрузка:** если при **`last_epoch_hash_equal=true`** окажется **`tip_hash_equal=false`**, это уже **аномалия уровня согласованности manifest с дисковым хвостом** (при идентичных epoch-файлах хеш наконечника цепочки должен согласовываться с содержимым) — такой случай стоит **эскалировать в pwm-coding** как потенциальный баг обновления manifest или гонки сохранения, даже если harness формально выходит с кодом 0.
- Если же расходится только `tip_hash`, но **расходятся и другие артефакты**, harness **упадёт** раньше по проверкам — приёмка sync не пройдёт.

**Вывод для приёмки:** **`tip_hash_equal=false` сам по себе не определяет FAIL по текущему контракту Wave A**, но при сочетании с равенством последнего epoch-файла это **сильный сигнал** для расследования, а не «шум».

---

## 7. Verdict

**approve with nits** (для оркестратора: **PASS_WITH_NITS**).

**Nits (приоритет):**

1. Закоммитить/приложить `docs/reviews/20260508-v2-8-slice6-testing.md`, чтобы закрыть разрыв доказательств.
2. Явно зафиксировать в тикете или runbook, что «синхронность мемпула» в Wave A = косвенная (tx + lag), если нужна договорённость с владельцем слайса.
3. При появлении стабильного `tip_hash_equal=false` при `last_epoch_hash_equal=true` — отдельный баг/расследование, не игнорировать как косметику.

---

## 8. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260508-v2-8-slice6-review.md
  - input_testing_expected: docs/reviews/20260508-v2-8-slice6-testing.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 8000
  confidence: low
```

---

## 9. Git handoff (оркестратору)

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-8-slice6-review.md'
git add 'tasks/20260508-v2-sprint8-slice6-automated-waves.json'
git commit -m 'docs(slice-6): Wave A independent review and ticket traceability'
```
