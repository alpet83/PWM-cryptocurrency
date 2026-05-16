# Review: domain_lo=0, corporate INIT gap, V4 boundary (doc slice)

**Ticket:** `tasks/20260516-domain-lo-zero-init-policy-gap.json`  
**Agent:** `pwm-review`  
**Date:** 2026-05-16  
**Scope:** documentation only (`docs/CONCEPT_ROADMAP.md`, `docs/DOMAINS.md`, `docs/WHITE_SPEC_v0.md`, `docs/rfc/6-policy-engine.md`)

## 1. Scope recap

Тикет просит зафиксировать продуктовую семантику: в будущих corporate-sector production base clusters `domain_lo = 0` — слот root/generic для компаний без аренды собственного namespace, регистрация через расширенный `INIT` с metadata и ожидаемой поддержкой emergency routing; граница с минимальным v0/v3 `INIT`; **без** изменений Phase 1B `domain_index.rs`. Чеклист тикета согласован с затронутыми разделами roadmap, DOMAINS, policy RFC, WHITE_SPEC.

## 2. Requirements fit

| Цель | Оценка |
|------|--------|
| Нигде не создаётся впечатление, что **текущий** runtime / `domain_index.rs` уже реализует corporate `domain_lo = 0` как отдельную бизнес-модель | **Выполнено.** `docs/DOMAINS.md` в блоке «Strategic reserve note» прямо: изменений индекса нет, новых runtime-valid кодов нет; `CONCEPT_ROADMAP` отделяет roadmap от «текущий DOMAINS как runtime-справочник». |
| Различие **минимального** v0/v3 `INIT` и будущего **V4** расширения | **Выполнено.** `WHITE_SPEC_v0.md` помечает «V4 compatibility gap» сразу после таблицы полей MVP; `docs/rfc/6-policy-engine.md` §7.3.1 задаёт подзаголовок gap marker и черновой `CorporateInitExtension`, подчёркивая отсутствие финального wire. |
| `domain_lo = 0` как **неарендуемый** root/generic; `domain_lo > 0` как арендуемый/namespace | **Выполнено** в явных буллетах в `CONCEPT_ROADMAP`, `DOMAINS.md`, RFC §7.3.1 и в абзаце `WHITE_SPEC_v0.md`. |

**Небольшая двусмысленность (нит):** в `docs/CONCEPT_ROADMAP.md` подраздел «Принципы аренды доменов» формулирует, что «`domain_lo` — это арендуемый routing / authority identifier» без оговорки, что слот `domain_lo = 0` в corporate root/generic сценарии **не** относится к lease lifecycle. Читатель, который перескочит подраздел «`domain_lo = 0` и корпоративный `INIT`», может на секунду смешать общее правило с исключением. Рекомендация уровня nit: одна фраза-муфт или отсылка к предыдущему подразделу в «Принципах аренды».

## 3. Style and module shape

Правки соответствуют стилю существующих normative/roadmap документов: английские подзаголовки и термины там, где уже принято в соседних RFC; согласованные отсылки к V4/V5 backlog. Продуктовый Rust не затрагивался.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / normative finalized wire contract in this slice; RFC снабжён disclaimers «gap marker, not a final wire format»).

## 4. Safety

Документационный слайс: доверие к границам протокола задано через явные gap/disclaimer; изменений в коде, RPC или криптографии нет.

## 5. Tests

Не применимо к doc-only слайсу; проверка была содержательной вычиткой и согласованностью перекрёстных ссылок между четырьмя файлами.

## 6. Verdict

**PASS_WITH_NITS** — цели слайса выполнены; одна опциональная редакционная ниточка про согласование абзаца «Принципы аренды доменов» с явным исключением для root/generic `domain_lo = 0` в том же документе.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260516-domain-lo-zero-init-policy-gap-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 9500
  confidence: low
```

---

**Verdict (one line):** `PASS_WITH_NITS` — документация корректно разделяет текущий Phase 1B runtime и будущую V4-корпоративную модель; рекомендуется ослабить потенциальное пересечение формулировки «все domain_lo арендуемые» с подразделом про `domain_lo = 0`.

Подслайсовое ревью: **GLOSSARY.md** не обновлялся (не финальный gate спринта).

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260516-domain-lo-zero-init-policy-gap-review.md'
git add 'tasks/20260516-domain-lo-zero-init-policy-gap.json'
# git commit -m 'docs(slice): domain_lo=0 INIT gap review traceability'
```
