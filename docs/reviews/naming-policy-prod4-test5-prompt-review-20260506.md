# Ревью: разделение бюджета имён (прод ≤4 / тесты ≤5) — правки промптов

**Дата:** 2026-05-06  
**Область:** только документация агентов (`docs/AGENT_PROMPT_*.md`, `docs/AGENT_PROMPTS.md`). Прод-код не менялся.

## 1. Scope recap

Владелец зафиксировал намерение: **hard cap ≤ 5 «слов» (сегментов `snake_case`)** относится **к тестовому коду**; для **продакшена** действует **более строгое** ограничение. Ранее формулировки слились: и прод, и тесты получили «≤ 5». Нужно **исправить промпты** и **перезапустить ревью** относительно новой политики.

## 2. Requirements fit

| Файл | Ожидание | Результат |
|------|-----------|-----------|
| `AGENT_PROMPT_coding.md` §Style | Прод: **≤ 4**; тесты: **≤ 5**; раздельный self-audit | Выполнено |
| `AGENT_PROMPT_testing.md` §Naming | Явно: тесты **≤ 5**, прод — по coding (**≤ 4**) | Выполнено |
| `AGENT_PROMPT_review.md` §Deliverable (3) | Ревьюер проверяет **разные** потолки | Выполнено |
| `AGENT_PROMPTS.md` | Краткий указатель без смешения правил | Выполнено |

Добавлена оговорка в coding: старые спринтовые документы могли говорить «≤ 5 для прод» — **текущий промпт имеет приоритет**.

## 3. Style и последствия для кода

Под новым **`pwm-review`** прод-символ с **5 сегментами** формально **вне политики** (если нет явного waiver на слайс).

**Пример из текущего дерева:** `summarize_pwmd_tx_reject_json` в `crates/pwm-core/src/reject_wire.rs` — **5 сегментов** → под **≤ 4 для прод** требует переименования от **`pwm-coding`** (и правки re-export / вызовов в `pwm-cli`, `pwm-tui`). Это **не блокирует** настоящий документный слайс; это **отложенная работа** по коду.

Исторические отчёты (`sprint-15-slice-R-*`, `v2-e3-review-*`) **не переписывались** — они отражают политику на момент слайса.

## 4. Safety

Не применимо (только Markdown).

## 5. Tests

Не применимо. После переименования прод-хелперов при необходимости прогнать затронутые **`cargo test`** пакеты.

## 6. Verdict

**PASS** для изменений промптов: формулировки согласованы и устраняют смешение «одного потолка для всех».

**Approve with nits (на будущее):** завести отдельный тикет на приведение прод-API к **≤ 4** сегментов там, где сейчас ровно 5 (например `summarize_pwmd_tx_reject_json`).

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS
artifacts: docs/reviews/naming-policy-prod4-test5-prompt-review-20260506.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 8000, "confidence": "low" }
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/AGENT_PROMPT_coding.md'
git add 'docs/AGENT_PROMPT_testing.md'
git add 'docs/AGENT_PROMPT_review.md'
git add 'docs/AGENT_PROMPTS.md'
git add 'docs/reviews/naming-policy-prod4-test5-prompt-review-20260506.md'
git commit -m 'docs(agents): prod fn names <=4 segments, tests <=5; review note'
```
