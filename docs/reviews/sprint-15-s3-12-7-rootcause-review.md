# Sprint 15 - S3.12.7 Rootcause Review

## 1) Scope recap

Проверен scope из `tasks/20260430-s15-slice3-12-7-wire-u128-decode-fix.json`: локализация `wire_decode_failed: u128 is not supported`, тип несовместимости, узкий фикс-план и минимальные regression tests.

Симптоматика сверена с:

- `tasks/20260430-s15-slice3-12-6-production-idle-read-fix.json`
- `docs/reviews/sprint-15-s3-12-6-testing.md`

## 2) Requirements fit (root-cause)

### Точный decode-контекст

Ключевой decode:

```rust
serde_json::from_slice::<PeerWireMsg>(&payload).map_err(|e| format!("wire_decode_failed: {e}"))
```

`PeerWireMsg` содержит data-frame варианты:

- `AccountViews { rows: Vec<PeerAccountViewWire> }`
- `CrossShardFacts { facts: Vec<CrossShardFact> }`

Где есть `u128` поля:

- `PeerAccountViewWire.balance_pwm: u128`
- `CrossShardFact.amount: u128`

### Почему live FAIL, а focused tests PASS

После handshake live path шлёт непустые `AccountViews`/`CrossShardFacts`, тогда как focused tests часто используют пустые `rows/facts` и не заходят в реальный decode `u128` payload.

Вывод: это отдельный data decode blocker, не продолжение idle-read issue из S3.12.6.

## 3) Тип compatibility-ошибки

- Наиболее вероятно: JSON decode несовместимость по `u128` на стадии `serde_json::from_slice::<PeerWireMsg>`.
- Не похоже на handshake mismatch (он проходит).
- Не похоже на промежуточный `serde_json::Value` path (decode идёт напрямую в `PeerWireMsg`).

## 4) Narrow fix guidance для `pwm-coding`

1. Патчить точечно wire codec слой:
   - `crates/pwmd/src/transport.rs`
   - `crates/pwmd/src/state.rs` (`PeerAccountViewWire.balance_pwm`)
   - при необходимости `crates/pwmd/src/ledger.rs` (`CrossShardFact.amount`) для wire payload.

2. Репрезентация `u128` на wire: decimal string-safe.
   - decode должен принимать и string, и number (переходная совместимость).
   - encode можно стабилизировать в string, но с осторожным rollout.

3. Safe rollout:
   - сначала расширить decode (accept both), encode оставить текущим;
   - затем, после обновления узлов, при необходимости перевести encode в string.

4. Не менять handshake/trust boundary/reconnect policy вне codec-слоя.

## 5) Минимальные regression tests

1. `PeerWireMsg::AccountViews` decode с `balance_pwm` как string decimal (и numeric variant при dual decode).
2. `PeerWireMsg::CrossShardFacts` decode с `amount` string/numeric.
3. Framed socket payload test (len + json) с непустым `rows`, без `wire_decode_failed`.
4. Negative test: overflow/invalid numeric string -> controlled decode error.

## 6) Safety

Security regression не выявлен, но текущий churn даёт operational DoS effect (постоянные переподключения, деградация foreign lookup).

## 7) Verdict

request changes

`u128` wire decode compatibility issue likely principal blocker текущего live churn.

## Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts:
  - docs/reviews/sprint-15-s3-12-7-rootcause-review.md
token_usage:
  source: estimate
  input: 14500
  output: 3200
  total: 17700
  confidence: medium
```
