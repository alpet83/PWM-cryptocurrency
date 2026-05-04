# Ревью: деградация после межшардового Import, сбои персиста после рестарта, согласованность

Простой пересказ для оператора (глоссарий, мини-сценарии): [`sprint-15-operators-cross-shard-persist-consistency-review-20260503-plain.md`](./sprint-15-operators-cross-shard-persist-consistency-review-20260503-plain.md).

**Статус:** независимый обзор по запросу оператора (pwm-review).  
**Контекст:** двухузловая схема; целевой шард после межшардового Import; после рестарта — ошибки сохранения блоков.

## 1. Scope recap

**Цель:** ответить на три вопроса (нуль vs чекпоинт, очистка «битых» данных при ресете, момент детектирования деградации относительно сохранения), проследить пути кода (**`InitState`**, degraded recovery, snapshot save, seal loop, Import / **`relay_import`**, **`handoff_register`**) и отделить **фактическое поведение** от **желаемого** для бэкендов, где рестарт «с нуля» не приемлем.

Связанный контекст: hotfix межшард + snapshot/handoff — `tasks/20260502-s15-hotfix-do-chain-reset-after-xshard.json`; модель персиста — `docs/guide-node-storage-and-snapshot.md`, `docs/pwmd.md`.

## 2. Трассировка кода и ответы на вопросы оператора

### 2.1. `InitState`, «готовность» и degraded

Фазы в **`crates/pwmd/src/state.rs`**, включая **`ReadyDegraded`**. **`InitState::is_ready`** считает готовым и **`Ready`, и `ReadyDegraded`** — HTTP может считать узел «готовым», пока персист неверен.

Ставят **`ready_degraded`**, в частности:

- **`spawn_snapshot_loader`** (**`lifecycle.rs`**): ошибка загрузки, ошибка **`into_runtime`**, и после успешного seal при ошибке autosnapshot (**`apply_snapshot_init_state`**).
- **`persist_snapshot_or_http_err`** (**`api/common.rs`**): ошибка записи после HTTP-мутации → degraded.
- **`relay_import`** (**`relay.rs`**): после **`mark_import_by_export`** при ошибке **`save_tip_summary`** → degraded без отката памяти.

### 2.2. Вопрос 1 — почему «сброс» уходит в ноль, а не в «ближайший checkpoint»?

**Фактическое поведение:** отдельной операции «открутиться к последнему согласованному checkpoint на диске как к единственному источнику истины при сбое» **нет**. Режимы загрузки в доках описывают **чтение при старте**, не **автоматический rollback** после ошибок коммита в памяти.

Причины «видимости нуля»:

- При **ошибке load** узел переходит в **`ready_degraded`**, **`inner.chain` не заменён снапшотом** — остаётся состояние после genesis-bootstrap (**`app_from_genesis_shard_identity`**), т.е. как после «нуля» относительно ожидания восстановления с диска.
- Удаление файлов оператором ведёт к genesis-only старту.
- **Нет** встроенного «rewind только summary до `checkpoint_height`» без ручной процедуры (follow-up: checkpoint rewind / журнал воспроизведения).

### 2.3. Вопрос 2 — почему reset не удаляет «повреждённые локальные данные» до рестарта?

**Фактическое поведение:** при **`ready_degraded`** узел **не** санитизирует каталог снапшота. Рестарт повторяет загрузку тех же файлов. Очистка — **ручная** или вне узла.

### 2.4. Вопрос 3 — почему деградация не обнаружена *до* сохранения блоков после Import?

Траектории разные:

1. **Локальный `/v1/tx` Import/Export:** после **`chain.seal`** — сохранение; при ошибке есть **`rollback_commit`** в ряде веток (**`handlers_tx`**).

2. **`relay_import`:** мутации **`mark_import_by_export`** в памяти, затем **`save_tip_summary`**. При ошибке — degraded, **без отката**.

3. **`v1_export_handoff_register`:** вставка в **`exported_registry`**, **`cross_shard.record_handoff`**, затем **`persist_snapshot_or_http_err`**. При ошибке память уже изменена; **`persist_snapshot_or_http_err`** только ставит degraded и возвращает 500 (**нет `take_bak`/`rollback`** в этом хендлере).

4. **`spawn_seal_loop`:** при **`is_ready()`** (включая **degraded**) выполняется **`chain.seal`**; при успешном seal и ошибке **`save_seal_persist`** — **`apply_snapshot_init_state`** ставит degraded **без отката tip**.

**Итог:** нет единого инварианта «нет durable ack → не поднимать canonical tip».

## 3. Requirements fit (согласованность без костыля «перезапусти цепь»)

**Текущее поведение** допускает **RAM ahead of disk** и продолжение seal при **`ReadyDegraded`** (**`is_ready`** истинен для degraded).

**Желаемое:** DurabilityGate; при ошибке записи — откат до последней марки или HALT read-only; различать health vs accepts writes.

## 4. Bugs / gaps

| ID | Область | Суть |
|----|---------|------|
| G1 | `lifecycle::spawn_seal_loop` | Успешный `seal` + ошибочный autosnapshot без отката tip |
| G2 | `handlers_roaming::v1_export_handoff_register` | Мутация state до persist; ошибка → degraded без отката |
| G3 | `relay::relay_import` | Мутация roaming + fail save без отката |
| G4 | `InitState::is_ready` + **`ensure_user_tx_allowed`** | degraded всё ещё проходит **`ensure_ready`** → пользовательские tx разрешены |
| G5 | Ops | Нет checkpoint rewind / восстановления без ручного wipe |

## 5. Рекомендуемые follow-up

1. Durability invariant + seal loop HALT или rollback (G1).
2. Транзакционность handoff + relay marks или откат (G2, G3).
3. Разделить readiness: writes_enabled ≠ degraded (G4).
4. Операторский или автоматический rewind к последнему атомарному checkpoint (журнал).
5. Golden failure tests для сценариев §4.

## 6. Verdict

**Request changes.** Полевая деградация согласуется с размытой семантикой durable commit между seal loop, handoff_register и relay_import.

---

## Participation / token estimate

- **agent:** pwm-review  
- **result:** PASS (content); файл записан оркестратором из handoff субагента  
- **tokens:** estimate ~6500, confidence medium  

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/sprint-15-operators-cross-shard-persist-consistency-review-20260503.md'
git commit -m 'docs(slice-s15): cross-shard persist and degraded recovery review'
```
