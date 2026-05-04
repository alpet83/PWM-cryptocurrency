# Review: модульные баннеры `//!` + аудит имён тестов (тикет `20260614`)

**Commits (часть coding):** `7caef1ad48483f72cbafb62acbbe97c7caccfd7c`; обновление тикета **`a9917dcc`** (полный SHA см. **`tasks/20260614-s15-module-banners-test-names-style.json`**).

## Scope recap

Тикет **`tasks/20260614-s15-module-banners-test-names-style.json`**: (1) английские **`//!`** по промпту coding — **PASS** (**pwm-coding**); (2) аудит имён тестов ≤ 5 сегментов по воркспейсу — **pwm-review**.

## Requirements fit — баннеры

Цель части 1 выполнена; выборочная проверка согласуется с handoff (**57** файлов).

## Style — имена тестов

По эвристике и выборке (**pwm-review**, Ask mode): имена **`#[test] fn`** и хелперов **массово** превышают бюджет **≤ 5** сегментов в **`pwm-cli`**, **`pwm-tui`**, **`pwm-core`**, частях **`pwmd`** вне уже переработанных деревьев wave **N.1**. Это **накопленный** долг; исправление вынесено в микро-слайс **S15-N** (волна **N.1** закрыта для **`pwmd`** test trees).

## Tests

Полный **`cargo test --workspace`** в сессии ревью не повторялся; зафиксирован лок **`pwm-tui.exe`** на Windows. Последующая приёмка владельцем и оркестратором подтвердила сохранность функциональности.

## Verdict

**PARTIAL** — баннеры ок; именование тестов в масштабе монорепозитория **не** приведено к конвенции (адресуется волнами **S15-N**).

## Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts:
  - docs/reviews/sprint-15-module-banners-test-names-review.md
token_usage:
  source: estimate
  total: 11500
  confidence: low
```
