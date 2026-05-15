# Review: cluster Attester seal-role derivation (RFC16 раздел 8.2)

**Ticket:** `tasks/20260513-slice-cluster-attester-seal-derive.json`  
**Reviewer:** pwm-review  
**Date:** 2026-05-12

## 1. Scope recap

Слайс закрывает цели тикета: при `cluster.enabled` и `ClusterRole::Attester` локальный периодический seal не должен конкурировать с committer; поведение не должно опираться только на `--debug-disable-seal-loop`. Заявленные артефакты: `derive_seal_role` в `lifecycle.rs`, fail-fast в `validate_cluster_cfg` (`config.rs`), подсказка в CLI (`main.rs`), обновление `cy-cluster-attester.ps1`, регрессионные тесты. Опорная спецификация: RFC16 раздел 8.2 в `docs/rfc/16-validator-clone-attestation.md` (информативный подраздел про выравнивание pwmd).

## 2. Requirements fit

**Соответствие RFC16 раздел 8 (и 8.2):** Реализация согласована с нормативной таблицей раздела 8 (attester не должен выполнять финальный competing seal) и с примечанием 8.2: для включённого кластера и роли Attester роль печати выводится в standby без обязательного debug-флага; флаг остаётся осмысленным для вне-кластерных follower/replay сценариев.

**Порядок в `derive_seal_role`:** Сначала явный `seal_role_override`, затем ветка «кластерный attester → Standby», затем `debug_disable_seal_loop`, иначе Active. Это согласуется с формулировкой RFC «после валидации seal-role»: комбинация attester + `--seal-role active` отсекается в `validate_cluster_cfg` до вызова `derive_seal_role` из `run_with`. Явный standby override для attester избыточен, но консистентен (первая ветка).

**Edge cases (по запросу ревью):**

- **Proposer:** ветка attester не срабатывает; поведение по флагу и active/standby как раньше — в рамках заявки слайса (proposer «не трогать»).
- **Follower (кластер выкл.):** `cy-cluster-follower.ps1` по-прежнему передаёт `--debug-disable-seal-loop` — соответствует RFC разделу 8.2 (non-cluster).
- **Кластер выкл., но `--cluster-role attester` в CLI:** `validate_cluster_cfg` при `!enabled` не нормирует роль; `derive_seal_role` не переводит в standby. Это пограничный операторский footgun вне явного scope «включённый кластер»; в тексте RFC акцент на `cluster.enabled`. Имеет смысл держать в mind для будущего hardening, не блокер данного слайса.

**Скрипт лаборатории:** `cy-cluster-attester.ps1` больше не передаёт `--debug-disable-seal-loop`; комментарий в `cy-cluster-attester.ps1` ссылается на раздел 8.2 — ок.

## 3. Style and module shape

Идентификаторы в затронутых путях проверены скриптом `scripts/check_rust_fn_name_segments.py` (`lifecycle.rs`, `config.rs`, `main.rs`): нарушений политики (prod ≤4 слов) нет. Новых крупных блоков в `main.rs`/`lib.rs` ради этого слайса не наблюдается. Подсказка clap для `--debug-disable-seal-loop` на английском, с отсылкой к RFC16 разделу 8.2 — уместно.

**Nit (документация скриптов, не Rust):** В `cy-cluster-common.ps1` в шапке по-прежнему написано, что attester должен использовать `--debug-disable-seal-loop` в launcher — это противоречит обновлённому слайсу и `cy-cluster-attester.ps1`. Рекомендуется выровнять комментарий (отдельная мелкая правка документации в оркестраторе / следующем chore).

## 4. Safety

Изменения конфигурационные и детерминированные: ранний отказ при недопустимом override снижает риск двух активных sealer в роли attester. Не видно новых panics/unwrap в горячих путях именно в этом диффе. Границы доверия (RPC, wire) не расширялись.

## 5. Tests

- `config.rs`: `cluster_attester_rejects_active_override` покрывает fail-fast для active override.
- `lifecycle.rs`: `derive_role_attester_is_standby` фиксирует standby при attester без debug-флага.

Достаточно для регрессии заявленной логики. Опционально на будущее: тест, что при attester + явный `seal_role_override = Some(Standby)` результат остаётся Standby (избыточно по смыслу).

## 6. Verdict

**Approve with nits** — функционально слайс соответствует RFC16 разделу 8.2 и тикету; единственное заметное замечание — устаревший комментарий в `cy-cluster-common.ps1`.

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260513-cluster-attester-seal-derive-slice.md
token_usage: {"source": "estimate", "input": null, "output": null, "total": 8500, "confidence": "low"}
```

(Оценка по объёму прочитанных файлов и отчёта; точных счётчиков провайдера нет.)
