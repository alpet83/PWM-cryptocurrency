## Sprint 14 Slice 7 — Independent Review

### Findings (severity ordered)

#### High
- `addr-derive` стал stateful по умолчанию: без `--wallet-out` теперь пишет wallet в default path, что ломает ожидаемую stateless-семантику и создаёт неожиданный side effect.
- `addr-bruteforce --overwrite-wallet` не отключает resume: поиск стартует не с нуля, что противоречит intent/документации для явного overwrite.

#### Medium
- Resolver default path резолвит home даже для явно заданного пути, что создаёт лишнюю точку отказа в нестандартных окружениях.

### Verdict
**BLOCK (request changes)** — нужен remediation по двум high-issues.
