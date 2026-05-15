# Docs review: RFC 0011–0014 vs WHITE_SPEC v0 / v0-en (auto-claim focus)

**Date:** 2026-05-05  
**Agent:** pwm-review  
**Scope:** `docs/rfc/11-14-burn-purpose-through-api-error-contract.md`, `docs/WHITE_SPEC_v0.md`, `docs/WHITE_SPEC_v0-en.md` после правок по auto-claim materialization.

## 1. Scope recap

Сопоставление нормативных утверждений о **explicit/auto claim**, free-day, maturity base, ошибках/API и связи с высокоуровневым white-spec §9 (RU/EN). Цель — выявить противоречия и пробелы до кодинга E-1.

## 2. Requirements fit (consistency WHITE_SPEC ↔ RFC pack)

WHITE_SPEC §9.3–§9.4 по сути **повторяет** RFC 0011/0012/0013 по auto-claim и free-slot; явных столкновений по смыслу «auto-claim не потребляет free-slot» не обнаружено. При этом несколько нормативных деталей **есть только в WHITE_SPEC или только в части RFC**, что создаёт риск частичной реализации при работе «только по одному источнику».

## 3. Style

Документы RFC краткие и согласованы по терминологии auto/explicit в целом. В RFC 0014 обнаружена **редакторская недочистка**: дублирующаяся строка списка Claim-кодов (см. Findings Medium).

## 4. Safety / protocol

Не код-ревью. На уровне спецификации главный риск — **несогласованность фазовой модели** (preflight vs apply) для ошибок auto-claim (см. High).

## 5. Tests

Не применимо (docs-only). Рекомендация для E-1: сценарные тесты на порядок внутри блока и симметрию preflight/apply, если API обещает симметрию для не-ClaimTx с auto-claim.

## 6. Findings by severity

### High

1. **Maturity arithmetic baseline только в WHITE_SPEC.**  
   - **WHITE_SPEC_v0 §9.3 / WHITE_SPEC_v0-en §9.3:** базовая норма `1 PWM = 1 hour`, эквивалент блокам при `BLOCK_TIME_SEC = 1`.  
   - **RFC 0012 § Specification:** задаёт интервалы непрерывности и `claim_units`/anchor, но **не фиксирует скорость созревания** (нет формулы `matured_units_available`). RFC 0013 § Maturity arithmetic ссылается на `floor(matured_units_raw)` без определения raw.  
   - **Зазор:** кодер, держащийся только RFC 11–13, может ввести иную экономику времени без нарушения текста RFC. Согласование: либо перенести baseline-формулу в RFC 0012 или 0013, либо явно указать «числовая скорость maturity — см. WHITE_SPEC §9.3».

2. **Preflight ≡ apply для `error.code` vs auto-claim только в apply.**  
   - **RFC 0013 § Explicit claim vs auto-claim:** auto-claim «только в `apply`», отдельного mempool/preflight вердикта как tx нет.  
   - **RFC 0014 § Error Semantics:** при одинаковом входе и pre-state **`preflight` должен вернуть тот же `error.code`, что и `apply`**; для auto-claim reject требуются `tx_kind=claim`, `claim_mode=auto`.  
   - **Противоречие/дыра:** для транзакции, которая **не является** `ClaimTx` (например TRANSFER/STAKE), но при apply запускает auto-claim и может упасть с `E_AUTOCLAIM_COMPUTE_FAILED` или другим claim-классом, не описано, как клиент получает симметричный preflight-вердикт с тем же `error.code`/ shape reject JSON (в т.ч. обязательные поля минимального JSON для «вложенной» ошибки claim). Требуется явное правило: preflight выполняет тот же state-transition шаг, что и apply (включая materialization), или ослабить RFC 0014 формулировку для этого класса случаев.

### Medium

3. **Дубликат строки Claim mapping в RFC 0014.**  
   - **`docs/rfc/14-claim-burn-api-error-contract.md` § Stable code mapping baseline:** два буллета «Claim:» почти одинаковы; первый список **без** `E_AUTOCLAIM_COMPUTE_FAILED`, второй — **с**. Читатель не знает, какой авторитетный; это нужно слить в **одну** строку с полным набором.

4. **Два параллельных namespaces кодов ошибок без моста.**  
   - **RFC 0011 § Validation:** `INVALID_PURPOSE_*`, `FREE_CLAIM_DAILY_LIMIT`, `CLAIM_*`, `TX_SCHEMA_UNSUPPORTED` (без префикса `E_`).  
   - **RFC 0013/0014:** `E_*` policy/API классы, включая `E_FREE_CLAIM_DAILY_LIMIT`.  
   - **WHITE_SPEC §9:** не задаёт коды; отсылает к RFC pack. Нет таблицы **RFC0011 code → RFC0013/E_** (или явного утверждения, что 0011 — внутренние mempool-имена, а wire — только `E_*`).

5. **Список типов транзакций в WHITE_SPEC не включает `ClaimTx`.**  
   - **WHITE_SPEC §1 Goals / §3 Types:** перечислены только legacy v0 типы до `BURN_MARK`. **`ClaimTx` появляется только в §9.** Для онбординга имплементатора возможно ложное впечатление, что Claim вне базового набора тел; стоит одной фразой в §1 или §3 указать расширение v2/`ClaimTx` (со ссылкой на §9 и RFC 11).

### Low

6. **Именование баланса стейка.**  
   - **Account struct WHITE_SPEC §4:** `staked`; **RFC 0012 / WHITE_SPEC §9:** `staked_pwm_units`. Семантика, вероятно, одна; для тикетов/кода полезно зафиксировать канонический идентификатор поля состояния.

## 7. Сомнительные места / решения (прояснить до E-1)

- **Порядок эффектов в одном transition:** изменение stake **и** сразу же auto-claim в том же шаге применения tx — в какой момент считается continuity reset относительно расчёта matured в этом же шаге? RFC 0012 требует эквивалентности последовательному порядку в блоке; для **одной** txs с несколькими под-эффектами стоит явно указать порядок (сначала сброс/обновление стейка, затем accrued/matured, затем claim), если это уже не следует из существующей модели аккаунта.

- **`E_AUTOCLAIM_COMPUTE_FAILED`:** грань между консенсус-критической инвариантной ошибкой и локальной/node-internal сбоем вычислений не раскрыта; влияет на класс `TEMPORARY_UNAVAILABLE` vs `POLICY_REJECT`/`VALIDATION_ERROR` и на обязательность одинакового кода mempool/preflight.

- **`anchor_ref`/inclusion для auto-claim:** для не-ClaimTx клиент может не иметь явного якоря; как API/ответы трактуют якорную диагностику при auto-only пути — остаётся на стыке RFC 12/13/14 без примера сценария.

- **RFC 0012 «релевантная транзакция»:** охватывает ли она побочные пути начисления `marks_accrued` по блоку (не «tx», а block reward) если это меняет «баланс марок» аккаунта? WHITE_SPEC §5 описывает начисление марок по блоку; не ясно, считается ли producer reward / passive accrual «релевантной транзакцией» для auto-claim или материализация только при явных txs.

## 8. Verdict

**Approve with nits** — блокирующих логических противоречий в паре WHITE_SPEC§9 ↔ RFC по free-slot/auto-claim нет, но есть **High-уровень** зазор по **формуле maturity** и **симметрии preflight/apply для auto-claim failures**; перед E-1 либо уточнить RFC, либо ослабить/уточнить RFC 0014. Исправить дубликат списка в RFC 0014 (Medium editorial).

---

## Participation / token estimate (orchestrator)

```json
{
  "agent": "pwm-review",
  "result": "PARTIAL",
  "artifacts": "docs/reviews/v2-white-spec-rfc-autoclaim-consistency-20260505.md",
  "token_usage": {
    "source": "estimate",
    "input": 12000,
    "output": 2800,
    "total": 14800,
    "confidence": "medium"
  }
}
```

(`PARTIAL` = gate не FAIL, но есть High-сeverity нормативные зазоры, требующие правки доков или явного решения перед кодингом.)
