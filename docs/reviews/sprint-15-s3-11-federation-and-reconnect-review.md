# Sprint 15 S3.11: Federation and Reconnect Review

## Scope
- Проверка reconnect/hello churn на стандартных нодах.
- Контракт node-level federation dictionary (шарды + height + TTL 60s).

## A) Reconnect/hello churn

### Verdict
`request changes`

### Findings
- По логам обеих нод наблюдается устойчивый паттерн: частые `peer hello accepted` (около раз в ~2с).
- Одновременно видны:
  - стабильный peer endpoint (`127.0.0.1:3130/3131`);
  - и постоянно новые ephemeral source ports (`29xxx`, `30xxx`, `31xxx`).
- Для stateful-режима это означает не удержание long-lived session, а регулярные reconnect-и.

### Operator interpretation
- Смена ephemeral source port сама по себе нормальна для нового TCP-коннекта.
- Ненормально, что такие новые коннекты возникают постоянно: это деградированный режим stateful transport.

## B) Federation dictionary contract

### Verdict
`request changes`

### Minimal contract

`FederationShardRow` (key: `shard_id`)
- `shard_id: string`
- `latest_height: u64`
- `last_seen_unix_ms: u64`
- `ttl_sec: 60`
- `expires_at_unix_ms: u64`
- `source: "hello" | "heartbeat" | "status"`
- `source_node_id: string`
- `fresh: bool`

### Update sources
- trusted peer hello
- trusted heartbeat (с высотой)
- trusted peer status

### Merge rules
- нет записи -> insert;
- `incoming.height > current.height` -> replace;
- `incoming.height == current.height` -> брать более новый `last_seen`;
- `incoming.height < current.height` -> height не понижать, обновлять only seen/source;
- `last_seen` монотонный по `max`.

### Expiration
- sweep раз в ~1s;
- eviction при `now >= expires_at_unix_ms`;
- TTL фиксирован 60s.

### API
`GET /v1/federation/shards`
- `generated_at_unix_ms`
- `ttl_sec`
- `view_health: complete | partial | stale`
- `expected_shard_count: u32 | null`
- `active_shard_count`
- `stale_shard_count`
- `rows: FederationShardRow[]`

## Test gaps
- Нет acceptance-теста на session longevity (stateful без частого reconnect).
- Нет теста/контракта на TTL eviction federation rows.
- Нет API-тестов на `view_health` семантику полноты сети.
