# S2.1 external lease backend — финальный gate (pwm-review)

**Дата:** 2026-05-09  
**Тикет:** `tasks/20260509-s21-external-lease-backend.json`  
**Проверенные коммиты (история репозитория):** `0977e8c03a487695ab2b6a7334f9680fc2928276` (feat: file lease backend), `7266d96785c9bd3e9644ab75903485d086701589` и `e53767b…` — артефакты тестового отчёта/тикета; префикс `62538c2…` из `commits[]` валиден как `62538c2e96db09bce049fb45adf3296be92d885e`.

## 1. Scope recap

Цель слайса: внешний координатор аренды для `single_sealer` при **двух независимых процессах с одним validator key**, закрытие процесс-локального зазора S2 через CAS и fail-closed сальвацию.

Реализовано по артефактам кодирования: трейт `LeaseBackend`, `FileLeaseBackend` (JSON + per-key `flock`/exclusive lock + tmp + rename), `ProcessLocalLeaseBackend`, интеграция в seal loop (`run_lease_gate`), конфигурация CLI/env, статус RPC.

## 2. Requirements fit

**Соответствует заявленному MVP:** для общего тома с разумными гарантиями блокировки и атомарного rename две копии `pwmd` читают/пишут одну запись по `validator_identity_hash`, перевыбор владельца через `takeover` с проверкой тройки `(term, fence, expiry)`, продление только при совпадении владения и живой `expiry`.

**Частично относительно формального acceptance-плана тикета:** автоматические тесты закрывают unit/CAS файл-бэкенда и однопроцессную симуляцию двух `LeaseRuntime`; пункты про **два процесса ОС**, **kill без release** и **индуцированные ошибки бэкенда end-to-end** остаются в основном для ручного/runbook-follow-up или отдельного harness (зафиксировано pwm-testing как PARTIAL).

**Граница продукта:** file-lock MVP не заменяет распределённый KV с TTL на стороне сервера для multi-host без общего тома — это согласовано с `artifacts.backend_recommendation.boundary`.

## 3. Style and module shape

- Модули с кратким англоязычным `//!`; разделение `lease_backend` / `lifecycle` выглядит уместно.
- `python scripts/check_rust_fn_name_segments.py` по `crates/pwmd/src/lease_backend.rs`, `lease.rs`, `lifecycle.rs`: **нарушений нет** (`prod_max=4`).
- Протокол wire не затрагивался — semver handshake вне области.

## 4. Safety

**Плюсы (fail-closed и CAS):**

- Любой `Err` из `acquire` / `renew` / `takeover` в `step_lease` снимает право на seal (`allow_seal = false`), состояние уходит в `FencedStandby` или синхронизацию standby; это согласовано с `run_lease_gate`, который **пропускает** тело seal loop только при `true`.
- `renew` и `release` завязаны на `(owner_id, term, fence)`; `takeover` — на точное совпадение записи наблюдаемого standby (включая `expiry`), затем монотонно растущие term/fence.
- Ядро split-brain по «две копии, один ключ»: при включённом `file` режиме и общем `seal_lease_dir` конкурирующие записи сериализуются файловым lock на ключ.

**Зоны риска (не блокеры MVP, но операторские):**

- Семантика блокировки и атомарности rename зависит от ФС/NFS/CIFS — вне кода; при слабом lock возможен гоночный регресс (известная оговорка для file-coordination).
- Время `now_ms` на узлах не синхронизируется кодом — логика `expiry`/takeover чувствительна к часам (для тестнет с общим томом часто достаточно).
- Отравление записи файла → `serde_json` error: трактуется как ошибка бэкенда → fail-closed (после успешной эскалации наблюдений оператор должен восстановить каталог leases).
- `ProcessLocal` остаётся явным режимом с **предупреждением в логе**, что для same-key multi-process защиты недостаточно — корректно с точки зрения прозрачности.

**Panics:** в прод-путях критичных unwrap не замечено; в тестах допустимы `expect` на часы/tempdir.

## 5. Tests

**Покрыто:** файл-бэкенд acquire/renew/takeover/release CAS; runtime takeover и блокировка «старого» владельца на `ProcessLocal` и симуляция с двумя `LeaseRuntime` на одном `FileLeaseBackend`; конфиг default `File` (`lease_backend_default_file`).

**Разрывы (подтверждены pwm-testing, не считаются ложными отчётами):**

- Нет контролируемого mock `LeaseBackend`, возвращающего `Err`, сквозь `step_lease`/`run_lease_gate` для жёсткого регресса «backend dead → ни одного seal».
- Нет автоматизации **двух процессов `pwmd`** с разными data-dir/ports (acceptance-план пункты 2–4).

Эти разрывы **не показывают дефекта в уже реализованной CAS-логике**, но **снижают уверенность регрессии** без follow-up integration harness.

## 6. Verdict

**Approve with nits** — качество MVP и модель безопасности соответствуют цели слайса; оставшиеся зазоры тестового контура и эксплуатационные зависимости от ФС задокументированы.

Приоритетные nits (для следующего тикета, не как скрытый патч): mock/error-injection для fail-closed; опционально двухпроцессный harness или зафиксированный manual run под критерии acceptance-плана.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts:
  - docs/reviews/20260509-s21-external-lease-final-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 14000
  confidence: medium
```

---

**Строка вердикта для оркестратора:** `PARTIAL — merge допустим при принятии scope MVP + отдельном follow-up на Err/harness двух процессов; блокер merge от ревью нет.`

