# `pwm-tui`: техническая документация

`pwm-tui` — текстовый UI-клиент для devnet PWM. Он показывает состояние сети через RPC ноды, дает навигацию по аккаунтам и содержит MVP-заглушки модальных действий.

## Роль и границы

**Роль `pwm-tui`**
- интерактивный read-mostly клиент поверх HTTP API;
- визуализация аккаунтов сети в таблицах `Owner`/`Receivers`;
- быстрый доступ к выбранному аккаунту и его полям (`id`, `nonce`, `initialized`, `active/dormant policies`, `finalized`, `rescue`, owner metadata), опционально raw JSON.

**Граница с `pwmd`**
- `pwm-tui` не хранит chain-state и не запечатывает блоки;
- все данные берутся из RPC `pwmd` (`/v1/head`, `/v1/accounts`, `/v1/account/:id`);
- ошибки доступности backend отображаются в footer как `head: offline` / `accounts: offline`.

**Граница с `pwm-core`**
- `pwm-core` в TUI напрямую не вызывается;
- детерминированная доменная логика (tx/state/crypto) остается в `pwm-core`, TUI выступает только как UI-слой.

## Runtime-флаги и env

- `PWM_RPC` — базовый URL RPC, по умолчанию `http://127.0.0.1:3030`.
- `PWM_TUI_DEBUG` — включает debug-панель с JSON выбранного аккаунта.
  - truthy-значения: `1`, `true`, `yes`;
  - в debug-режиме в интерфейсе добавляется блок `debug JSON` и пометка в footer.

## Поток получения данных (`poll_data`)

Цикл обновления выполняется примерно раз в 1 секунду.

1. Вычисляется базовый URL из `PWM_RPC`.
2. Запрос `GET /v1/head`:
   - при успехе обновляется строка `head` (`height`, `tip`);
   - при ошибке ставится `err = "head: offline"`.
3. Запрос `GET /v1/accounts`:
   - при успехе формируется `rows` (`id`, `balance_pwm`, `staked`, `marks`, `initialized`, `nonce`);
   - поля чисел читаются tolerant-образом (строка или число, иначе `0`);
   - при ошибке (если `err` еще пустой) ставится `err = "accounts: offline"`.
4. Для выбранной строки:
   - всегда обновляется `detail_line`: `selected`, `init`, `nonce`, `active_policies`, `dormant_policies`, `finalized`, `rescue`, owner metadata summary;
   - в debug-режиме дополнительно вызывается `GET /v1/account/:id`, результат форматируется в pretty JSON.

## Архитектура UI

## Состояние

`Ui` хранит:
- `head` — краткий статус цепочки;
- `rows` — список аккаунтов из `/v1/accounts`;
- `detail_line` — строка выбранной записи;
- `debug_detail` — JSON выбранного аккаунта (только debug);
- `err` — последняя ошибка опроса.

Доп. runtime-состояние:
- `owner_sel`, `recv_sel` — индексы выделения в левой и правой панелях;
- `active: Panel` (`Owner`/`Receivers`) — текущий фокус;
- `modal: Option<&'static str>` — состояние активной модалки-заглушки.

## Панели, фокус и selection model

- Экран разделен на две таблицы: `Owner` (левая) и `Receivers` (правая).
- Активная панель подсвечивается желтой рамкой.
- Выделение:
  - `Owner` показывает только первую запись (`take(1)`);
  - `Receivers` показывает оставшиеся записи (`skip(1)`).
- При смене данных используется `clamp_sel`, чтобы индексы выделения не выходили за границы.
- Если выбранный элемент в правой панели недоступен, используется fallback на первую запись.

## Footer и модалки

- Footer показывает:
  - `head` (height/tip);
  - подсказки по управлению;
  - текущий `PWM_RPC`;
  - маркер `PWM_TUI_DEBUG=1` в debug-режиме;
  - текст последней ошибки опроса (если есть).
- Модалки:
  - `F5` — статус-only подсказка для burn через CLI (`tx-burn-mark`);
  - `F6` — форма `send` (`from/to/amount/fee/confirm`) с локальной валидацией:
    - `from` берётся из текущей выделенной строки `Owner`; wallet v3 не требует `active_account_id_hex`, и это legacy-поле не является runtime-источником sender в TUI;
    - перед submit TUI проверяет, что выбранный Owner можно подписать текущим материалом кошелька; locked encrypted wallet, отсутствующий master seed для non-root/multi-account derivation или аккаунт вне wallet блокируют отправку с явным статусом; verified legacy/root-key fallback может сработать только для совместимых root/default случаев;
    - same-domain: submit в `POST /v1/tx`;
    - cross-domain: submit в `POST /v1/roaming-intents` на текущий native/source RPC + lifecycle статус (`queued/exported/relayed/imported/expired/failed`); после `relayed` — автоматическая отправка **`POST /v1/tx` (Import)** с source RPC (подпись ключом получателя `to` из wallet); для nonce/баланса на стороне получателя при необходимости используется **target** RPC: **`PWM_TUI_TARGET_RPC`**, иначе эвристика порта от `PWM_RPC`; **шаг 5** — отображение проверки изменения баланса на target (ожидаемый кредит = `amount`, без fee); target peer для relay/handoff по-прежнему достигается `pwmd` через configured trusted seed;
  - закрытие информационных модалок: `Enter`/`Esc`.

## Модель ввода

Базовые клавиши:
- `Tab` — переключение фокуса между `Owner` и `Receivers`;
- `Up`/`Down` — перемещение выделения в активной панели;
- `F5` — открыть TODO-модалку "burn/send";
- `F6` — открыть send-форму;
- `F10` или `q` — выход из приложения.

Когда открыта модалка:
- `Enter`/`Esc` — закрыть модалку;
- `F10`/`q` — немедленный выход из TUI;
- остальные клавиши игнорируются.

## Поведение debug-режима

При `PWM_TUI_DEBUG`:
- в layout добавляется дополнительная вертикальная зона (~35%) для `debug JSON`;
- для текущего выбранного аккаунта выполняется `GET /v1/account/:id`;
- ответ рендерится как pretty JSON;
- в footer добавляется явная пометка `PWM_TUI_DEBUG=1`.

Без debug:
- панель `debug JSON` отсутствует;
- `debug_detail` очищается;
- интерфейс остается компактным (две таблицы + detail + footer).

## Известные ограничения и временные решения

- Разделение `Owner`/`Receivers` основано на эвристике:
  - первый аккаунт из `/v1/accounts` считается owner;
  - остальные считаются receivers;
  - это временно до появления явной модели "мой кошелек/контакты".
- `F5` остаётся status-only.
- `F6` выполняет one-window cross-domain send через roaming-intent API на native/source node и завершает поток подписанным Import (см. §Footer/F6 выше).
- Relay/handoff на transport-уровне делает source `pwmd` через trusted configured seed; **HTTP target** для чтения счёта получателя и шага 5 задаётся **`PWM_TUI_TARGET_RPC`** при несовпадении портов с эвристикой. Ручной fallback остаётся CLI-only (`tx-handoff-register` + `tx-import`) и требует trusted peer context на target.
- Источник truth данных для списков — только RPC ноды; локального профиля адресов в TUI пока нет.

## Быстрая карта расширения TUI

1. **Явная модель владельца/получателей**: заменить "first row heuristic" на данные профиля/конфига пользователя.
2. **Реальные modal workflows**:
   - `F6`: форма перевода (from/to/amount/fee) + отправка в `POST /v1/tx`;
   - `F5`: согласованная операция burn/mark по финальной спецификации.
3. **Расширение данных панели**: добавить историю и/или фильтры при появлении нужных endpoint.
4. **UX-слой ошибок**: кроме footer, дать контекстные статусы в модалках и retry-поведение.
5. **Синхронизация с `docs/TUI_SPEC_v0.md`**: держать этот файл как "as-implemented", а спецификацию — как "target".
