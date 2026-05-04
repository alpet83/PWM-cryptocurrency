# S15-S3.15.1 — coding

## Причина

`relay::select_target` брал `TransportConfig.peer_seeds` (TCP адрес peer listener, например `:3131`) и строил `http://{seed}/v1/status`. На этом порту слушает **wire peer**, не Axum RPC — запрос падает.

## Исправление

- `TransportConfig.relay_http_seeds: Vec<SocketAddr>` + CLI **`--transport-relay-http-seed`** (повторяемый список).
- Если список пустой: **`relay_http_bases`** = для каждого `peer_seed` адрес с портом **`peer_tcp.port - 100`** (обратная операция к `resolve_peer_listen`: rpc+100 → peer).
- Юнит-тесты на deriviation и приоритет явного списка.
- Тесты `v1_roaming_intent_*` обновлены на текст **`no HTTP relay base configured`**.
- Версия **pwmd 0.1.32**.

## Файлы

- `crates/pwmd/src/config.rs`, `main.rs`, `relay.rs`, `lib.rs` (assertions)
- `crates/pwmd/Cargo.toml`
- `issues-report.md`

```yaml
agent: pwm-coding
result: PASS
token_usage: { source: estimate, total: 14000, confidence: low }
```
