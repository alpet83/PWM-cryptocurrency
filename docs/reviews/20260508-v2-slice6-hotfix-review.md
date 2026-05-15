# Final gate — mini-hotfix Wave A (strict hash / tip_hash divergence)

**Тикет:** `tasks/20260508-v2-slice6-hotfix-tip-hash-divergence.json`  
**Входы:** `docs/reviews/20260508-v2-slice6-tip-hash-divergence-diagnosis.md`; заявленный `docs/reviews/20260508-v2-slice6-hotfix-testing.md` (**отсутствует в дереве**). Верификация: **`scripts/wave_a_same_shard_stop.py`**, **`docs/runbook-same-shard-sync-v1.md` §6**, заметки делегации **`pwm-coding`** в тикете.

---

## 1. Scope recap

- **Цель:** убрать **ложнозелёный** Wave A при расхождении chain-identity по хэшам (manifest `tip_hash`, байты последнего `block_e*.json`), не смешивая это с приёмкой «совпали балансы/структура манифеста», и **не** выдавать исправление live-консенсуса за один mini-hotfix.
- **Заявленный hotfix (pwm-coding):** усилить harness exit semantics + runbook; без изменения runtime PoA/seal в этом slice.

---

## 2. Requirements fit

**Немедленный риск false-green Wave A — закрыт на уровне gate:**

- В `scripts/wave_a_same_shard_stop.py` после записи отчёта при **`not tip_hash_equal` или `not epoch_hash_eq`** вызывается `print_hash_divergence_diag`, затем **`raise RuntimeError`** с перечислением причин (`tip_hash_equal=false`, `last_epoch_hash_equal=false`). Это даёт **ненулевой exit** и останавливает сценарий «JSON красивый / exit 0 при разных байтах идентичности цепочки».
- Отчёт JSON обогащён **checkpoint_height** в ветках `node1`/`node2` для диагностики — согласуется с текстами заметок в тикете.

**Соотношение с диагнозом:**

- Диагноз требовал как минимум **FAIL при `last_epoch_hash_equal=false`**; реализация делает это и **дополнительно** фейлит при **`tip_hash_equal=false`**. Это **не слабее** критерия «strict hash» и соответствует runbook (оба поля — обязательные для PASS).

**Документация оператора:**

- `docs/runbook-same-shard-sync-v1.md` §6 обновлён: **PASS** требует `last_epoch_hash_equal=true` и `tip_hash_equal=true`; **FAIL** явно включает оба вида hash mismatch. Нарратив согласован с кодом harness.

**Трассируемость тест-прогона:**

- Файл **`docs/reviews/20260508-v2-slice6-hotfix-testing.md`** не найден; делегация **pwm-testing** в тикете всё ещё ожидает формальное подтверждение smoke **после** hotfix. Ревью опирается на **статическую верификацию** и согласованность с заметками pwm-coding, а не на независимый post-fix отчёт pwm-testing — **нит** (см. §7).

---

## 3. Style and module shape

- Продуктовый Rust не входил в diff ревьюера; именование Python-хелперов умеренное (`print_hash_divergence_diag` в пределах разумного для скрипта).

---

## 4. Safety

- Изменения ограничены **локальным** harness и документацией; доверенные сетевые поверхности не расширяются.
- Явный `RuntimeError` вместо молчаливого PASS снижает риск **операторской** ложной уверенности без вмешательства в консенсус-логику в ноде.

---

## 5. Tests

- **Отчёт pwm-testing** по самому hotfix отсутствует — рекомендуется один прогон Wave A и запись **`docs/reviews/20260508-v2-slice6-hotfix-testing.md`** (ожидание: при наблюдаемой ранее дивергенции хэшей — **ненулевой exit**, stderr с блоком диагностики).
- Исторический прокси **`docs/reviews/20260508-v2-8-slice6-testing.md`** описывает **старое** поведение (exit 0 при hash mismatch) и **не заменяет** post-fix smoke.

---

## 6. Consensus scope — no over-claim

- Тексты в тикете (делегация pwm-coding) и суть изменений **явно ограничивают** scope: **только** операторский gate Wave A + runbook; **«Runtime/consensus поведение не менялось»** — принимается; недетерминизм `ts`/seal в core остаётся **отдельным** продуктовым треком, как в диагнозе.
- Ревью подтверждает: hotfix **не утверждает** устранение **underlying consensus divergence**, только **прекращает маскировку** chain-identity расхождения зелёным exit в Wave A.

---

## 7. Verdict

**approve with nits** — для оркестратора: **`PASS_WITH_NITS`**.

**Nits:**

1. Закрыть разрыв доказательств: **`pwm-testing`** — зафиксировать **`docs/reviews/20260508-v2-slice6-hotfix-testing.md`** после прогона на текущем harness.
2. Держать в бэклоге продуктовый трек из диагноза (детерминированный seal ts / proposer policy), если цель — одна байтовая цепь в testnet, **вне** этого mini-hotfix.

---

## 8. Participation / token estimate

```
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260508-v2-slice6-hotfix-review.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 6500, "confidence": "low" }
```

---

## 9. Git handoff (оркестратор)

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-slice6-hotfix-review.md'
git add 'tasks/20260508-v2-slice6-hotfix-tip-hash-divergence.json'
git commit -m 'docs(slice-6): Wave A hotfix gate review and ticket close'
```
