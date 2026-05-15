# Hotfix validation: `tx_batch_profile_drop` (slice 5)

**Date:** 2026-05-08  
**Ticket:** `tasks/20260508-v2-slice5-hotfix-profile-drop-test.json`  
**Agent:** pwm-testing  

## Summary

На проверенном **`HEAD` репозитория hotfix не подтверждён**: в `crates/pwmd/src/transport/peer_session/mod.rs` тест `tx_batch_profile_drop` по-прежнему ищет ключ **`shard_mismatch`** в `sync_tx_drop_reason_total`, тогда как для `route_test(..., full_v1: false, same_shard: true)` счётчик увеличивается по **`profile_mismatch`** (поведение согласовано с нормализацией reason-кодов; см. контекст в `docs/reviews/20260508-v2-8-slice5-review.md`).

**Итоговый статус автоматических проверок:** **FAIL** до появления коммита pwm-coding с заменой ожидаемого ключа (или разделением сценариев shard vs profile).

## Верифицированный коммит

```
53fab53432fee5f1d037b237a0b7c79c5e0fcfca
```

## Команды

| Команда | Результат | Примечание |
|--------|-----------|------------|
| `cargo check -p pwmd` | **PASS** | ~0.2s |
| `cargo test -p pwmd tx_batch_profile_drop` | **FAIL** | `assertion left == right` на строке ~900 (`get("shard_mismatch")` → `None`) |
| `cargo test -p pwmd peer_session::tests` | **PARTIAL** | **14 PASS / 1 FAIL** — единственное падение `tx_batch_profile_drop` |

Hang watchdog: не срабатывал; длительность прогонов порядка долей секунды.

## Preflight `target/debug`

- **Скрипт:** `tools/dev/preflight_target_debug.ps1` (Windows PowerShell); `bash` / `pwsh` в сессии недоступны — оболочка WSL/bash для `.sh` не сработала.
- **Вывод:** размер каталога под порогом (`226464982` bytes при лимите 4096 MiB); `removed: no`.

## Очистка процессов

Запущенные тесты завершились штатно; фоновых `pwmd` / `pwm-tui` не поднималось (`cleaned: yes`, нечего завершать).

## Рекомендация pwm-coding

Одна правка ожидания: `shard_mismatch` → `profile_mismatch` в `tx_batch_profile_drop`, либо два узких теста с явными предпосылками shard vs legacy profile gate.

---

## Machine handoff (оркестратор)

- `agent`: pwm-testing  
- `result`: **FAIL**  
- `artifacts`: `docs/reviews/20260508-v2-slice5-hotfix-testing.md`  
- `commands`: см. таблицу выше; watchdog: нет  
- `cleanup`: cleaned yes; процессы не оставлялись  
- `preflight_target_debug`: `powershell tools/dev/preflight_target_debug.ps1`, under threshold, `removed: no`  
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 15000, "confidence": "low" }`  
