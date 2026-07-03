# Оффчейн batch-burn

Назначение: агрегировать множество burn-событий клиента в один проверяемый batch для внешних интеграций (`svcpool.io` и аналоги).

## V7 HTTP API

- `POST /v1/offchain/batch` принимает JSON array записей `{ account_id, amount, nonce }`.
- Нода строит SHA-256 binary Merkle tree:
  - leaf preimage: `PWMv1/OFFLEAF || account_id[32] || amount_be_u128 || nonce_be_u64`;
  - node preimage: `PWMv1/OFFNODE || left_hash || right_hash`;
  - нечётный последний leaf дублируется на уровне.
- Ответ содержит `batch_id`, `merkle_root`, `entry_count`, `anchor_tx_hash`.
- `GET /v1/offchain/batch/:id` возвращает сохранённый root и anchor id.
- `GET /v1/offchain/batch/:id/proof/:entry_index` возвращает leaf hash и sibling proof, достаточные для клиентской проверки включения.

## Anchor status

V7-5 API хранит batch process-local и выдаёт deterministic `anchor_tx_hash`, вычисленный из `batch_id`, `merkle_root` и текущего tip. Это additive runtime anchor surrogate без нового consensus tx variant.

Полная on-chain transaction anchoring остаётся следующим шагом: нужен отдельный signed daemon/operator path или новый additive tx/memo contract, чтобы root попадал в блок как consensus-visible факт без ломки существующего `/v1` и snapshot форматов.

## Legacy v0 note

Старый stub использовал BLAKE3 leaf + `PWMv0-OFFCHAIN-BATCH` provider signature для CLI demo. Он оставлен как исторический контекст; production HTTP путь V7 использует SHA-256 Merkle по схеме выше.
