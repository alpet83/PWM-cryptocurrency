# Sprint 15 S1 Review

## Verdict
`request changes`

## Blockers
1. Критично: `ttl_sec` в `POST /v1/export-readiness` задаётся клиентом без server-side upper bound.
2. Высокий риск: `EXPORT` через `/v1/roaming-intents` может обходить fail-closed readiness guard.
3. Средний риск: reject-диагностика в `POST /v1/tx` неструктурированная (строка вместо стабильного code/hint поля).

## Required Remediation
- Ввести серверный cap на readiness TTL и тесты на oversized TTL.
- Применить ту же readiness-политику к `/v1/roaming-intents` (или формально запретить bypass с тестом).
- Усилить контракт reject-диагностики и покрыть тестами.
