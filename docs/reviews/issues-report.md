# Issues Report

## 2026-04-26 — Sprint 13 Slice 0
- **Issue:** Отсутствовал файл `docs/reviews/issues-report.md` для фиксации процессных граблей.
- **Workaround:** Файл создан в рамках Slice 0; далее использовать как единый журнал process/issues по Sprint 13.
- **Impact:** Низкий, на реализацию кода не влияет; риск потери процессного контекста устранён.

## 2026-04-29 — Sprint 14 Slice 17 remediation2
- **Issue:** В `pwmd` найден потенциальный deadlock из-за инверсии порядка lock (`app.init -> app.inner` в `/v1/status` и `app.inner -> app.init` в snapshot persist/lifecycle).
- **Root cause:** `init` и `inner` удерживались одновременно в разных путях с противоположным порядком при конкурентных `status` + `tx/finalize/seal`.
- **Fix:** Snapshot-save остался под `inner`, но обновление `init` перенесено строго после `drop(inner)`; `/v1/status` теперь читает `init` в локальные значения и отпускает lock до чтения `inner`.
- **Follow-up:** Для всех новых API/lifecycle путей сохранять правило: не держать `app.inner` и `app.init` одновременно; при persist сначала завершать работу под `inner`, потом отдельным шагом обновлять `init`.
