# Independent review: MVP V3 Sprint 3 — demo genesis & public devnet runbook

**Date:** 2026-05-16  
**Reviewer role:** pwm-review  
**Ticket:** `tasks/20260516-v3-sprint3-demo-genesis-devnet.json`  
**Plan:** `docs/plans/mvp_v3.md` (Sprint V3-3)

## 1) Scope recap

Заявленный слайс закрывает near-one-command путь для внешнего тестера: генерация demo genesis с premine 21B PWM (в canonical raw), проверка суммы, запуск CY-топологии (три терминала), ссылки на smoke по `docs/api-v1.md`. Изменены: `scripts/demo-genesis-build.ps1`, `scripts/demo-genesis-verify.ps1`, `scripts/demo-devnet-start.ps1`, `cy-cluster-common.ps1`, `docs/runbooks/demo-devnet-quickstart.md`, фрагменты `docs/plans/mvp_v3.md`, `docs/api-v1.md`, `docs/tester-guide-devnet-smoke.md`, метаданные тикета. Продакшен-Rust в слайсе не менялся (вне области этого ревью).

## 2) Requirements fit

- **21B PWM / raw scale:** В runbook и скриптах явно: `PWM_RAW_SCALE = 1_000_000`, целевая raw-сумма `21_000_000_000_000_000`, совпадает с owner requirement и картой pwm-info.
- **Верификатор:** Суммирует `gen_cfg.funding.accounts[*].bal` через `BigInteger`, сравнивает с ожидаемым значением; отсутствие файла и пустой список — fail-fast. Для артефакта `pwm genesis-build` поля `bal` сериализуются как **строки** (`GenesisAccountOut.bal: String` в `cmd_genesis.rs`), поэтому `ConvertFrom-Json` в PowerShell не теряет точность для 21B raw (в отличие от гипотетического JSON с числовым литералом того же порядка).
- **Без скрытого `tmp/genesis-custom.json` на primary path:** `demo-devnet-start` вызывает build по умолчанию; `cy-cluster-common` допускает `$env:PWM_DEMO_GENESIS_PATH` / `$env:PWM_DEMO_GENESIS_PASSPHRASE` при сохранении прежнего дефолта.
- **Прозрачность demo/devnet:** Runbook помечает не-production posture, предупреждает о passphrase/секретах.
- **Пробел (из pwm-testing):** Полный 3-node + API smoke на свежем demo genesis в среде тестирования не выполнялся (конфликт/недоступность `127.0.0.1:3030`). Это **пробел приёмки slice-3 тикета**, а не обнаруженный дефект скриптов; риск снят частично: dry-run, fail-fast verify, реальный build+verify прошли.

**Итог по фиту:** цели спринта по артефактам и математике выполнены; интеграционная приёмка «ноды подняты + smoke» остаётся рекомендованным следующим шагом (см. план Sprint V3-4).

## 3) Style and module shape

PowerShell и Markdown; оркестраторные правила длины идентификаторов Rust к данному диффу не применимы. Структура скриптов читаема: общие дефолты passphrase через env с явным dev fallback, опциональный `-DryRun`, вызов verify после build.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4) Safety

- **Секреты:** В репозиторий не добавлены закрытые ключи/wallet; дефолтные passphrase (`12345`) явно помечены как devnet-only. Риск оператора: `demo-devnet-start.ps1` печатает значения `$GenesisPassphrase` в подсказках трёх терминалов — при нестандартном секрете он попадёт в историю консоли (низкая серьёзность, ожидаемо для lab).
- **Trust boundaries:** Скрипты запускают `cargo run` и читают локальные JSON/YAML; внешней сети нет.
- **Гонка портов/state:** Runbook кратко описывает занятые порты и очистку `tmp/state-cy-*`.

## 5) Tests

- **Покрыто pwm-testing:** dry-run, verify при отсутствии genesis (exit 1), полный build+verify с `21_000_000_000_000_000` raw.
- **Не покрыто:** end-to-end после `./cy-cluster-*.ps1` с demo genesis и три `Invoke-RestMethod` на живой proposer-RPC.
- **Рекомендация владельцу:** прогон smoke в чистой сессии (остановить конфликтующие процессы на `3030`/`13030` или сменить порта в документации — только если политика лаба меняется).

## 6) Verdict

**PASS_WITH_NITS**

**Findings (по убыванию серьёзности):**

1. **Низкая — пробел интеграционной приёмки:** Нет подтверждения полного API smoke после старта трёх нод на свежем demo genesis в отчёте pwm-testing; основание для уверенности — отдельный прогон оператором или перенос в Sprint V3-4 explicit matrix (**owner decision / test gap**, не блокер кода скриптов).
2. **Низкая — UX документации:** В `docs/plans/mvp_v3.md` секция «Декомпозиция на таски» ссылалась на «будущие» файлы для V3-2/V3-3; обновлена на фактические пути (**mechanical**, закрыто ревью).
3. **Низкая — устаревший комментарий:** В шапке `cy-cluster-proposer.ps1` указана жёсткая зависимость от `tmp\genesis-custom.json`; фактически genesis задаётся через `cy-cluster-common` и env (**mechanical**, закрыто ревью).
4. **Информационная:** Если вручную собрать genesis с числовым (не строковым) `bal` в JSON, верификация через `ConvertFrom-Json` на старых PowerShell теоретически рискует точностью; для канона `pwm-cli` это не применимо.

**Классификация nits (автозакрытие vs владелец):**

- **Mechanical / docs-scripts:** обновление ссылок на таски в `mvp_v3.md`, комментарий в `cy-cluster-proposer.ps1` — **auto-close** (выполнено в рамках ревью).
- **Owner / процесс:** полный 3-node + API smoke на чистой машине — **owner decision**, трек Sprint V3-4 или повтор pwm-testing.

## 7) Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS_WITH_NITS",
  "artifacts": "docs/reviews/sprint-v3-3-demo-genesis-devnet-review-20260516.md",
  "token_usage": {
    "source": "estimate",
    "input": 14000,
    "output": 4500,
    "total": 18500,
    "confidence": "medium"
  }
}
```

## Glossary

GLOSSARY.md: без изменений (подслайсовое ревью Sprint V3-3; нового жаргона для глоссария не зафиксировано).

## Команды / проверки, учтённые при ревью

- Чтение: `scripts/demo-genesis-build.ps1`, `scripts/demo-genesis-verify.ps1`, `scripts/demo-devnet-start.ps1`, `cy-cluster-common.ps1`, `cy-cluster-proposer.ps1` (фрагмент), `docs/runbooks/demo-devnet-quickstart.md`, `docs/api-v1.md`, `docs/tester-guide-devnet-smoke.md`, `docs/plans/mvp_v3.md`, тикет и info map; точечная сверка сериализации `bal` в `crates/pwm-cli/src/cmd_genesis.rs` (только чтение для обоснования верификатора).
- Устные результаты pwm-testing из тикета: dry-run, fail-fast verify, успешный build+verify.

---

**Verdict (one-line):** PASS_WITH_NITS — математика premine и путь demo genesis выглядят_sound для V3 public devnet package; полный 3-node/API smoke остаётся невыполненным в отчёте тестирования и переносит минимальный остаточный риск на оператора/Sprint V3-4.
