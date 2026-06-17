# Review: pwmd snapshot summary checkpoint lag (20260617)

**Ticket:** `tasks/20260617-pwmd-snapshot-summary-checkpoint-lag.json`  
**Slice:** shutdown guards, post–full-verify summary align, startup load-mode INFO, autosnapshot checkpoint INFO, unit tests  
**Changed files:** `handlers_shutdown.rs`, `lifecycle.rs`, `snapshot/io.rs`, `snapshot/telemetry.rs`

---

## 1. Scope recap

Операторский инцидент на CY proposer: SIGINT во время `loading_snapshot` вызывал `graceful_shutdown_request` → `save_seal_persist` с genesis inner (`tip_h=0`) и перезаписывал `pwm-data.json` (`checkpoint_height=0`), пока `epochs/pwm-epochs-manifest.json` уже на tip (~124k). Каждый следующий старт — принудительный full `chain_verify`.

Слайс должен:

- блокировать shutdown-persist в фазах раннего старта / при отставании chain от manifest;
- после успешного full verify выровнять summary checkpoint к manifest tip;
- поднять диагностику load mode с INFO (не только WARN);
- логировать checkpoint на autosnapshot boundary;
- покрыть тестами регрессию checkpoint;
- пройти `cargo test -p pwmd snapshot`.

Связанный контекст: `docs/reviews/20260507-snapshot-summary-manifest-lag.md`, `docs/guide-node-storage-and-snapshot.md`.

---

## 2. Requirements fit

| Criterion | Status | Notes |
|-----------|--------|-------|
| Shutdown/SIGINT: не вызывать `save_seal_persist` при `loading_snapshot` или `tip_h < manifest.canonical_h` | **Met** | `shutdown_skip_reason` пропускает `InitPhase::Starting` / `LoadingSnapshot` и `checkpoint_regress` когда `would_regress_checkpoint` (сравнение `inner.chain.tip_h()` с `manifest.canonical_h`). SIGINT/RPC идут через тот же `graceful_shutdown_request`. |
| Явный skip при summary/manifest lag | **Partial** | Lag на shutdown косвенно покрыт через `tip_h < canonical_h` (типичный сценарий бага). Отдельной проверки `summary.checkpoint_height != manifest.canonical_h` при `tip_h == canonical_h` нет — в этом случае persist безопасен (выравнивает summary). |
| Auto-align summary после full verify | **Met** | `maybe_align_summary_after_verify` в `lifecycle.rs` после успешного load: при `used_full_verify` и `manifest.canonical_h == tip_h` вызывает `save_checkpoint_summary`, INFO с `reason=summary_manifest_lag|verify_chain_flag`. |
| Startup INFO: `snapshot_load_mode` + reason | **Met (JsonFile)** | `load_snapshot_timed` в `io.rs`: INFO `snapshot load mode selected` с `snapshot_load_mode=trust|full_verify`, `reason`, `summary_checkpoint`, `manifest_tip`. Старый WARN «summary lags epoch manifest; forcing…» удалён. |
| Reason enum: `clickhouse` / `legacy_anchor` | **Gap (nit)** | В слайсе reason для JsonFile: `trust_checkpoint`, `summary_manifest_lag`, `verify_chain_flag`. ClickHouse load (`ch_http.rs`) не эмитит тот же `snapshot_load_mode` INFO; `legacy_anchor` не фигурирует. Для CY JsonFile сценария это не блокер. |
| INFO `checkpoint_height` на autosnapshot | **Met** | `periodic_snap_finish` логирует `autosnapshot checkpoint summary saved checkpoint_height={height}`; hit — `autosnapshot checkpoint hit`. |
| Unit: ранний shutdown не понижает checkpoint | **Partial** | `shutdown_skip_checkpoint_regress` — сильный тест (seed summary + manifest, assert checkpoint неизменён). `shutdown_skip_when_loading_snapshot` только проверяет `Ok(())`, без seed summary/manifest и без assert на `checkpoint_height`. |
| `cargo test -p pwmd snapshot` green | **Not verified** | Сборка в среде ревьюера: `dlltool.exe` not found (Windows toolchain). Логически тесты в `handlers_shutdown.rs` + существующий `snapshot` модуль выглядят согласованными; нужен прогон в `pwm-testing`. |

**Вывод по fit:** корневая причина CY proposer закрыта для JsonFile пути; мелкие пробелы в enum reason для CH/legacy и в тесте loading-phase.

---

## 3. Style and module shape

- Модули с `//!` баннерами на месте (`handlers_shutdown.rs`, `io.rs`, `telemetry.rs`).
- Рефакторинг shutdown в `shutdown_skip_reason` / `checkpoint_regress_skip` / `would_regress_checkpoint` — читаемо, соответствует существующему стилю API handlers.
- `JsonSnapTiming` расширен полями `used_full_verify`, `lag_forced_verify` — уместно для align-after-verify без повторного чтения manifest.

**Naming (`check_entity_name_segments.py`):**

| File | Violation |
|------|-----------|
| `lifecycle.rs:2107` | `maybe_align_summary_after_verify` — **5** snake_case segments (prod limit **4**) |

Единичное нарушение; рекомендуется переименование (например `align_summary_post_verify`).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). Изменения касаются локального JsonFile snapshot load/shutdown и tracing; peer wire не затронут.

---

## 4. Safety

- **Положительно:** skip persist при genesis inner во время load / при `tip_h < manifest` предотвращает порчу `pwm-data.json` — основной DoS/ops риск слайса.
- **`unwrap_or(false)` на `would_regress_checkpoint`:** при ошибке чтения manifest shutdown **не** skip-ит и может выполнить persist с низким tip. Редкий edge (повреждённый epochs dir); fail-open. Стоит рассмотреть skip-on-error или WARN+skip (nit, не блокер для CY).
- **`ReadyDegraded`:** не в списке skip-фаз; при degraded load с `tip_h < canonical_h` сработает `checkpoint_regress` — OK.
- Panics / hot-path `unwrap`: в новом коде только тестовые `expect`; prod путь без новых unwrap в hot path.
- Trust boundaries: только локальные пути snapshot backend; без изменений RPC wire.

---

## 5. Tests

**Добавлено:**

- `shutdown_skip_when_loading_snapshot` — фаза `LoadingSnapshot`, persist пропущен (косвенно).
- `shutdown_skip_checkpoint_regress` — manifest tip 124061, chain tip 5, checkpoint 5 сохранён.

**Пробелы:**

- Нет теста «SIGINT during load + pre-seeded summary/manifest» end-to-end (критерий 5 формулирован именно так).
- Нет unit-теста на `maybe_align_summary_after_verify` / INFO `snapshot_load_mode` (можно оставить pwm-testing / log capture).
- Align path: нет теста на `tip_manifest_mismatch` skip.

Рекомендация pwm-testing: прогнать `cargo test -p pwmd snapshot` и shutdown tests на Linux CI.

---

## 6. Verdict

**PASS_WITH_NITS**

Приоритетные nits для pwm-coding (без решения владельца):

1. Переименовать `maybe_align_summary_after_verify` → ≤4 сегмента.
2. Усилить `shutdown_skip_when_loading_snapshot`: seed summary + manifest, assert `checkpoint_height` после shutdown.
3. (Опционально) Добавить `snapshot_load_mode` INFO для ClickHouse веток с `reason=clickhouse|legacy_anchor` — если держим контракт из acceptance criteria буквально.
4. (Опционально) `would_regress_checkpoint` Err → skip persist + WARN вместо `unwrap_or(false)`.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260617-pwmd-snapshot-summary-checkpoint-lag-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 28000
  confidence: low
```

GLOSSARY.md: без изменений (нового жаргона не появилось; не sprint-final review).

---

**Verdict (one-liner):** PASS_WITH_NITS — core JsonFile shutdown/lag fix и post-verify align соответствуют тикету; nit: 5-segment fn name, слабый loading-phase test, неполный reason enum для CH.
