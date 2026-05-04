# Sprint 14 — Slice 12 (Docs) Review

Дата: 2026-04-28  
Тип: targeted docs review  
Статус: approve

## Проверено

- В `docs/GENESIS_BLOCK.md` проверки `/v1/status`, `/v1/head`, `/v1/account/...` теперь явно привязаны к порту из `--listen`, что снимает рассинхрон примеров `3030/3040`.
- В `docs/reviews/genesis-validator-key-roles-20260428.md` текущий контракт отражён как `m/1000000'/1'`; legacy `m/0'/0'` оставлен только как историческая пометка.
- В `docs/reviews/wallet-schema-version-behavior-20260428.md` добавлена пометка о историчности и актуальная ремарка, что create-paths сохраняют wallet в v3.
- В `docs/MVP-checklist.md` ссылки на tester guides приведены к корректному относительному виду; файлы присутствуют, `[x]` валиден.

## Риски

- Существенных рисков не выявлено: правки документационные и локальные.
