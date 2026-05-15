# Spot review: текущие незакоммиченные изменения (E‑3 / claim‑burn клиенты)

**Дата:** 2026-05-06  
**Охват:** файлы из `git status` на момент ревью (modified + untracked), без полного аудита воркспейса.  
**Критерии:** `docs/AGENT_PROMPT_coding.md` / `docs/AGENT_PROMPT_review.md` — **продакшен ≤ 4** сегментов `snake_case`, **тесты ≤ 5**.

## Перечень затронутых путей

- `CHANGELOG.md`
- `crates/pwm-cli/src/cli_cmd.rs`, `cli_dispatch.rs`, `cmd_tx.rs`, `rpc_helpers.rs`, `tests/mod.rs`
- `crates/pwm-core/src/lib.rs`, **`crates/pwm-core/src/reject_wire.rs`** (новый)
- `crates/pwm-tui/src/account_view.rs`, `config.rs`, `lib.rs`, `send_form.rs`, `status.rs`, `test_support.rs`, `tui_loop.rs`, `tx_submit.rs`, **`burn_form.rs`** (новый)
- `crates/pwm-tui/tests/wallet_roaming.rs`
- `docs/tester-guide-cli-tui-scenarios.md`
- артефакты тикета: `docs/reviews/v2-e3-review-20260505.md`, `tasks/20260505-v2-e3-clients-claim-burn.json` (при необходимости коммита — отдельно от кода)

## Соответствие политике имён

### Продакшен (≤ 4 сегментов)

| Символ | Сегментов | Вердикт |
|--------|-----------|---------|
| `summarize_pwmd_tx_reject_json` (`pwm-core`, re-export, вызовы в CLI/TUI) | **5** | **REQUEST_CHANGES** при применении текущего промпта — переименовать или сузить публичную поверхность (отдельный микро-слайс `pwm-coding`). |
| `is_xdom_xfer_reject_body` (`pwm-tui/src/tx_submit.rs`, private) | **5** | **REQUEST_CHANGES** — укоротить (например слить `xdom`/`xfer` в docstring). |

Остальные просмотренные публичные символы в **`burn_form.rs`**, **`submit_burn_mark`**, **`burn_replay_guard_status`** и ряде хелперов укладываются в **≤ 4** или близко к лимиту без превышения.

### Тесты (≤ 5 сегментов)

| Символ | Сегментов | Вердикт |
|--------|-----------|---------|
| `tx_claim_cli_invalid_mode_mentions_claim_mode_flag` (`pwm-cli/src/tests/mod.rs`) | **9** | **REQUEST_CHANGES** — сократить имя теста + при необходимости однострочный `//` с полным сценарием. |

## Прочее (кратко)

- **`reject_wire`:** разбор JSON без лишних `unwrap` на горячем пути ответа — ок для UX-хелпера; тесты покрывают `claim_mode` и `ok:true`.
- **Дублирование логики форм** (`BurnForm` vs `SendForm`): ожидаемый паттерн для slice; полная унификация не требуется spot-review.
- **Тестер-guide / CHANGELOG:** согласовать с финальными именами команд/флагов перед коммитом релизной ветки.

## Итоговый вердикт (spot)

**REQUEST_CHANGES** по именованию относительно **прод ≤ 4 / тесты ≤ 5** на перечисленных символах; функциональная часть не пересматривалась глубоко (это не замена полного `pwm-review` по тикету).

Полный аудит воркспейса зарезервирован как **Slice 4** в Sprint V2-4 в `docs/plans/mvp_v2.md`.

## Participation / token estimate

```
agent: pwm-review
result: PARTIAL
artifacts: docs/reviews/v2-e3-modified-files-spot-review-20260506.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 12000, "confidence": "low" }
```
