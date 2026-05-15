# RFC nits closure — независимый gate (V2-8)

**Дата:** 2026-05-08  
**Зона:** только документация (`docs/rfc/15-same-shard-sync-v1.md`).  
**Референс-коммит docs-fix:** `5b87b15` — *docs(rfc): lock deterministic P0 baseline and finalized semantics for V2-8 waves*.

**Вход `docs/reviews/20260508-v2-8-rfc-nits-closure-note.md`:** файл в дереве репозитория не найден; проверка выполнена по актуальному тексту RFC в `HEAD` и диффу указанного коммита.

---

## 1. Scope recap

Проверка трёх заявленных закрытий нитов после docs-fix шага:

1. Явный **MUST NOT** на недетерминированный источник ожидаемого пропозера.
2. Достаточно явная семантика **`finalized_height`** для воспроизводимости / wave-тестов.
3. Явное примечание, что **квалификатор сетевой зоны на wire для v1 не обязателен** (interoperability без обязательного поля в конверте §6).

---

## 2. Requirements fit

### 2.1 Детерминизм пропозера (MUST NOT на эвристики)

**Закрыто.** В §7.1 пункт 3 нормативно зафиксировано:

- решения по eligibility пропозера **MUST** опираться только на `(height, validators_fixed_order)`;
- **MUST NOT** зависеть от локальных эвристик (`avg_peer_count`, «кто первый в чате», порядок прихода, гонки по wall-clock).

Плюс пункт 1 даёт явную формулу `expected_proposer = validators_fixed_order[height % N]`, что привязано к P0 baseline волновых сценариев.

### 2.2 `finalized_height` для wave-тестов

**Закрыто на уровне спецификации для тестового дизайна.** После параграфа про кортеж fork-choice добавлен блок «For MVP v1, receivers MUST apply bounded semantics for `finalized_height`» с тремя пунктами:

- **Источник:** `TipAnnounce`, смысл — peer-local finalized prefix на канонической ветви пира.
- **Монотонность по сессии:** неубывание; уменьшение — stale, игнор.
- **Ограниченное использование:** только после валидации, clamp `<= remote_head_height`, без отката ниже локального finalized префикса.

В связке с §6.4 (late/out-of-slot относится к высотам ниже локального `finalized_height`; stale-ответы не трогают fork-choice входы) поведение для сценариев «волны» задаётся однозначнее, чем в версии до `5b87b15`.

### 2.3 Необязательность net-zone на wire (v1)

**Частично / нит.** Операционная модель зон и сегментов есть в §4.3 и §13 (логический сегмент, `local_segment_id`, ingress kinds, без новых wire message types для storm guard — §4.3). Однако **отдельной нормативной строки уровня** «в конверте §6 нет обязательного поля net-zone / network zone задаётся `local_segment_id` и классами пиров из политики оператора» **в тексте RFC нет**. Смысл читателю выводим из §13, но критерий «explicit note» из architecture gate (см. `docs/reviews/20260508-pretest-architecture-gate-p0-p2.md`) формально не выполнен дословно.

---

## 3. Style

Документ RFC сохраняет согласованную нумерацию (новая §6.4 до раздела 7), стиль MUST/MUST NOT выдержан. Замечаний по оформлению в рамках gate нет.

---

## 4. Safety

Изменения нормативные (late/out-of-slot, stale responses, семантика `finalized_height`); они **сужают** недопустимые интерпретации и снижают риск тихих расхождений между нодами при тестах — с точки зрения спецификации это плюс. Рисков «дыр» в связке §6.4 ↔ §7 не выявлено при чтении; противоречий с §7.3 не видно.

---

## 5. Tests

Gate касается только RFC. Исполняемые тесты не запускались. Для wave-тестов рекомендуется явно зафиксировать в тест-плане опору на §7.1 п.1/3, §7.3 `finalized_height` bullet list и §6.4.

---

## 6. Verdict

**PASS_WITH_NITS** — P0 по детерминизму пропозера и по `finalized_height` закрыты; остаётся **документный нит**: одна явная фраза про отсутствие обязательного net-zone / segment поля в §6 envelope (как операционное vs wire), если владелец хочет буквальное закрытие формулировки из pretest gate.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260508-v2-8-rfc-nits-gate.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 8000
  confidence: low
```

---

## 8. Git handoff для оркестратора

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-8-rfc-nits-gate.md'
git commit -m 'docs(review): RFC15 nits closure independent gate'
```
