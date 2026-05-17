# PWM API v1 (V4 policy runtime baseline)

Статус: Baseline after MVP v4 closeout (2026-05-17)  
Область: public devnet `/v1/*` контракт с учётом V4 policy runtime

## 1) Зачем этот документ

`docs/api-v1.md` — source-of-truth для `/v1/*` baseline после закрытия MVP v4.  
Цель: зафиксировать публичный контракт для внешнего smoke и интеграционного прототипирования, включая уже shipped policy-runtime поверхность (без обещаний V5+ функциональности).

## 2) Граница baseline после V4

### Входит в public baseline (`/v1/*`)

Ниже endpoints, которые считаются публичным контрактом после V4 closeout и должны меняться только аддитивно:

- `GET /v1/status`
- `GET /v1/head`
- `GET /v1/accounts`
- `GET /v1/account/:id`
- `POST /v1/tx`

### Не входит в публичный stable contract

- **Operator endpoints**: нужны для операционного управления, отладки и межузлового handoff в devnet-сценариях.  
- **Dev-only endpoints**: внутренний/диагностический surface для dev-профиля.

Изменения operator/dev маршрутов в V3 допускаются без гарантий внешней стабильности, если это не ломает public stable `/v1/*`.

## 3) Endpoint-классы

Ниже три класса: публичный стабильный freeze (`/v1/*` из §2), операторские маршруты и dev-only поверхность.

## 3.1 Public stable endpoints (V4 baseline)

Поля статус-кодов и формы JSON ниже задают целевой baseline-контракт после MVP v4; перед внешним релизом их нужно сверять с `docs/pwmd.md` и фактическим поведением `pwmd`.

| Endpoint | Назначение | Минимальный контракт (после V4) |
|---|---|---|
| `GET /v1/status` | Readiness и runtime status | Возвращает фазу старта и признак готовности; пригоден для smoke-gate |
| `GET /v1/head` | Текущая высота/тип | Возвращает `{ height, tip }` |
| `GET /v1/accounts` | Список аккаунтов | Возвращает объект `{ accounts: [...] }` (массив аккаунтов текущего state) |
| `GET /v1/account/:id` | Аккаунт по id | `400` при невалидном id, `404` если не найден |
| `POST /v1/tx` | Подача транзакции | Принимает tx JSON, применяет валидацию и возвращает `204` на success |

Примечание по ошибкам: для claim/burn/policy reject semantics действует RFC 0014 (`docs/rfc/14-claim-burn-api-error-contract.md`) как стабильный baseline кодов/классов ошибок, где применимо.

### 3.1.1 V4 inspection-поля аккаунта (`/v1/accounts`, `/v1/account/:id`)

В `AcctOut` (когда поля присутствуют в state) возвращаются inspection-поля policy runtime:

- `rescue_address`
- `active_policies`
- `dormant_policies`
- `finalized`
- `owner_kind`
- `owner_display_name`
- `owner_country_hint`
- `company_metadata_commitment`
- `external_verification_ref`
- `requested_domain_lo`

Источник контракта: `docs/pwmd.md` (`GET /v1/accounts`, `GET /v1/account/:id`).

### 3.1.2 `POST /v1/tx`: V4 policy/runtime shape

- `POST /v1/tx` принимает `PolicyTx` наравне с остальными tx-видами.
- При policy reject сервер возвращает RFC 14-совместимый JSON с `error.code` из стабильного набора `E_POLICY_*`.
- Минимальные ожидаемые коды включают: `E_POLICY_NOT_INSTALLED`, `E_POLICY_NOT_ACTIVE`, `E_POLICY_DENIED`, `E_POLICY_SENDER_FILTERED`, `E_POLICY_ROUTING_DENIED`, `E_POLICY_MISSING_COSIGN`, `E_POLICY_RESCUE_REQUIRED`, `E_POLICY_EMERGENCY_COSIGN_REQUIRED`, `E_POLICY_ACCOUNT_FINALIZED`, `E_POLICY_IRREVERSIBLE`.

## 3.2 Operator endpoints (outside stable public contract)

- `POST /v1/roaming-intents`
- `GET /v1/roaming-intents/:id`
- `POST /v1/roaming-intents/:id/finalize`
- `POST /v1/export-provenance`
- `GET /v1/flow/recent`
- `POST /v1/bridge-federation/reset`
- `GET /v1/operator/log/override`
- `POST /v1/operator/log/override`
- `DELETE /v1/operator/log/override`

Эти маршруты критичны для операторских сценариев и диагностик, но **не** фиксируются как публичный стабильный контракт V3.

Runtime log-control endpoints are specified in `docs/rfc/17-runtime-log-control-rpc.md`.
They are authorized operator/debug controls, not public client API.
Current gate in `pwmd`: loopback requests are accepted; non-loopback requires `PWM_ADMIN_TOKEN` + `Authorization: Bearer <token>`.

## 3.3 Dev-only endpoints (outside stable public contract)

- `POST /v1/peer/hello` (dev-only handshake probe)
- `GET /v1/dev/peers` (dev-only peer registry)

Эти маршруты предназначены для devnet/debug и могут изменяться без совместимости с внешними клиентами.

## 4) Минимальный public devnet smoke

Ниже минимальный smoke-пакет для внешнего тестера.
Для near-one-command запуска demo genesis + devnet используйте runbook: `docs/runbooks/demo-devnet-quickstart.md`.

## 4.1 `GET /v1/status`

```bash
curl -sS http://127.0.0.1:3030/v1/status
```

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"
```

Ожидание: ответ JSON с полями runtime phase/readiness.

## 4.2 `GET /v1/head`

```bash
curl -sS http://127.0.0.1:3030/v1/head
```

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/head"
```

Ожидание: JSON c `height` и `tip`.

## 4.3 `GET /v1/accounts` -> `GET /v1/account/:id`

```bash
curl -sS http://127.0.0.1:3030/v1/accounts
# Возьмите id из ответа и подставьте в следующий запрос:
curl -sS http://127.0.0.1:3030/v1/account/<ACCOUNT_ID_HEX>
```

```powershell
$resp = Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/accounts"
$id = $resp.accounts[0].id
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/account/$id"
```

## 4.4 `POST /v1/tx` (минимальный шаблон)

Фактический tx payload зависит от типа транзакции и подписи, поэтому для smoke здесь фиксируется только transport-вызов:

```bash
curl -sS -X POST http://127.0.0.1:3030/v1/tx \
  -H "Content-Type: application/json" \
  -d '<SIGNED_TX_JSON>'
```

```powershell
$body = "<SIGNED_TX_JSON>"
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/tx" -Method Post -ContentType "application/json" -Body $body
```

## 5) Совместимость и versioning policy (V4 baseline)

- Freeze касается только **public stable `/v1/*`** из раздела 3.1.
- Для breaking-изменений публичного контракта после V4 требуется отдельное версионирование (`/v2/*`) или явно задокументированное исключение.
- Operator/dev endpoints остаются evolveable surface и **не являются стабильным публичным контрактом**.

## 6) Deferred boundaries (явно не часть текущего V4 baseline)

В рамках текущего baseline этот документ не обещает:

- production offchain batch API;
- runtime IPv4 claiming engine;
- V5/V6/V7 runtime фичи.

Этот документ фиксирует V4-baseline контракт для public devnet и дальнейшего review.
