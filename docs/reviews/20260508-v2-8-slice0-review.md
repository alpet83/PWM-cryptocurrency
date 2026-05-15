# V2-8 Slice 0 — Quality gate (RFC freeze)

**Agent:** `pwm-review`  
**Date:** 2026-05-08  
**Scope:** Sprint V2-8 Slice 0 — freeze нормативного контракта same-shard sync v1 (документы только).

## 1. Scope recap

По `docs/plans/mvp_v2.md` (секция Sprint V2-8) Slice 0 — это **RFC/doc freeze**: протокол сообщений, fork-choice v1, anti-DoS, capability negotiation до кодовых правок wire/state.

Заявленные артефакты:

- `docs/rfc/15-same-shard-sync-v1.md` — основной контракт sync v1 (статус Frozen для Slice 0).
- `docs/rfc/8-shard-runtime-identity-and-peering.md` — нормативная отсылка из §4.1 к RFC 0015 при `services` ⊇ `sync`.
- Отчёт `pwm-testing`: `docs/reviews/20260508-v2-8-slice0-testing.md` (PARTIAL с нитами).

Прод-код в срезе не входит; проверка — **spec / трассируемость / внутренняя согласованность текстов**.

## 2. Разбор нитов `pwm-testing` (blocking vs non-blocking)

| Нит | Blocking для freeze Slice 0? | Обоснование |
|-----|------------------------------|-------------|
| В `mvp_v2.md` не было явного пути к `docs/rfc/15-same-shard-sync-v1.md` в блоке «Файлы/модули» | **Нет** | Операторская трассируемость; на смысл контракта не влияет. **Исправлено в ходе ревью** (см. §6). |
| В RFC 15 §11 не было отдельной приёмки Slice 0 | **Нет** | Freeze уже отражён в шапке и Abstract; отсутствие буллетов усложняло воспроизводимый docs-gate. **Исправлено в ходе ревью** (§11 «Slice 0 — RFC freeze»). |
| Размытость `sync_capabilities` (§5.1) vs `sync_profile` (§5.2) | **Нет** | Не логическое противоречие, а уточнение формы на wire. **Исправлено в ходе ревью** (связка логического токена с `sync_profile` + уточнение в §5.1). |

Итог: **ни один нит не блокирует** закрытие Slice 0 как RFC freeze при условии принятых docs-правок (выполнено в этом же коммите).

Дополнительное наблюдение (вне списка testing): RFC 0008 остаётся в статусе **Draft**, тогда как RFC 0015 — **Frozen**. Это ожидаемо, если freeze Slice 0 касается прежде всего **sync v1**, а идентичность/peering дорабатывается следующими слайсами; противоречий с RFC 0015 по handshake/шарде на уровне прочтения не выявлено. **Follow-up:** при отдельном freeze RFC 8 — выровнять статусы и пересечь чеклисты.

## 3. Requirements fit (план и тикет)

- Цель Slice 0 из `mvp_v2.md` (freeze message-contract, acceptance) **выполняется**: RFC 15 покрывает wire taxonomy, negotiation, fork-choice v1, DoS, legacy, observability и критерии слайсов 1–5.
- Зависимости RFC 15 ↔ RFC 8 и ссылки на `WHITE_SPEC` / ADR в заголовках присутствуют.
- Тикет `tasks/20260508-v2-sprint8-slice0-rfc-freeze.json` согласован с артефактами; после ревью статус переводится в **done**.

## 4. Style / safety / tests (применимость)

- **Style (доки):** структура RFC читаемая, англоязычные нормативные формулировки согласованы с остальными RFC в репо.
- **Safety:** для docs-only среза — проверена явность anti-DoS и границ доверия к пирам; критичных пробелов относительно заявленного scope нет.
- **Tests:** автотесты кода не применимы; гейт — ревью документов + закрытие нитов трассируемости/§5/§11.

## 5. Verdict

**PASS-WITH-NITS (закрытие Slice 0 RFC freeze):** блокирующих замечаний нет; ниты testing закрыты **мелкими правками docs** в том же изменении, что и этот отчёт.

### Follow-ups для kickoff Slice 1 (wire skeleton)

1. Реализовать типы/сериализацию сообщений §6 и общие поля заголовка сообщения (в т.ч. `shard_id` / reject).
2. Зафиксировать в коде форму расширения handshake `sync_profile` согласно §5.2 и правилам RFC 8.
3. Ввести метрики/логи из §10 минимум в объёме, достаточном для отладки wire (можно нарастить по Slice 5).
4. Явно определить в реализации соответствие «`full_v1` vs `legacy_observe`» §5.3 до подключения live sync.
5. (Опционально) Запланировать отдельный docs-тикет на **статус RFC 0008** и сквозную матрицу «Draft vs Frozen» для операторов.

## 6. Правки, внесённые агентом `pwm-review` (только docs/plan)

- `docs/plans/mvp_v2.md` — в Sprint V2-8 добавлена явная строка на `docs/rfc/15-same-shard-sync-v1.md`.
- `docs/rfc/15-same-shard-sync-v1.md` — уточнена связка §5.1/§5.2, добавлена нормативная фраза после описания `sync_profile`, добавлен подраздел **§11 Slice 0 — RFC freeze (docs gate)**.

---

## Participation / token estimate

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260508-v2-8-slice0-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 14000, "confidence": "low" }`

**Примечание:** поле `result: PASS` отражает успешный **quality gate** по Slice 0; формулировка «PASS-WITH-NITS» в §5 соответствует закрытым в коммите нитам, а не открытому долгу.
