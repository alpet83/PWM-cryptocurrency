# Review: CUP запрос ограничен одной эпохой (`sync_live` / attester stall)

Тикет: `tasks/20260517-attester-sync-stall-at-4pct.json`  
Фокус: `crates/pwmd/src/transport/peer_session/sync_live.rs` — `cup_req_range`, обработка NACK `catchup_epoch`, метрика `nack:catchup_epoch`, тест `cup_req_stays_in_epoch`.  
RCA: `tasks/20260517-attester-sync-stall-at-4pct-debug.md`.

## 1. Scope recap

Устранение зацикливания CUP после рестарта с частичным снапшотом (`tip_h≈544`), когда окно запроса пересекало границу эпохи (`EPOCH_SPAN = 1000`) и responder стабильно отвечал `catchup_epoch`. Ожидания слайса: кламп диапазона до `epoch_range(epoch_idx(from_h)).last_h`, для `catchup_epoch` — сброс отложенного ретрая (`cup_next_ms = 0`, `cup_try = 0`), отдельное ведро причины в `sync_cup_fail_reason` (JSON: `sync_cup_fail_reason_total`), регрессионный тест сценария `544 → 545..1000` при большом peer tip.

Связь с дорожными картами: точечный фикс транспортного синка (same-shard sync), без смены специфики wire-сообщений.

## 2. Requirements fit

**Согласованность с `on_cup_req`.** Сервер отклоняет запрос, если `epoch_idx(start_h)` или `epoch_idx(end_h)` не совпадает с переданным `epoch_id` (оба должны быть в одной эпохе и совпасть с заявленной). Клиент задаёт `from_h = local_h + 1`, `to_h = min(from_h + lag_win - 1, epoch_last)`, `epoch_id = epoch_idx(from_h)` (`send_cup_req`). После клампа `start` и `end` лежат в интервале `[epoch_first_h, epoch_last_h]` для индекса `epoch_idx(from_h)`, поэтому `epoch_idx(start_h) == epoch_idx(end_h) == epoch_idx(from_h) == epoch_id`. От off-by-one с `epoch_range`: для индекса 0 высоты `1..=1000`, пример RCA `545..1000` корректен.

**Размер окна.** Проверка `end_h - start_h + 1 <= SYNC_CUP_WIN_CAP` на стороне сервера остаётся выполнимой: после клампа длина не превышает размер окна до границы эпохи (в этом сценарии 456 блоков при лимите 4096).

**Немедленный ретрай при `catchup_epoch`.** После NACK состояние CUP снимается, но счётчик попыток и таймер сбрасываются так, чтобы следующее обновление tip могло сразу заново попытаться CUP с уже эпохальным диапазоном — это согласуется с RCA (не упираться в backoff после исправления окна).

**Пробелы относительно полного RCA-чеклиста.** В тексте RCA перечислены также экспозиция полей синка в dev API и responder-side негативные тесты `on_cup_req`; они не входят в заявленный объём этого слайса и не блокируют приёмку заявленного фикса.

## 3. Style and module shape

Структура модуля не раздута; добавлен компактный хелпер `cup_req_range` рядом с существующей логикой CUP.

Запуск `python scripts/check_rust_fn_name_segments.py crates/pwmd/src/transport/peer_session/sync_live.rs`: нарушений политики длины имён не найдено.

Комментарии к изменениям — по делу (английский в производственном коде сохранён в духе файла).

## 4. Safety

Поверхность протокола: те же типы сообщений и те же текстовые коды NACK (`catchup_epoch`, `catchup_range`, …); меняются только высоты клиентского запроса и детализация метрик — не повышают доверие к peer и не добавляют неограниченных аллокаций.

`epoch_idx(from_h)?` теоретически возвращает ошибку только при `height == 0`; при `head_h > local_h` получается `from_h >= 1`, то есть ошибка недостижима на нормальном контуре — при аномальном состоянии цепочки ошибка пробросится из `maybe_start_cup` (приёмлемо как защита).

Риск задержки прогресса при одной эпохе за запрос сохранён по дизайну: после догрузки до `epoch_last` последующие tip-апдейты запустят CUP для следующей эпохи — это ожидаемое поведение и лучше, чем вечный NACK.

## 5. Tests

**Покрыто:** `cup_req_stays_in_epoch` интеграционно проверяет реальный проводной путь через `maybe_start_cup`: при `tip_h = 544` и удалённом `head_h = 4640` в кадре `SyncCatchupReq` ожидаются `start_height == 545`, `end_height == 1000` и согласованный `epoch_id` с `epoch_idx(545)`. Это прямое соответствие числам из RCA.

**Не покрыто в этом диффе (ниты):**

- Автоматическая проверка ветки `on_nack` для `catchup_epoch` (сброс `cup_try` / `cup_next_ms` и счётчик `nack:catchup_epoch`).
- Отдельный unit/integration тест responder: cross-epoch запрос по-прежнему даёт NACK `catchup_epoch` (был явный wishlist в RCA).
- Явная проверка границ «последний блок эпохи» и «переход на вторую эпоху» в одном-двух маленьких тестах — усилило бы регресс по клампу без тяжёлого harness.

Эти пробелы не противоречат заявленной приёмке слайса, но снижают уверенность при будущих рефакторах вокруг NACK/CUP state machine.

## 6. Verdict

**Approve with nits** — логика клампа и `epoch_id` согласованы с `on_cup_req` и моделью `EPOCH_SPAN`; wire-схема и коды ошибок без семантической смены протокола. Рекомендуется последующее доболнение тестами на NACK/`on_cup_req`, если нужна полная трассируемость к списку тестов из RCA.

**Однострочно для оркестратора:** PASS_WITH_NITS — логика и ключевой RCA-тест ок; добавить при желании автотесты на `on_nack`/`on_cup_req`.

**Wire / semver:** версию рукопожатия или поля сообщений менять не требуется; совместимость старых нод сохранена (старее могущие отправлять cross-epoch по-прежнему получат `catchup_epoch` от нового responder).

---

## Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/20260517-attester-cup-epoch-clamp-review.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 12000,
    "confidence": "low"
  }
}
```

Это финальное ревью спринта для словаря: **нет** — блок Glossary не применим; `docs/GLOSSARY.md` не меняем.

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260517-attester-cup-epoch-clamp-review.md'
git add 'tasks/20260517-attester-sync-stall-at-4pct.json'
git commit -m 'docs: CUP epoch clamp review + ticket pwm-review artifact'
```
