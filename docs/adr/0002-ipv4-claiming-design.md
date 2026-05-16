# ADR 0002: IPv4 Claiming Design (foundation boundary)

## Статус

Draft (V3 foundation, decision baseline for V5 implementation).

## Контекст

`docs/CONCEPT_ROADMAP.md` фиксирует IPv4 claiming как стратегическую тему V5 и риск R1, который нельзя оставлять без архитектурной рамки до protocol-freeze.

Также:

- `docs/rfc/5-genesis-and-bootstrap.md` отмечает claim-seeded track как post-v1 extension;
- `DRAFT_WHITEPAPER-ru.md` задает продуктовый ориентир на IPv4-linked distribution;
- V3 scope в `docs/plans/mvp_v3.md` явно запрещает включать runtime IPv4 allocation engine в foundation-спринты.

## Решение (уровень архитектуры)

1. **Разделение ролей:** claim-регистрация и проверка контроля IPv4 диапазонов выполняются во внешнем claim-registry контуре (off-chain).
2. **On-chain фиксируется агрегат:** chain принимает только верифицируемые batch commitments/claims, а не полный реестр IPv4 диапазонов.
3. **Дизайн claim-window:** распределение делится на дискретные фазы (итерации), каждая фаза имеет собственные правила eligibility и объема распределения.
4. **Анти-double-claim:** уникальность подтверждается через off-chain registry + on-chain commit semantics (batch root/phase linkage).
5. **Топология распределения:** если premine создаётся в одной базовой шарде, production claiming не должен наивно раздавать средства тысячами межшардовых переводов от одного genesis-адреса. Предпочтительный baseline для будущего RFC — fan-out через промежуточные **shard distribution accounts**: базовый pool сначала распределяется крупными tranche/commitment по shard-local узлам, а массовые claim payouts выполняются локально внутри соответствующей шарды.
6. **Гейт совместимости:** V3 не вводит runtime tx/path для полного IPv4 claiming; до V5 допускаются только ADR/RFC-уровень спецификации.

## Почему так

- Уменьшается риск ранней перегрузки L1 runtime данными IPv4 ownership.
- Сохраняется прозрачный путь к масштабируемому распределению через агрегаты.
- Снижается нагрузка на cross-shard слой в начале сети: claim payouts могут выполняться shard-local, а не через тысячи `EXPORT`/`IMPORT` от одного premine holder.
- Решение согласуется с offchain/batch направлением из roadmap и с ограничениями V3 foundation-only.

## Bootstrap distribution topology

Будущий implementation RFC должен явно выбрать одну из моделей начального распределения:

- **Single base premine + shard fan-out (preferred baseline):** весь supply может быть minted в базовой шарде, но перед массовым claiming средства расходятся крупными траншами на shard distribution accounts. Эти аккаунты становятся локальными источниками выплат claimants внутри шарды. Такой путь сохраняет компактный demo/genesis baseline и уменьшает стартовый cross-shard шум.
- **Fat genesis with many pre-funded accounts:** genesis сразу содержит множество shard-local allocations. Это уменьшает runtime fan-out, но утолщает genesis block/state, усложняет аудит, bootstrap package, replay fixtures и future snapshot commitments.
- **Hybrid:** genesis содержит только coarse shard pools, а индивидуальные allocations раскрываются через batch roots / claim registry.

ADR 0002 фиксирует только стратегическое направление: **избегать массовой раздачи с одного адреса через межшардовые переводы**. Точная структура shard distribution accounts, batch roots, proofs и governance спорных claims остаётся задачей V5 implementation RFC.

## Deferred implementation boundaries (не часть V3)

В V3 **не** реализуются:

- production claim-registry сервис;
- финальный on-chain tx `ClaimIPv4Batch` и его экономические параметры;
- runtime fan-out от premine pool к shard distribution accounts;
- решение о "fat genesis" vs coarse shard pools как production genesis format;
- governance-механика спорных claim и апелляций;
- интеграция с V5 tokenomics/runtime admission.

## Последствия

- Для V5 обязателен отдельный implementation RFC/ADR с точным wire/state контрактом.
- API `v1` в V3 не должен рекламировать runtime IPv4 claim endpoint как стабильный публичный контракт.

## Ссылки

- `docs/CONCEPT_ROADMAP.md`
- `docs/plans/mvp_v3.md`
- `docs/rfc/5-genesis-and-bootstrap.md`
- `DRAFT_WHITEPAPER-ru.md`
