# Sprint 15 — слайс **O** (code **O**): оптимизация и разгрузка «жирных» модулей

**Дата:** 2026-05-02  
**Код слайса:** `S15-O` / **O**  
**Источник правды по аудиту:** [CODEBASE_REFACTORING.md](../CODEBASE_REFACTORING.md)

## Цель

Снизить технический долг по размеру файлов и дублированию **без блокировки** основного плана MVP (S4 snapshot abstraction и далее). Слайс **внеочередной**: выполняется перед продолжением крупной реализации по roadmap.

## Границы

| In scope (O) | Out of scope (отложить на под-слайсы O.x или позже) |
|--------------|-----------------------------------------------------|
| Задачи из [sprint-15-slice-O-checklist.md](sprint-15-slice-O-checklist.md) (безопасный минимум + согласованные средние шаги) | Полная декомпозиция `pwm-tui/main.rs` / `transport.rs` / `pwm-cli/main.rs` одним махом |
| Механические правки, вынос тестовых stub из prod, общие мелкие extractions в `pwm-core` по чеклисту | Новый отдельный crate `pwm-rpc-client` без отдельного решения оркестратора |
| Документирование прогресса и регресс-тесты после каждого шага | Рефакторинг поведения протокола / консенсуса |

## Конвейер

`pwm-review` (дифф по объёму) → `pwm-coding` → `pwm-testing` (`cargo test` полного workspace затронутых crate + точечные сценарии из чеклиста).

## Якорные метрики (из аудита)

Критические по строкам: `pwm-tui` `main.rs` (~6.4k), `pwm-cli` `main.rs` (~5.1k), `pwmd` `lib.rs` / `transport.rs` / `api.rs` / `snapshot.rs`. Целевое состояние — итеративное уменьшение «god files», не разовый big-bang.

## Связанные тикеты

- `tasks/20260502-s15-slice-O-codebase-cleanup.json` (**S15-O**, группы A+B — по завершении `completed`)
- `tasks/20260503-s15-slice-O1-modular-decomposition-wave1.json` (**S15-O.1** волна 1 — `completed`)
- `tasks/20260504-s15-slice-O1-wave2-tui-modules.json` (**волна 2** — `completed`, коммит `996af47`)
- `tasks/20260505-s15-slice-O1-wave3-roaming-sendform-history.json` (**волна 3** — roaming / send_form / history)
- `tasks/20260506-s15-slice-O1-wave4-account-view-selection.json` (**волна 4** — account_view / selection)
- `tasks/20260507-s15-slice-O1-wave5-tui-loop.json` (**волна 5** — tui_loop / `run()`)
- `tasks/20260508-s15-slice-O1-wave6-tui-render-split-imports.json` (**волна 6** — imports + render split)
- `tasks/20260509-s15-slice-O1-wave7-tui-modal-render.json` (**волна 7** — modal/overlay render helpers)
- `tasks/20260510-s15-slice-O1-wave8-tui-wallet-modals-render.json` (**волна 8** — book/unlock/encrypt modals)
- `tasks/20260511-s15-slice-O1-wave9-tui-panels-render.json` (**волна 9** — owner/receivers/detail render)
- `tasks/20260512-s15-slice-O1-wave10-tui-draw-remainder.json` (**волна 10** — thin term.draw)
- `tasks/20260513-s15-slice-O1-wave11-tests-extract.json` (**волна 11** — lib + `tests/`, §6.2)
- `tasks/20260514-s15-slice-O1-wave12-test-support-narrow-pub.json` (**волна 12** — узкий корень crate + `#[doc(hidden)] test_support`)
- `tasks/20260502-s15-slice-O1-wave13-pwmd-transport-peer-types.json` (**волна 13** — `transport` §2.2 row #1 `peer_types.rs`; коммиты `da29dc5`/`fa4712e`; ревью `docs/reviews/sprint-15-slice-O1-wave13-review.md`)
- `tasks/20260517-s15-slice-O1-wave14-pwmd-transport-metrics.json` (**волна 14** — §2.2 row #2 `metrics.rs`; `26db44b`/`e40130b`; ревью `docs/reviews/sprint-15-slice-O1-wave14-review.md`)
- `tasks/20260518-s15-slice-O1-wave15-pwmd-transport-tick.json` (**волна 15** — §2.2 row #3 `transport_tick.rs`; `1b62851`; ревью `docs/reviews/sprint-15-slice-O1-wave15-review.md`)
- `tasks/20260519-s15-slice-O1-wave16-pwmd-transport-dial.json` (**волна 16** — §2.2 row #4 `dial.rs`; `54c8618` + fix `7c29822`; ревью `docs/reviews/sprint-15-slice-O1-wave16-review.md`)
- `tasks/20260520-s15-slice-O1-wave17-pwmd-transport-peer-session.json` (**волна 17** — §2.2 row #5 `peer_session.rs`; `03b44e5`/`686026e`; ревью `docs/reviews/sprint-15-slice-O1-wave17-review.md`)
- `tasks/20260521-s15-slice-O1-wave18-pwmd-transport-health.json` (**волна 18** — §2.2 row #6 частично `health.rs`; `c3ab48b`/`11a3b5d`; pwm-review опционально)
- `tasks/20260522-s15-slice-O1-wave19-pwmd-transport-policy.json` (**волна 19** — `policy.rs`; `509e759`; pwm-review опционально; итог слайса — эксплуатационное тестирование)
- `tasks/20260523-s15-slice-O1-wave20-pwmd-transport-lifecycle.json` (**волна 20** — `lifecycle.rs`; `de4d8b6`; pwm-review опционально)
- `tasks/20260524-s15-slice-O1-wave21-pwmd-transport-handshake-incoming.json` (**волна 21** — `handshake_state.rs` + `incoming_hello.rs`; `dc26e6c`; pwm-review опционально)
- `tasks/20260525-s15-slice-O1-wave22-25-pwmd-transport-spawn-tests-bridges.json` (**волны 22–25** — `spawn.rs`, dial `attempt_seed_connect` re-export, `tests.rs`/`trust_peer_test.rs`, `bridges.rs`; `df79a13`; pwm-review опционально)
- `tasks/20260526-s15-slice-O1-wave26-29-pwmd-transport-peer-session-split.json` (**волны 26–29** — каталог `peer_session/` (`wire`, `inbound`, `seed`); `d0fd767`/`5b443a3`; ревью `docs/reviews/sprint-15-slice-O1-wave26-29-peer-session-split-review.md`)
- `tasks/20260527-s15-slice-O1-wave30-peer-session-seed-split.json` (**волна 30** — `peer_session/seed/` подмодули `connect`/`handshake`/`session`; `0b1df13`/`fc47e23`; ревью `docs/reviews/sprint-15-slice-O1-wave30-peer-session-seed-split-review.md`)
- `tasks/20260528-s15-slice-O1-wave31-pwmd-transport-tests-split.json` (**волна 31** — `transport/tests/` вместо монолита `tests.rs`; `14ab897`/`fccf93e`; ревью `docs/reviews/sprint-15-slice-O1-wave31-transport-tests-split-review.md`)
- `tasks/20260529-s15-slice-O1-wave32-peer-session-seed-session-split.json` (**волна 32** — `peer_session/seed/session/`; `fbe0c3d`/`88e8ddb`; ревью `docs/reviews/sprint-15-slice-O1-wave32-peer-session-seed-session-split-review.md`)
- `tasks/20260530-s15-slice-O1-waves33-36-pwmd-lib-inline-tests-split.json` (**волны 33–36 составные** — `pwmd/src/tests/` вместо inline `mod tests`; `6b5eaec`/`db2c5d2`; ревью `docs/reviews/sprint-15-slice-O1-waves33-36-pwmd-lib-inline-tests-split-review.md`)
- `tasks/20260531-s15-slice-O1-cli-waves1-4-main-modules.json` (**pwm-cli waves 1–4** — §2.3 старт; `67a6945`/`a462ab2`; ревью `docs/reviews/sprint-15-slice-O1-cli-waves1-4-main-modules-review.md`)
- `tasks/20260601-s15-slice-O1-cli-waves5-8-main-modules.json` (**pwm-cli waves 5–8** — `cmd_tx`/`cmd_roaming`/`cmd_wallet`/`cmd_book`; `91e9796`/`f517e4b`/`0dda232`; ревью `docs/reviews/sprint-15-slice-O1-cli-waves5-8-main-modules-review.md`)
- `tasks/20260603-s15-slice-O1-cli-wave9-cmd-offchain.json` (**pwm-cli wave 9** — `cmd_offchain` / `off-demo`; `af7cf48`/`dedf967`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave9-cmd-offchain-review.md`)
- `tasks/20260604-s15-slice-O1-cli-wave10-cmd-addr.json` (**pwm-cli wave 10** — `cmd_addr`; `759284e`/`9bda0a5`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave10-cmd-addr-review.md`)
- `tasks/20260605-s15-slice-O1-cli-wave11-main-rpc-to-rpc_helpers.json` (**pwm-cli wave 11** — RPC/HTTP-хелперы в `rpc_helpers`; `1715c49`/`20e4c58`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave11-main-rpc-to-rpc_helpers-review.md`)
- `tasks/20260606-s15-slice-O1-cli-wave12-parse-signer-wallet-shell.json` (**pwm-cli wave 12** — `cli_parse`/`signer`/`wallet_shell`; `8fd29ad`/`e74aac9`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave12-parse-signer-wallet-shell-review.md`)
- `tasks/20260607-s15-slice-O1-cli-wave13-integration-tests-dir.json` (**pwm-cli wave 13** — `tests/cli_smoke.rs`; `47bff57`/`72c769a`/`1fe010e`/`e61538e`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave13-integration-tests-dir-review.md`)
- `tasks/20260608-s15-slice-O1-cli-wave14-cli-exit-module.json` (**pwm-cli wave 14** — `cli_exit`; `eee1449`/`2642bff`/`a217829`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave14-cli-exit-module-review.md`)
- `tasks/20260609-s15-slice-O1-cli-wave15-cli-cmd-module.json` (**pwm-cli wave 15** — `cli_cmd`; `767a3fb`/`84d522a`/`8ca19b1`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave15-cli-cmd-module-review.md`)
- `tasks/20260610-s15-slice-O1-cli-wave16-cli-dispatch.json` (**pwm-cli wave 16** — `cli_dispatch`; `db519a2`/`61219e9`/`baee67e`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave16-cli-dispatch-review.md`)
- `tasks/20260611-s15-slice-O1-cli-wave17-wallet-split-s24.json` (**pwm-cli wave 17** — §2.4 `wallet/`; `5c5a994`/`3fb4d27`/`75e39b1`; ревью `docs/reviews/sprint-15-slice-O1-cli-wave17-wallet-split-s24-review.md`)
- `tasks/20260612-s15-slice-O1-pwmd-api-split-s25.json` (**pwmd wave18** — §2.5 `api/`; `54cc0bb`/`e7b492c`/`0006b6f`; ревью `docs/reviews/sprint-15-slice-O1-pwmd-api-split-s25-review.md`)
- `tasks/20260613-s15-slice-O1-pwmd-snapshot-split-s26.json` (**pwmd wave19** — §2.6 `snapshot/`; **`done`**; `a0662c6` / `0de6ac4` / `710f79b`; ревью `docs/reviews/sprint-15-slice-O1-pwmd-snapshot-split-s26-review.md`)

**Коммиты:** pwm-cli §6.2 первый вынос тестов из `main.rs` — `f3806e60a714a5d62efd932c2f1baf8453721546`; размещение приведено к **`src/tests/mod.rs`** (аналогично `pwmd`).

Перед использованием MCP **user-cqds_mcp_mini**: контейнеры CQDS должны быть запущены; при сбое поиска — fallback **`rg`** в корне репозитория.
