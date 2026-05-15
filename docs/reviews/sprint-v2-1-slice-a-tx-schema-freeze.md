# Sprint V2-1 — Slice A: Tx schema freeze (`purpose` + claim)

**Дата:** 2026-05-05  
**Статус:** RFC draft freeze (docs-only, без правок `crates/*`)  
**База:** [sprint-v2-1-slice-1-normative-freeze.md](./sprint-v2-1-slice-1-normative-freeze.md), [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md), [sprint-v2-1-rfc-inputs-20260505.md](./sprint-v2-1-rfc-inputs-20260505.md)

---

## 1) Scope и нормативный фокус

Этот слайс фиксирует tx-уровень для Sprint V2-1:

1. Нормативная схема **BurnMarkTx v2** с обязательным полем `purpose`.
2. Черновая, но стабильная по форме схема **ClaimTx** (free vs paid marker).
3. Черновик **stable error taxonomy** для tx/preflight.
4. Требования к детерминированной сериализации и валидации.
5. Правило совместимости с legacy tx, чтобы не ломать MVP при rollout.

Вне scope данного слайса: формулы state-аккаунтинга и полный policy matrix (Slice B/C), финальный API-формат ответов (Slice D).

---

## 2) BurnMarkTx schema v2 (normative freeze)

### A. Поля

- `tx_type`: `"burn_mark"`.
- `schema_version`: `2`.
- `purpose`: `string` (обязательное поле).

### B. Лимит и единица измерения (выбран один вариант)

- Константа: `PURPOSE_MAX_BYTES = 80`.
- Лимит считается **в байтах UTF-8** после нормализации.
- Поле валидно, если `1 <= utf8_len(purpose_norm) <= 80`.

Выбор фиксируется как единственный для Slice A (без альтернатив "графемы vs байты"), чтобы убрать неоднозначность в узле/API/preflight.

### C. Нормализация

`purpose_norm` вычисляется строго так:

1. Вход трактуется как валидный UTF-8.
2. Применяется только `trim` по Unicode White_Space на краях строки.
3. Внутренние пробелы и регистр не изменяются.
4. Дополнительная нормализация (NFC/NFD/NFKC/NFKD) **не** применяется.

### D. Запрещённые символы

`purpose_norm` не может содержать кодпоинты диапазонов:

- `U+0000..U+001F` (C0 controls),
- `U+007F..U+009F` (DEL + C1 controls).

Любой такой символ делает tx невалидной.

### E. Семантика поля

- Консенсус трактует `purpose` как непрозрачную метку и не интерпретирует бизнес-смысл.
- Рекомендация для операторов: не передавать PII в открытом виде; использовать off-chain форматирование (например, salted digest).

---

## 3) ClaimTx schema draft (normative draft for freeze)

### A. Обязательные поля

- `tx_type`: `"claim_mark"`.
- `schema_version`: `1`.
- `account_id`: `string` (идентификатор отправителя/владельца claim).
- `nonce`: `u64`.
- `mode`: enum `["free", "paid"]`.
- `claim_units`: `u64` (запрашиваемая к материализации целая дельта).
- `anchor_ref`: `u64` (опорный маркер/высота для детерминированной проверки дельты).
- `fee`: `u64`.
- `sig`: `bytes`/`string` (подпись по каноническому payload).

### B. Free-vs-paid marker

- Маркер режима: поле `mode`.
- Если `mode = "free"`, тогда `fee` должно быть `0`.
- Если `mode = "paid"`, тогда `fee` должно быть `> 0` и проходить policy-порог комиссии.
- Решение free/paid обязано быть единым в policy path (mempool + block apply + preflight).

### C. Минимальная валидация (Slice A baseline)

- Проверка обязательных полей и типов.
- `claim_units > 0`.
- Корректность `mode` и зависимости `mode`/`fee`.
- Корректный `nonce` относительно аккаунта.
- Детерминированная подпись на каноническом сериализованном payload.
- При `mode = "free"`: не нарушен лимит одна бесплатная claim за UTC-day (по chain time).

---

## 4) Error taxonomy draft (stable codes)

Минимальный стабильный набор кодов для tx/preflight:

- `INVALID_PURPOSE_LENGTH`
- `INVALID_PURPOSE_CHARS`
- `INVALID_PURPOSE_ENCODING`
- `CLAIM_REQUIRED_FIELD_MISSING`
- `CLAIM_MODE_INVALID`
- `CLAIM_FEE_MODE_CONFLICT`
- `CLAIM_NONCE_INVALID`
- `CLAIM_DELTA_INVALID`
- `FREE_CLAIM_DAILY_LIMIT`
- `TX_SCHEMA_UNSUPPORTED`

Требование стабильности: символьные коды не переименовывать между узлом и API при переходе к Slice D; расширение набора допускается только аддитивно.

---

## 5) Deterministic serialization / validation notes

1. `schema_version` обязателен для tx-домена.
2. Сериализация полей фиксируется в каноническом порядке:
   - `tx_type`, `schema_version`, затем остальные поля в предопределённом порядке схемы.
3. Строки кодируются только UTF-8.
4. Для `BurnMarkTx`, в сериализацию и подпись попадает `purpose_norm` (уже trim + валидация символов).
5. Никаких локальных часов клиента в правилах free/paid; только chain time блока включения.
6. Одинаковый validation verdict в mempool, preflight и block apply обязателен.

---

## 6) Backward-compat strategy (legacy tx)

Чтобы не ломать MVP при rollout:

1. Узел временно поддерживает оба формата BurnMarkTx:
   - **legacy v1**: без `purpose`,
   - **v2**: с обязательным `purpose`.
2. Для legacy v1 применяется адаптер совместимости:
   - вычисляется `purpose_norm = ""` (пустая метка),
   - legacy tx не отклоняется только из-за отсутствия `purpose`.
3. Новые клиенты и preflight по умолчанию формируют только v2.
4. ClaimTx вводится как новый тип и не меняет валидность старых не-claim tx.
5. После стабилизации сети deprecation legacy v1 выносится в отдельный RFC/ADR (вне Slice A).

---

## 7) Decision log (Slice A)

1. Лимит `purpose` зафиксирован в **80 UTF-8 байт** после trim.
2. Запрещены control-символы C0/C1 (`U+0000..U+001F`, `U+007F..U+009F`).
3. Выбран минимальный детерминированный пайплайн нормализации без Unicode composition transforms.
4. Для ClaimTx зафиксирован обязательный marker `mode` (`free|paid`) и связка с `fee`.
5. Принята минимальная стабильная error taxonomy с кодом `FREE_CLAIM_DAILY_LIMIT`.
6. Зафиксирована стратегия совместимости с legacy BurnMarkTx v1 через адаптер.

---

## 8) Handoff notes for next slices

- Slice B: уточнить семантику `anchor_ref` и state-поля для расчёта `claim_units`.
- Slice C: формализовать edge-cases (reorg, partial balance movement, race around UTC boundary).
- Slice D: привязать стабильные коды к финальным API-ответам/JSON схемам.
