# Sprint 15 — микро-слайс **N** (style **N**): короткие имена тестов + docstrings

**Код слайса:** `S15-N`  
**Дата заведения:** 2026-05-03  
**Статус (2026-05-03):** волны **N.1–N.4** закрыты (inline **`#[cfg(test)]`** для **`pwm-cli` / `pwm-core` / `pwmd`** — см. **`4a1523c`**). Отдельный аудит **prod**-идентификаторов **`fn`** (>5 сегментов) вне объёма этого слайса.

**Вводная ревью:** PARTIAL по именам тестов (pwm-review за тикетом `tasks/20260614-s15-module-banners-test-names-style.json`) — массовые имена **`#[test] fn`** и тест-хелперов длиннее **≤ 5 сегментов** `snake_case`; прод‑поведение пользователем подтверждено отдельно.

## Цель

Стабилизировать **объявленный** стиль (**`AGENT_PROMPT_coding.md`**, **`AGENT_PROMPT_testing.md`**, **`AGENT_PROMPT_review.md`**): для тестового кода — **≤ 5 сегментов** на имя `fn`/хелпера; перенос «истории сценария» в **`///`** или короткий **`//`** над функцией. Семантика тестов **не меняется**.

## Границы

| In scope | Out of scope |
|----------|----------------|
| Переименование тестовых **`fn`** и **test-only** хелперов; добавление **`///`** / **`//`** | Изменение прод‑логики, контрактов API, сообщений пользователю |
| Волнами по crate/директориям (отдельный тикет на волну) | Одним коммитом весь монорепозиторий |

## Конвейер (классический)

**`pwm-coding`** → **`pwm-testing`** → **`pwm-review`** (оркестратор по `docs/AGENT_PROMPT_orchestrator.md`).

Приёмка на волну: **`cargo fmt --all -- --check`**, **`cargo test -p <crate>`** для затронутых crate; по возможности **`cargo check --workspace`**. На Windows перед полным прогоном закрыть **`pwm-tui`** и процессы, держащие **`target\debug\*.exe`**.

## Волны (итеративно)

| Волна | Область | Тикет |
|-------|---------|--------|
| **N.1** | `crates/pwmd/src/tests/**`, `crates/pwmd/src/transport/tests/**` | `tasks/20260615-s15-slice-N-wave1-pwmd-test-fn-names.json` — **DONE** (`28a3ec2`, `3d25ddb`, документирование); ревью `docs/reviews/sprint-15-slice-N-wave1-pwmd-test-fn-names-review.md` |
| **N.2** | `pwm-cli` **`src/tests/mod.rs`**, **`tests/*.rs`** | `tasks/20260616-s15-slice-N-wave2-pwm-cli-test-fn-names.json` — **DONE** (`2de256c`, `5f11691`, документирование); ревью `docs/reviews/sprint-15-slice-N-wave2-pwm-cli-test-fn-names-review.md` |
| **N.3** | `pwm-tui` **`tests/**/*.rs`** | `tasks/20260617-s15-slice-N-wave3-pwm-tui-tests-fn-names.json` — **DONE** (`cff2fab`, `3343f11`, `64e8c49`, документирование); ревью `docs/reviews/sprint-15-slice-N-wave3-pwm-tui-tests-fn-names-review.md` |
| **N.4** | **`pwm-cli`** inline tests (`wallet/mod`, `bruteforce`), **`pwm-core`** unit tests, **`pwmd`** inline tests | `tasks/20260618-s15-slice-N-wave4-inline-tests-fn-names.json` — **DONE** (`4a1523c`); ревью `docs/reviews/sprint-15-slice-N-wave4-final-review.md` |

Чеклист: [sprint-15-slice-N-checklist.md](sprint-15-slice-N-checklist.md).
