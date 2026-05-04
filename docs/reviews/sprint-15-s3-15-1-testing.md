# S15-S3.15.1 — testing

## CLI / HTTP (локально при двух нодах)

| URL | Ожидание |
|-----|----------|
| `GET http://127.0.0.1:3031/v1/status` | **200** — HTTP API DO |
| `GET http://127.0.0.1:3131/v1/status` | ошибка соединения / не HTTP — peer TCP listener |

Тот же паттерн для CY: RPC `:3030`, peer `:3130`.

## Вывод

Relay не должен слать HTTP на `--transport-peer-seed` (TCP peer port); база для relay — RPC `--listen` удалённой ноды (или явный `--transport-relay-http-seed`).

---

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-15-1-testing.md
token_usage:
  source: estimate
  total: 4000
  confidence: low
```
