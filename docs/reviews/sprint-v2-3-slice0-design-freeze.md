# Sprint V2-3 Slice 0: design freeze + schema prep

Дата: 2026-05-06  
Тикет: `tasks/20260506-v2-sprint3-emission-whales.json`  
План: `docs/plans/mvp_v2.md` (Sprint V2-3)

## 1) Freeze полей `GenCfg` для V2-3

В `pwm-core` закреплены новые конфиг-поля (подготовка контракта, без активации новой эмиссионной формулы в этом слайсе):

- `policy_ver: u32` — версия reward policy.  
  - legacy default: `1` (`LEGACY_POLICY_VER`)
- `pwm_stake_min: u128` — порог стейка для будущей эмиссии PWM (киты).  
  - default: `100000`
- `marks_stake_min: u128` — минимальный стейк для будущей эмиссии/начисления marks.  
  - default: `1`
- `season_enabled: bool` — флаг включения сезонности.  
  - default: `false`
- `season_coeff_ppm: u128` — сезонный коэффициент в ppm (1_000_000 = 1.0).  
  - default: `1000000`

Принцип Slice 0: defaults выбраны так, чтобы текущий reward-path фактически оставался legacy и не менял поведение сети до Slice 1/2.

## 2) Детерминизм (норматив для реализации Slice 1+)

Разрешенные входы для расчета reward/emission:

- `height` (высота блока),
- `header.ts` (timestamp из заголовка блока),
- детерминированное состояние на этом шаге (stake/validators/аккаунты из state),
- поля `GenCfg` из genesis.

Запрещенные входы:

- wall-clock (`SystemTime::now`, локальное время ОС, timezone-хуки),
- RNG/случайность,
- внешние I/O источники при расчете консенсусной логики.

Это сохраняет replay-детерминизм и совместимость snapshot re-validation.

## 3) Schema migration (`genesis`) и backward rule

Freeze-решение для Slice 0:

- целевая версия genesis schema: `5`,
- backward-совместимость в `pwmd` loader: принимать `schema_version` `4` и `5`,
- для `schema_version=4` отсутствующие V2-3 поля должны получать default-значения (см. раздел 1),
- для `schema_version=5` поля сериализуются и парсятся явно.

Минимальная реализация в Slice 0:

- parser в `pwmd` обновлен до поддержки `4/5` и default-fallback,
- `genesis-build` в `pwm-cli` пишет `schema_version: 5` и новые поля с legacy-safe значениями,
- бизнес-формула reward в runtime не переключается в этом слайсе.

## 4) Non-goals Slice 0

- Не внедряется новая формула распределения PWM «китам».
- Не включается сезонный расчет по календарю в runtime.
- Не меняется текущая точка/порядок начисления в `Chain::seal`.
- Не добавляются новые e2e/unit тесты формулы эмиссии (перенесено в Slice 1/2).
- Не выполняется миграция historical snapshot форматов beyond schema fallback.

## 5) Контракт на следующий слайс (Slice 1)

Slice 1 реализует только бизнес-логику на уже зафиксированном контракте:

- использование `policy_ver` как переключателя на новую формулу,
- применение `pwm_stake_min` и `marks_stake_min` в расчетах,
- подключение `season_enabled` + `season_coeff_ppm` к детерминированному пути от `header.ts`,
- тесты граничных кейсов (threshold/season).
