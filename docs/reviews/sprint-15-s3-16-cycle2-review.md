# S15-S3.16 cycle2 — индекс межшардовых транзакций (файлы + поиск)

**Тикет:** `tasks/20260430-s15-slice3-16-cycle2-xshard-credit-tui-step5.json`  
**Поиск:** не полагаться на встроенный grep IDE (зависания). Использовать **`rg`** из корня репозитория или MCP **user-cqds_mcp_mini** (`cq_files_ctl` + `start_grep`).

## Индекс

| Область | Файл | Роль | Что искать (`rg`) |
|--------|------|------|-------------------|
| Relay HTTP | `crates/pwmd/src/relay.rs` | Доставка handoff на peer, POST import на target | `relay_handoff`, `relay_import`, `push_relay_flow`, `export-provenance`, `POST /v1/tx`, `target_hi_for_import` |
| HTTP API / tx | `crates/pwmd/src/api.rs` | Handoff verify, roaming intents, `v1/tx`, provenance, сводки | `handoff_msg`, `verify_handoff`, `enforce_import_provenance`, `roaming-intents`, `TxBody::Import`, `stuck_relayed_without_import` |
| Roaming state machine | `crates/pwmd/src/roaming.rs` | Intent lifecycle, export/import связка | `RoamingIntent`, `exported`, `relayed`, `imported`, `mark_` |
| Ledger / bridge | `crates/pwmd/src/ledger.rs` | Учёт export/import сумм | `import`, `export`, `bridge` |
| State | `crates/pwmd/src/state.rs` | Персистентность, locks | `roaming`, `export` |
| Policy / gates | `crates/pwmd/src/tx_policy.rs` | Гейты на Import | `Import`, `recipient` |
| Federation / identity | `crates/pwmd/src/federation.rs`, `identity.rs` | Домены, ключи | `domain`, `cluster` |
| TUI flow | `crates/pwm-tui/src/main.rs` | Пошаговый cross-shard, вызовы RPC | `submit_roaming_intent`, `xflow_`, `roaming-intents`, `Cross-domain` |
| E2E тесты | `crates/pwmd/src/slice20_e2e_tests.rs` | Референс сценариев | `import`, `export`, `relay` |

Ядро консенсуса (применение `TxBody::Import` / суммы): искать workspace-wide `TxBody::Import` и `apply_` / `execute` в crate с протоколом (имя crate в `Cargo.toml` workspace).

## Гипотезы: списание есть, зачисления нет

1. **Import tx не доходит до target** — `relay_import` падает молча или статус intent «imported» без реального apply (сверить логи `relay: POST /v1/tx` и ответ API).
2. **Provenance / handoff** — `verify_handoff` или `enforce_import_provenance_prefilter` отклоняют импорт на target при уже relayed handoff.
3. **Неверный получатель / domain / amount** в теле Import относительно Export — частичное применение или no-op.
4. **Recipient init gate** — `enforce_recipient_init_gate` блокирует кредит до инициализации аккаунта на target.
5. **Рассинхрон статуса** — intent помечен relayed/imported в памяти, а ledger не обновлён (проверить `ledger.rs` + snapshot).

## Вердикт (review)

Индекс и точки входа зафиксированы для cycle2; детальный root-cause — после прогона с логами и шага 5 TUI (баланс target).

**Coding note (cycle2, pwm-coding):** симптом «debit без credit» при чистых нодах совпадал с отсутствием **клиентской** отправки `Import` после `relayed` (опрос intent не создаёт tx). Исправление в `pwm-tui`: автоматический Import на **source** RPC (реле на target), шаг 5 — проверка дельты баланса получателя на target; комиссия снимается на source, в зачисление входит **только** `amount` экспорта.

```yaml
participation:
  agent: pwm-review
  verdict: partial
  note: "pwm-review в сессии не записал файл; оркестратор создал артефакт по обходу репозитория."
```
