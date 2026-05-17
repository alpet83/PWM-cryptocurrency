# MVP V4: план vs факт по «длинным» E2E тестам политик

**Дата:** 2026-05-17  
Контекст: вопрос о наличии продолжительных end-to-end сценариев, проверяющих политики на живом devnet, и организация проверки через существующие `*.ps1`.

## Было ли это в закрытом плане V4?

**Нет как обязательного закрывающего gate.** В спринте **V4-6** integrated smoke фиксировал компиляцию, `cargo test -p pwm-core --lib`, `cargo test -p pwm-cli`, `cargo test -p pwmd --lib`, бенч snapshot и т.д. (см. `docs/reviews/20260517-v4-integrated-smoke.md`). **Живой** devnet с последовательной подачей `PolicyTx`, проверкой `POST /v1/tx` и валидацией reject-кодов в логах **явно относился к optional hardening**, вместе с полным `cargo test --workspace`, ручным TUI и long soak (см. `docs/CONCEPT_ROADMAP.md`, `docs/MVP-checklist.md` §0v4).

Юнит- и lib-тесты `pwm-core` покрывают семантику политик (routing, cosign gate, emergency, finalization) детерминированно; это **не заменяет**, но и **не дублировало** операторский devnet-run в закрытии V4.

## Что добавлено сейчас

1. **Скрипт** `scripts/devnet_v4_policy_e2e.ps1`  
   - Опционально чистит `tmp/state-cy-*` (`-CleanState`).  
   - Вызывает `scripts/demo-devnet-start.ps1` (генезис + wallet, если не `-SkipGenesis`).  
   - Поднимает **proposer + attester** (как и другие CY smoke-скрипты) и ждёт `GET /v1/status`.  
   - Выполняет через `cargo run -p pwm-cli --bin pwm`: `tx-init` с демо-индексом genesis, `tx-policy-set` / `tx-policy-activate` для **обратимой** `routing.same_domain_only`, затем опциональный просмотр `/v1/accounts` и `tx-policy-deactivate`.  
   - Пишет markdown-отчёт в `tmp/devnet_v4_policy_e2e_<ts>.md`.

2. **Опция** `-BruteDemoOnly` — офлайн `pwm addr-bruteforce`; параметр **`-BruteMaxTry`** (по умолчанию **`1000000`**) задаёт верхнюю границу перебора для уверенного попадания в лоторею derivation/domain при масках phase1 (**`flags-mask 1023`**, **`expected-flags 0`**, домен **`CY`**). Прогон **намеренно длинный**: для субагента **pwm-testing** использовать **`cq_process_ctl`** (**`host=true`**) **`spawn`** → **`wait`** (таймаут порядка **900–3600 s** при холодном `cargo run` и ~1 M попыток), см. **`docs/AGENT_PROMPT_testing.md`** секция harness.

3. **Runbook** `docs/runbooks/demo-devnet-quickstart.md` — подпункт **6.1** со ссылкой на скрипт и этот документ.

## Ограничения автоматизации (честно)

| Сценарий | Почему не в «одной кнопке» без доработок |
|----------|------------------------------------------|
| `cosign_required` + witness cosign на произвольном `PolicyTx` | В `pwm-cli` автоматически добавляется только **rescue** cosign для активации emergency; generic witness cosign в CLI отсутствует — проверка остаётся в тестах ядра или ручной сборке `SignedTx`. |
| `default_behavior` / `sender_filter` на входящий `Transfer` | Нужны **два** инициализированных аккаунта и средства на отправителе; скрипт можно расширить вторым `tx-init` + `tx-send`. |
| Emergency routing + rescue | Нужен второй аккаунт как `rescue_address`, корректный `INIT` и `tx-policy-activate` с rescue-кошельком — отдельный сценарий (см. `docs/pwm-cli.md`). |

## Рекомендация по прогону на вашей машине

Из корня репозитория (после закрытия старых `pwmd` и при необходимости удаления `tmp/state-cy-*`):

```powershell
./scripts/devnet_v4_policy_e2e.ps1 -CleanState
```

Ожидайте **минуты** на сборку `cargo run` и старт кластера. При занятых портах CY — см. troubleshooting в quickstart.

## Соавторство в коммитах (локально vs публичный репо)

Локально допускаются служебные метки соавторства от инструментов; для публичного репозитория разумно вычищать или не добавлять такие trailer’ы, если владелец хочет атрибуцию только людям и продуктовым редакторам.
