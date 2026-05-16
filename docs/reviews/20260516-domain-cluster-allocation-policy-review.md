# Review: domain cluster allocation policy (doc slice)

**Ticket:** `tasks/20260516-domain-cluster-allocation-policy.json`  
**Scope:** `docs/CONCEPT_ROADMAP.md` (раздел «Резервирование нескольких IT-кластеров»), `docs/DOMAINS.md` («Strategic reserve note (post-V3)»).  
**Reviewer:** `pwm-review`  
**Date:** 2026-05-16

## 1. Scope recap

Тикет фиксирует продуктовую оговорку: модель «одна отрасль ≈ один базовый кластер» недостаточна для IT; возможное будущее распределение — несколько базовых кластеров под IT-семейство с ограничением по арендуемым `domain_lo` на кластер. Явно заявлено отсутствие изменений `domain_index.rs` в этом слайсе. Чеклист тикета указывает на секции CONCEPT_ROADMAP и DOMAINS — они дополнены согласованно.

## 2. Requirements fit

| Цель | Оценка |
|------|--------|
| Согласованность с `domain_hi` как идентичностью кластера/шарда и `domain_lo` как селектором/арендуемым пространством внутри сектора | **Да.** Текст трактует дополнительные IT-кластеры как отдельные base clusters (разные `domain_hi`), каждый со своим набором арендуемых `domain_lo`, что соответствует вводному в `DOMAINS.md` §«Как читать domain_code». |
| Таблица секторов остаётся runtime-источником истины на текущей фазе | **Да.** В ROADMAP явно: до отдельного ADR `docs/DOMAINS.md` остаётся runtime-справочником. В DOMAINS — что таблица отражает runtime-индекс, запись IT сохраняется. |
| Не создавать впечатление, что `domain_index.rs` уже изменён | **Да.** Прямые формулировки в ROADMAP и DOMAINS об отложенном изменении таблицы и об отсутствии новых runtime-valid кодов. |
| Governance: до 16 IT-кластеров как будущий ADR/RFC, не текущее распределение | **Да.** Формулировки «рабочий ориентир для будущего ADR/RFC», «решение … отдельным ADR», «конкретные коды … ADR/RFC перед production-аукционами». |

**Зазор (низкий приоритет):** число «до 255» арендуемых `domain_lo` при полном `u8`-диапазоне может читаться как оговорка про резерв одного значения или как округлённая формулировка. Если протокол когда-нибудь нормативно зафиксирует «ровно 256 слотов» или «256 минус зарезервированные», имеет смысл одной фразой связать с точным правилом сегментации `domain_lo` для Sector — не блокер для этого слайса.

## 3. Style and module shape

Документационный слайс: язык и структура согласованы с остальным CONCEPT_ROADMAP (подзаголовки, bullet tradeoffs, отсылка к ADR). DOMAINS: секция помечена как post-V3 / strategic, что снижает риск путаницы с нормативной таблицей.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

Рисков для исполняемого кода нет. Продуктовый риск документации — преждевременное трактование «16 кластеров» как решённой квоты; текст это смягчает через ADR/RFC и tradeoff-блок.

## 5. Tests

Не применимо к Markdown-only изменениям; регрессии runtime не ожидаются при отсутствии правок коду.

## 6. Verdict

**PASS_WITH_NITS** — цели слайса выполнены; одна необязательная ниточка про точную семантику «255» против полного диапазона `domain_lo`.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260516-domain-cluster-allocation-policy-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 5500
  confidence: low
```

## 8. Sprint-final glossary traceability

Не финальное ревью спринта; отдельное обновление `docs/GLOSSARY.md` не требуется по правилам промпта для этого слайса.
