# Sprint 15 S3.10 Design Review: Foreign Account Home Lookup

## Verdict
`request changes`

## Contract recommendation
- `local_state_balance` — только локальная запись этой ноды; для foreign это не authoritative truth.
- `spendable_on_this_shard` — `Some(balance)` только для local/home account.
- `authoritative_home_balance` — баланс, полученный от home shard через trusted peer lookup; `None` значит unknown, не `0`.
- `authoritative_home_initialized` — nullable init-state от home shard.
- `home_lookup_status` — минимум `local | ok | unavailable | not_found | stale | error`.
- `balance_pwm` остаётся legacy compatibility field и не должен использоваться новым TUI как foreign display balance.

## Display rule
- Local account: показывать `spendable_on_this_shard` / `local_state_balance`.
- Foreign + `home_lookup_status=ok`: показывать `authoritative_home_balance`.
- Foreign + peer unavailable/stale/error: показывать `???`.
- Foreign + home says not found: показывать `not found`, не `0`.
- Foreign init unknown: показывать `???` и не превращать в `false`.

## Implementation plan
1. Добавить peer account lookup protocol в `pwmd`.
2. Для foreign account route lookup к trusted live peer с matching home `domain_hi`.
3. Расширить `AcctOut`: `authoritative_home_initialized`, `home_lookup_status`.
4. Обновить `/v1/account/:id`, затем `/v1/accounts` с timeout/concurrency cap.
5. В TUI заменить `u128/bool` на tri-state display: known/unknown.
6. В CLI не трактовать unavailable foreign init как uninitialized.

## Required tests
- Reachable home peer returns authoritative balance/init.
- No trusted route returns `unavailable` + authoritative fields `None`.
- Home says missing returns `not_found`.
- TUI renders unknown foreign balance as `???`.
- Unknown foreign init does not become `false`.

## Risks
- Cache must have TTL/height and stale must render `???`.
- Only trusted peers may answer authoritative lookup.
- Account lookup leaks watched addresses to peers.
- `/v1/accounts` must avoid unbounded fanout.
