# V2 RFC publication pack — testing gate (docs-only)

**Date:** 2026-05-05  
**Scope:** RFC 0011–0014 + `docs/rfc/README-v2-claims-pack.md`  
**Gate:** pre-coding publication sanity (consistency + implementability for future automated tests)  
**crates/*:** not reviewed (unchanged per task)

## Verdict: **PASS**

Пакет пригоден как опорная спецификация для E-1/E-2/E-3: цепочка зависимостей ясна, кросс-RFC ссылки согласованы, противоречий уровня «нельзя реализовать без выбора форка» не выявлено.

## Consistency check

| Area | Result / notes |
|------|----------------|
| **Dependency flow** | `README`: 0011 → 0012 → 0013 → 0014 — совпадает с содержанием (tx → state → policy → API wire). |
| **Cross-references** | Relative links из README и каждого RFC на `docs/reviews/sprint-v2-1-*` разрешаются (файлы присутствуют). |
| **State vs tx** | RFC 0012 (maturity, `anchor_ref`, `claim_units`, stake discontinuity, free-day) согласован с полями ClaimTx в RFC 0011; границы `claim_units` заданы в 0012, не конфликтуют с baseline 0011. |
| **Policy vs API errors** | RFC 0013 задаёт нормализованные `E_*` semantic classes; RFC 0014 принимает тот же набор для Claim mapping + отдельный Burn набор — трассируемость «0013 → 0014» явная. |
| **Dual error namespaces** | RFC 0011 фиксирует stable-by-meaning **tx/preflight** строковые коды (`INVALID_PURPOSE_*`, `CLAIM_*`, …); RFC 0013 — **policy** `E_*`. Это осознанное слоение, не логическая коллизия, но см. testability (ниже). |
| **Symmetry mempool/preflight/apply** | Повторяется в 0011, 0013, 0014 в одном духе; противоречий по каноничности `apply` не видно. |

## Testability notes (for later `pwm-core` / `pwmd` / conformance)

1. **Маппинг слоёв ошибок:** для золотых векторов «tx parse → policy class → API `error.code`» полезно в реализации (или отдельном conformance-доке слайса E-1) явно связать коды RFC 0011 с `E_*` RFC 0013/`error.code` RFC 0014 там, где имена расходятся (например `FREE_CLAIM_DAILY_LIMIT` vs `E_FREE_CLAIM_DAILY_LIMIT`, `CLAIM_DELTA_INVALID` vs `E_CLAIM_UNITS_INVALID`). Сейчас это следует из смысла, но не из одной нормативной таблицы в RFC.
2. **Burn в API:** RFC 0014 перечисляет `E_BURN_*`; RFC 0011 даёт tx-уровень для burn — тесты API должны якориться на 0014 + policy burn-расширения, а не на полном перечне 0011 без 0014.
3. **Контролируемые фикстуры:** maturity/floor/anchor predicates из 0012–0013 хорошо ложатся на table-driven unit tests (одинаковый snapshot → эквивалентный вердикт по фазам), как только появится код.
4. **Исключено этим гейтом:** прогон `cargo test`, MCP/CQDS process — артефакт docs-only; риски регрессии кода не применимы.

## Sign-off

| Role | Result |
|------|--------|
| pwm-testing (docs gate) | **PASS** |
