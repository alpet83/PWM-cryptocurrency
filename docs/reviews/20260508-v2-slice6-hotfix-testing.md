# Wave A strict hash gate — testing report (mini-hotfix)

**Тикет:** `tasks/20260508-v2-slice6-hotfix-tip-hash-divergence.json`  
**Цель:** подтвердить, что после hotfix в `scripts/wave_a_same_shard_stop.py` расхождение chain-identity по хэшам даёт **ненулевой exit** и **читаемую диагностику**, без регрессии точки входа CLI.

**Проверенный harness:** коммит `36823f3` (`fix(testing): make Wave A fail on tip/epoch hash divergence`), дерево поверх `9f94908` (док-ревью).

---

## 1. Статическая проверка пути выхода

При **`not tip_hash_equal` или `not epoch_hash_eq`** после записи `wave-a-report.json` вызывается `print_hash_divergence_diag`, затем `RuntimeError` с явным перечислением причин — перехватывается в `main`, **`sys.exit(1)`**.

Ожидаемое поведение «hash divergence ⇒ FAIL» согласовано с `docs/runbook-same-shard-sync-v1.md` §6.

---

## 2. Исполнение harness (полный прогон)

| Проверка | Команда | Код выхода | Примечание |
|----------|---------|------------|------------|
| Полный Wave A | `python scripts/wave_a_same_shard_stop.py` | **1** | wall-clock **~404 s** |
| Регрессия CLI | `python scripts/wave_a_same_shard_stop.py --help` | **0** | парсер/баннер аргументов |

### 2.1 Ненулевой exit и диагностика (stderr)

В наблюдаемом прогоне на Windows (репо `P:\opt\docker\PWM-cryptocurrency`, бинарь `pwmd` из общего `rust-target-shared`) после остановки нод:

- В stderr напечатан блок **`=== Wave A hash divergence diagnostics ===`** с полями:
  - `tip_hash_equal`, `last_epoch_hash_equal`
  - `nodeA`/`nodeB`: `tip_hash`, `head_height` (= manifest `canonical_h`), `checkpoint`, хэши последнего epoch-файла
- Завершение: `wave-a failed: wave-a hash divergence: tip_hash_equal=false, last_epoch_hash_equal=false`

Интерпретация: на текущем недетерминированном `ts`/заголовке tip **оба** индикатора расходятся между нодами — gate срабатывает как задумано (нет false-green).

Полный JSON отчёта на успешном пути в этом прогоне **не печатался в stdout** (ошибка до ветки «всё совпало»), что ожидаемо при FAIL.

---

## 3. Регрессия пути выполнения скрипта

- Импорт/старт, разбор аргументов, вызов `--help` — **без ошибок**.
- Остальная логика прогона (кошельки, genesis-build, два `pwmd`, tx, ожидание stop-height, чтение snapshot/manifest) отработала до пост-стоп проверок; падение произошло **только** на hash gate, с явным сообщением.

---

## 4. Риски и следствия для приёмки

- **Wave A остаётся красным** в средах с прежним поведением `Chain::next_apply_ctx` / wall-clock `ts`, пока не будет продукта или тестового профиля с согласованными заголовками — это **ожидаемо** после ужесточения gate, не дефект harness.
- Для зелёного Wave A потребуется отдельная работа **pwm-coding** (детерминизм seal / политика proposer), вне scope данного mini-hotfix.

---

## 5. Краткий вывод

- **FAIL при расхождении хэшей:** подтверждён (exit **1**, stderr diagnostics + итоговая строка `wave-a failed: …`).
- **Регрессия CLI:** не обнаружена (`--help` → 0).
