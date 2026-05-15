# Slice 1 final review — protocol semver + build-control logging

**Дата:** 2026-05-09  
**Роль:** `pwm-review` (независимый гейт)  
**Тикет:** `tasks/20260509-protocol-versioning-debug-controls.json`  
**Диапазон:** `08cc97d` (coding), `223057f` (тикет), `c4031c4` (testing-артефакты)

## 1. Scope recap

Slice 1 по тикету и design-gate: **семвер-совместимость в handshake** (отказ при несовпадении major, предупреждение при расхождении minor/patch), **startup build-control** (маркер сборки, путь к бинарю, mtime в unix-ms, pid), **стабильные reason-лейблы** для отказов по версии, **дисциплина в промптах** coding/review для bump `handshake::PWM_PROTOCOL_VERSION`. Slice 2 (dump блоков, time-align seal) вне объёма — корректно отложен.

## 2. Requirements fit

| Цель | Оценка |
|------|--------|
| Major mismatch → reject | Да: `protocol_compat` → `HandshakeRejectReason::ProtocolVersionMajorMismatch`; транспорт закрывает сессию с лейблом `protocol_version_major_mismatch`, деталь с expected/received. |
| Minor/patch mismatch → warn only | Да: `ProtocolCompat::FractionalMismatch`; в `incoming_hello` только `warn!`, без инкремента `reject_reason_total`. |
| Malformed semver → reject | Да: строгий парсер `major.minor.patch` (ровно три числовых сегмента); лейбл `protocol_version_malformed`. |
| Build control на старте | Да: после `init_logging` вызывается `log_build_control` — marker из `CARGO_PKG_VERSION` + опциональные `+ts:`/`+git:`, `binary_path`, `binary_mtime_utc_unix` (`…ms` или `unavailable`), `pid`; при недоступном `current_exe` — одна строка с `reason=`. |
| Дисциплина в документации агентов | Да: `docs/AGENT_PROMPT_coding.md` — блок Protocol semver bump discipline; `docs/AGENT_PROMPT_review.md` — проверка решения по `PWM_PROTOCOL_VERSION` при wire-изменениях. |

**Зазор:** отдельный интеграционный тест только на patch-ветку не добавлен; логика общая с minor — остаточный риск низкий (зафиксирован в отчёте testing).

## 3. Style and module shape

- Идентификаторы: прогон `python scripts/check_rust_fn_name_segments.py` по затронутым `pwmd` файлам — **нарушений нет**.
- Модули: `handshake` получил минимальный `//!`; новые типы/функции вписаны в существующую форму.
- **Политика wire-semver:** введён явный `PWM_PROTOCOL_VERSION`; для этого слайса bump до `0.1.0` согласован с тем, что меняется только дисциплина проверки объявленной версии у пира (а не обязательно семантика полей `NodeHello`). Корректность трактовки «no wire field change» vs bump — отражена в handoff coding; при будущих wire-изменениях review-prompt обязывает явное решение.

## 4. Safety

- Нет новых `unwrap` на горячем пути handshake; отказы идут через существующие guard-пути.
- Парсинг версии ограничен фиксированным форматом — нет неограниченного разбора строки сверх трёх сегментов.
- Build-control не паникует при ошибке `current_exe` / metadata.
- **Наблюдаемость:** расхождение `PWM_PROTOCOL_VERSION` (`0.1.0`) и `CARGO_PKG_VERSION` (например `0.1.52`) — не баг логики guard, но **операторский риск путаницы** в логах (в одной строке `pwmd/<crate>`, в hello — wire-версия). Рекомендация: держать это явно в runbook/операторской заметке или при следующем touch добавить в build-control отдельное поле wire `protocol_version=` (вне must-fix этого гейта).

## 5. Tests

- Покрытие согласно `docs/reviews/20260509-slice1-semver-build-control-testing.md`: unit-тесты `handshake` (parse, major, minor warn), `incoming_hello` (major reject + метрика, minor accept), `main` (marker, mtime missing/existing), smoke `incoming_hello::` и dial trust — **PASS**.
- Префлайт `target/debug` в testing не выполнялся — не блокер для вердикта по коду, но оркестратору стоит придерживаться политики preflight для воспроизводимости CI/локальных прогонов.

## 6. Verdict

**Approve with nits** (технически готово к слиянию; процессные и док-ниты ниже).

**Nits (не блокеры merge):**

1. Сообщение коммита `08cc97d`: «tmp test msg» — **стоит исправить** (amend/reword или отдельный chore) для читаемости истории.
2. Зафиксировать в операторской доке различие **crate version** vs **wire `PWM_PROTOCOL_VERSION`** или расширить build-control лог одним полем wire-версии при следующем изменении.
3. По желанию: точечный тест на `0.1.0` vs `0.1.1` для симметрии с minor.

**Must-fix:** нет для объявленного scope Slice 1.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
 
artifacts:
  - docs/reviews/20260509-slice1-semver-build-control-final-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 14000
  confidence: medium
```

## 8. Merge readiness (Slice 1)

- **PASS** — функциональные критерии Slice 1 выполнены, тестовый гейт по отчёту **PASS**, блокирующих дефектов не выявлено.
- **Merge readiness:** **ready** — рекомендуется перед merge поправить сообщение коммита с реализацией (косметика процесса).

---

_End of final review._
