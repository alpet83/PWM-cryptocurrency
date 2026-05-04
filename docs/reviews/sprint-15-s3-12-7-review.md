# Sprint 15 - S3.12.7: финальный review (wire `u128` decode)

## 1) Findings (по убыванию серьёзности)

Нет findings уровня high/critical. Узкий decode-shim, единая точка `decode_wire_msg_payload` в `crates/pwmd/src/transport.rs`, encode/handshake не расширены.

Low - покрытие тестами. В `crates/pwmd/src/transport.rs` для `AccountViews` регресс использует `balance_pwm` как decimal строку (`wire_decode_account_views_accepts_non_empty_u128_payload`); для `CrossShardFacts` дополнительно проверяется числовой JSON (`amount: 42`). Симметричного кейса `balance_pwm` как число нет — риск низкий, оба поля проходят через общий `de_u128_compat` в `crates/pwmd/src/wire_serde.rs`.

Low - область действия типа. `#[serde(deserialize_with = "...de_u128_compat")]` стоит на полях в `PeerAccountViewWire` (`crates/pwmd/src/state.rs`) и `CrossShardFact` (`crates/pwmd/src/ledger.rs`), то есть на уровне типа, а не только в `PeerWireMsg`. По текущему коду десериализация этих структур из JSON идёт в основном через `PeerWireMsg`; иных активных путей не выявлено. Это предупреждение на будущее, не дефект текущего среза. Отражено в `issues-report.md`.

Информационно (про трекинг): после принятия отчёта нужно закрыть финальную делегацию в `tasks/20260430-s15-slice3-12-7-wire-u128-decode-fix.json`.

## 2) Requirements fit (acceptance S3.12.7)

Сверка по `tasks/20260430-s15-slice3-12-7-wire-u128-decode-fix.json` и артефактам:

- Нет recurring `wire_decode_failed: u128 is not supported` в live smoke: соответствует (`docs/reviews/sprint-15-s3-12-7-testing.md`).
- Стабильность seed-сессий без decode-churn: соответствует.
- Focused tests на u128 decode: соответствует (три юнит-теста + `cargo test -p pwmd wire_decode_`).
- `home_lookup_status=ok` на стабильной сессии: соответствует.
- Узкий backward-safe фикс без redesign и без изменения trust boundary: соответствует.

Итог: соответствие полное по scope S3.12.7.

## 3) Safety

- Декодер не расширяет доверенную модель peer.
- Негативные значения отклоняются контролируемо (`visit_i64` / parse error).
- Ограничение кадра 1 MiB уже присутствует; нового DoS-вектора не добавлено.

## 4) Style

- Имена в рамках локального правила: `de_u128_compat`, `decode_wire_msg_payload`.
- Вынос в `wire_serde.rs` логичен и снижает дублирование.

## 5) Tests

Плюсы:
- непустые `AccountViews` и `CrossShardFacts`;
- negative-case на невалидный/отрицательный формат;
- общий decode-path покрыт.

Нит:
- можно добавить кейс `balance_pwm` как JSON number для симметрии (не блокер).

## 6) Verdict

Approve with nits.

Acceptance criteria выполнены, фикс узкий и decode-scoped.

## Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-12-7-review.md
token_usage:
  source: estimate
  input: 26000
  output: 3200
  total: 29200
  confidence: medium
```
