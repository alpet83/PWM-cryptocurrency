# Review: pwm-cli wallet v3 single-file `--index` on tx commands

**Ticket:** `tasks/20260620-pwm-cli-wallet-v3-single-file-tx-index.json`  
**Date:** 2026-06-17  
**Reviewer:** pwm-review  
**Coding handoff:** PASS (claimed `cargo test -p pwm-cli` green)

---

## 1. Scope recap

Слайс закрывает операторский кейс «один wallet v3 YAML с несколькими `accounts[]`»: premine / victim (flags=2) / rescue в одном файле, выбор подписанта через `--wallet` + `--index` на tx-командах (не только `tx-init`), rich hint при stale nonce на `--activation-tx`, обновление `docs/pwm-cli.md` и runbook `v6-owner-stability-soak-50k.md`.

Затронутые файлы: `cli_cmd.rs`, `cli_dispatch.rs`, `cmd_tx.rs`, `tests/mod.rs`, `docs/pwm-cli.md`, `docs/runbooks/v6-owner-stability-soak-50k.md`.

Зависимость: `20260617-pwm-cli-tx-init-v3-wallet-account-index`.

---

## 2. Requirements fit (vs acceptance criteria)

| # | Criterion | Status | Notes |
|---|-----------|--------|-------|
| 1 | `--index` на `tx-policy-activate/set/deactivate`; signer = `m/0/N`; v2 без регрессии | **Partial** | Clap + dispatch + `load_tx_wallet_signer` → `load_wallet_account_signer` на всех policy-командах — **да**. Регрессия v2: при `derivation_index != 0` и опущенном `--index` (clap default `0`) `resolve_wallet_account` отклоняет запрос; раньше `load_tx_signer_source` брал корневой аккаунт через `load_sender_from_wallet`. Для v2 с `derivation_index == 0` поведение сохраняется. В `design_notes` заявлен default = `wallet.derivation_index` / min `accounts[]`, в коде не реализован. |
| 2 | `--index` на `tx-send/stake/unstake/burn-mark` через тот же helper | **Met** | Единый `load_tx_wallet_signer`; `--master` по-прежнему обходит wallet-index path. |
| 3 | `--activation-tx` + HTTP 409 bad nonce → hint (file vs chain nonce, live `--index`) | **Met** | `enrich_act_nonce_err` / `is_act_nonce_err`; тест `tx_pol_nonce_hint_409` с mock GET nonce. |
| 4 | Live `tx-policy-activate` same-wallet e2e unit/smoke | **Partial** | Parse-тест `tx_pol_act_sw_idx` (wallet + index + rescue-account-index). Нет smoke, который собирает tx с rescue cosign из того же wallet (только `prepared_activation_roundtrip` / `build_init_activation`). Для operator gate достаточно runbook + ручной soak, но AC формулировка шире. |
| 5 | `docs/pwm-cli.md`: таблица `--index` + emergency same-wallet пример | **Partial** | `--index` документирован в секциях каждой tx-команды и в «Signing + send flow». Отдельной сводной таблицы нет (заголовок «Карта команд» пустой). В блоке «Примеры» нет emergency same-wallet сценария (есть в runbook). |
| 6 | Runbook: primary = один wallet v3; шаги 2–8b без обязательных split yaml | **Met** | §Emergency routing переписан: `WAL`, `VICTIM_IDX`, `RESCUE_IDX`, live path 7b; split — optional fallback. |
| 7 | Unit-тесты: parse + signer selection; 409 nonce message | **Partial** | `tx_cmd_idx_parse`, `tx_pol_act_sw_idx`, `tx_init_sel_wallet_idx` (helper), `tx_pol_nonce_hint_409`. Нет теста v2 default-index / `derivation_index != 0`. |
| 8 | `cargo test -p pwm-cli` green | **Not re-run** | Зафиксировано в ticket `notes`; ревью не перезапускало. |

**Итог по AC:** ядро v3 single-wallet path реализовано; два осмысленных пробела — **v2 default-index регрессия** (явный AC) и **отсутствие emergency-примера в `pwm-cli.md`**.

---

## 3. Style and module shape

- **Helper consolidation:** `load_tx_init_source` переименован в `load_tx_wallet_signer` и переиспользуется всеми tx-runners — соответствует `design_notes`, дублирования нет.
- **Module banners:** `cmd_tx.rs`, `cli_dispatch.rs`, `cli_cmd.rs` имеют `//!` на английском.
- **Dispatch:** `cli_dispatch.rs` прокидывает `index` во все целевые `run_tx_*` — согласовано с clap.
- **Naming (`check_entity_name_segments.py`):** production symbols в пределах политики (≤4 сегмента). В `cmd_tx.rs` `#[cfg(test)]` функции с 5 сегментами (`tx_init_sel_wallet_idx`, …) — checker помечает как `kind: prod` (ложное срабатывание); для тестов лимит 5, нарушений нет.
- **Clap defaults:** `default_value_t = 0` на всех новых `--index` — согласовано, но конфликтует с заявленным v2-friendly default (см. AC #1).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

---

## 4. Safety

- **Trust boundaries:** пути wallet остаются локальными; rescue cosign только для `routing.emergency_redirect` — guard сохранён.
- **Prepared activation:** чтение JSON с диска без выполнения произвольного кода; при 409 подсказка не раскрывает секреты.
- **`enrich_act_nonce_err`:** дополнительный GET nonce только при уже известном 409+nonce — приемлемый UX trade-off.
- **Паники / unwrap:** в hot path по-прежнему `exit_user_error` на ошибках RPC; новых `unwrap` в production path не добавлено.
- **Риск оператора:** при v2 wallet и забытом `--index` — fail-fast с понятным сообщением (не silent wrong signer), но это регрессия удобства, не security bug.

---

## 5. Tests

**Покрыто хорошо:**
- Clap parse `--index` на send/burn/stake/unstake/policy-set/deactivate (`tx_cmd_idx_parse`).
- Same-wallet emergency activate flags (`tx_pol_act_sw_idx`).
- `load_tx_wallet_signer` + v3 multi-account (`tx_init_sel_wallet_idx`).
- Nonce mismatch detection и rich stderr (`tx_pol_nonce_err_detect`, `tx_pol_nonce_hint_409`).
- Prepared activation roundtrip (`prepared_activation_roundtrip`).

**Пробелы:**
- Нет теста: v2 wallet, `derivation_index = N (N>0)`, команда без `--index` — ожидаемое поведение по AC не зафиксировано.
- Нет unit/smoke live `run_tx_policy_activate` с `rescue_account_index` в том же wallet (cosign на подписанном tx).
- `pwm-testing` должен подтвердить полный `cargo test -p pwm-cli` (ревью не дублировало прогон).

---

## 6. Verdict

**Approve with nits** — слайс готов для v3 operator soak и закрывает главную боль (stale prepared activation + multi-account signing). Перед merge в operator gate рекомендуется добить **default index для v2** (или явно задокументировать breaking change) и **emergency-пример в `pwm-cli.md`**.

### Prioritized nits (pwm-coding)

1. **Medium — AC #1:** при `--wallet` без `--master` и `index == 0` (clap default) резолвить signer в `wallet.derivation_index` для schema v2 (и/или min `derivation_index` в v3), как в `design_notes`; добавить regression test.
2. **Low — AC #5:** в `docs/pwm-cli.md` §Примеры — блок emergency same-wallet (`--index`, `--rescue-account-index`, `--activation-target`); опционально сводная таблица tx-команд с колонкой `--index`.
3. **Low — AC #4/7:** smoke-тест `load_rescue_source` / live activate cosign из одного wallet (можно без live RPC, assert на `cosigns` в собранном `SignedTx`).

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260620-pwm-cli-wallet-v3-single-file-tx-index-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 28000
  confidence: low
```

---

## Git handoff

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260620-pwm-cli-wallet-v3-single-file-tx-index-review.md'
git commit -m 'docs(slice): pwm-cli wallet v3 --index tx commands review'
```
