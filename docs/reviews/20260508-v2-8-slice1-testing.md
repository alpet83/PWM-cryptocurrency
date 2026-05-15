# Тестирование: V2-8 Slice 1 — wire schema и feature gates (same-shard sync v1)

**Дата:** 2026-05-08  
**Агент:** pwm-testing  
**Якорный коммит реализации:** `eb5fc5a` (`feat(pwmd): add same-shard sync v1 wire schema and feature gates`)  
**Тикет:** `tasks/20260508-v2-sprint8-slice1-wire-schema.json`

---

## Executive summary

Автоматические проверки вокруг изменений прошли успешно: `pwmd` собирается, целевые тесты transport/handshake зелёные. Декодирование нового каркаса `sync_headers_req` и сохранение обратной совместимости hello без `sync_profile` покрыты unit-тестами; режим **`LegacyObserve`** при отсутствии профиля подтверждён как в `handshake`, так и в `wire_decode`. Полноценная сетевая докачка блоков по slice 1 в scope не входит (stub-маршрутизация по тикету/ревью кодинга).

**Вердикт:** **PASS** (automated gates для данного слайса).

---

## Ссылки из тикета

| Путь | Статус |
|------|--------|
| `docs/rfc/15-same-shard-sync-v1.md` | файл присутствует |
| `docs/reviews/20260508-v2-8-slice0-review.md` | не перепроверялось содержимое; вход тикета валиден по имени |

---

## Preflight `target/debug`

- Скрипт: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1`
- Результат: успех; сообщение вида «target/debug … bytes (threshold 4096MiB)» без очистки.
- **removed:** no

*(Bash-вариант `tools/dev/preflight_target_debug.sh` на хосте не запускался: `pwsh` отсутствует в PATH; резервный PS1 достаточен.)*

---

## Команды и результаты

| Команда | Результат |
|---------|-----------|
| `cargo check -p pwmd` | **PASS** |
| `cargo test -p pwmd decode_` | **PASS** (6 тестов, включая `decode_sync_headers_req_ok`, `decode_legacy_hello_ok`) |
| `cargo test -p pwmd handshake` | **PASS** (12 тестов, включая `mode_legacy_without_profile`, `mode_full_with_valid_profile`) |
| `cargo test -p pwmd transport::` | **PASS** (10 тестов: wire_decode, production harness, peer_micro, dial trust) |

`cargo fmt` для данного слоя Rust не выполнялся: в диффе только `docs/` и `tasks/` (нет изменений в `crates/`).

---

## Покрытие по требованиям запроса

1. **Сборка / тесты изменённой зоны:** `cargo check -p pwmd` и фильтры `decode_`, `transport::` — выполнены, без падений.

2. **Handshake / legacy observe:**  
   - `handshake::tests::mode_legacy_without_profile` — ожидание режима без `sync_profile`.  
   - `transport::tests::wire_decode::decode_legacy_hello_ok` — JSON hello без поля профиля, `sync_mode() == LegacyObserve`.

3. **Новые wire-варианты:** минимально проверены через `decode_sync_headers_req_ok` (скелет `PeerWireMsg::SyncHeadersReq`).

---

## Риски и ограничения (не блокеры автоматической валидации)

- Интеграционных end-to-end сценариев «два процесса, полный sync v1 по сети» в этом слайсе нет — это согласуется с формулировкой тикета (каркас + gates + совместимость).
- Полный матричный прогон `cargo test -p pwmd` без фильтра не выполнялся; ограничение осознанное — фокус на transport/handshake около изменений.
