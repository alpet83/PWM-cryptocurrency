# Sprint V2-3 Slice 3 — независимое ревью (demo guide + guardrails)

**Дата:** 2026-05-06  
**Тикет:** `tasks/20260506-v2-sprint3-emission-whales.json`  
**Диапазон:** `c84a40a` (Slice 3 demo + AGENTS/.cursorrules + тикет), `b12d322` (нормализация `.cursorrules` + метаданные в тикете)  
**Артефакты:** `docs/reviews/sprint-v2-3-slice3-demo-guide-20260506.md`, `AGENTS.md`, `.cursorrules`, обновления `tasks/20260506-v2-sprint3-emission-whales.json`

## 1. Scope recap

Заявленный скоуп: операторский runbook для демонстрации V2-3 (schema v5, legacy-safe defaults, ручное включение `policy_v2` через правку `gen_cfg`, наблюдение PWM/marks по REST, таблица ожидаемых reward cases) **без** изменения продуктового runtime; плюс регистрация границ оркестратора в репозитории (`AGENTS.md`, `.cursorrules`) и трассируемость в тикете.

Связь с планом: `docs/plans/mvp_v2.md` Sprint V2-3; контракт полей и schema freeze: `docs/reviews/sprint-v2-3-slice0-design-freeze.md`.

## 2. Requirements fit

**Соответствие freeze и runtime V2-3**

- Указание `schema_version=5`, значения legacy-safe по умолчанию (`policy_ver=1`, `pwm_stake_min=100000`, `marks_stake_min=1`, `season_enabled=false`, `season_coeff_ppm=1000000`) совпадают с `docs/reviews/sprint-v2-3-slice0-design-freeze.md` и константами в `crates/pwm-core/src/genesis.rs` (`LEGACY_POLICY_VER`, `DEF_PWM_STAKE_MIN`, `DEF_MARKS_STAKE_MIN`, `DEF_SEASON_COEFF_PPM`).
- Замечание про явные значения в `dev_net()` согласуется с наличием `dev_net` в `genesis.rs` с теми же константами.
- Путь «сгенерировать genesis, затем отредактировать JSON для `policy_ver=2`» честно зафиксирован как временный demo-path при отсутствии CLI-флагов; это не противоречит Slice 0 freeze (поля контракта уже есть).
- Таблица ожидаемых кейсов (legacy vs ниже порогов vs на пороге vs сезонный ppm) качественно согласуется с описанной в тикете логикой Slice 1 (legacy branch, гейты по stake, масштабирование season). Для оператора уровень детализации достаточен; количественные «дельты» оставлены на шаблон измерения — это ожидаемо для demo-гайда.

**Техническая согласованность примеров JSON**

- В шаге правки `gen_cfg` используются строковые литералы для полей порогов и ppm. Это **согласуется** с wire-моделью в `crates/pwmd/src/snapshot/genesis.rs` (поля `pwm_stake_min`, `marks_stake_min`, `season_coeff_ppm` как строки и разбор через `parse_u128_json`), то есть не выглядит как подводный камень для загрузки genesis.

**Пробелы / низкий риск**

- `Start-Sleep -Seconds 12` и «пример N=10» не гарантируют ровно N блоков при произвольной конфигурации времени блока; для демо-дока приемлемо, но оператору стоит ориентироваться на фактическую смену `height`, а не только на таймер.
- Документ не ссылается на preflight `target/debug` / рекомендации длинных `cargo run` из оркестраторского пайплайна — для локального демо некритично, но при жёстких лимитах диска можно было бы добавить одну строку-напоминание.

## 3. Style and module shape

- Demo guide: структура разделов 1–6 логична; команды PowerShell и проверки JSON читаемы; шаблон таблицы для фиксации фактов — полезная практика для трассируемости демо.
- `AGENTS.md`: краткие границы оркестратора vs субагентов, без противоречий репозиторным правилам.
- `.cursorrules`: YAML frontmatter, предупреждение про CQDS/smart-grep, явная роль main chat как оркестратора с делегированием в `pwm-*`, ссылка на `AGENTS.md`. Нормализация в `b12d322` устраняет риск дублирования или размытия формулировок.

Продукционный Rust в этом слайсе не рецензировался (вне скоупа документов).

## 4. Safety

- Документ не добавляет исполняемой логики. Операторские риски минимальны: напоминание про `PWM_GENESIS_PASSPHRASE` и локальный `genesis-file` уместно; рекомендация не использовать прод-секреты в таких демо остаётся на стороне оператора (можно усилить одной фразой в будущем, не блокер).
- REST-вызовы на localhost — ожидаемый devnet сценарий.

## 5. Tests

Автотесты на данном слайсе не добавлялись. Приёмка Slice 3 — документальная: согласованность с freeze и с парсером genesis проверена статически (см. §2). Для полевой проверки оператор выполняет шаги runbook; это вне scope автоматического gate в данном коммите.

## 6. Verdict

**Approve with nits**

Приоритетные замечания (неблокирующие):

1. Уточнить в гайде, что ожидание по таймеру не заменяет проверку `height` при демонстрации «N блоков».
2. Опционально: одна строка про preflight/target или длинный `cargo run` для сред с тесным диском.

Блокирующих несоответствий freeze, guardrail-документам оркестратора или wire-формату genesis не выявлено.

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS
artifacts: docs/reviews/sprint-v2-3-slice3-review-20260506.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 9500, "confidence": "low" }
```

---

**Вердикт (one-liner для оркестратора):** PASS — approve with nits; demo guide и guardrails согласованы с V2-3 freeze/orchestrator; в тикете зафиксированы `b12d322`, `artifacts.slice3_review_md`, делегирование pwm-review Slice3 и `commits[]` включает `9e2065e` (бандл отчёта+тикета).
