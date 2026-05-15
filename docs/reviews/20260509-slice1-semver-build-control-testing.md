# Slice 1 — тестирование: semver handshake + build-control (2026-05-09)

**Коммиты:** `08cc97d` (coding), `223057f` (обновление тикета).  
**Тикет:** `tasks/20260509-protocol-versioning-debug-controls.json`

## Вердикт: **PASS**

## 1) Major mismatch → отказ handshake

- **Код:** при `local.major != remote.major` → `Err(HandshakeRejectReason::ProtocolVersionMajorMismatch)` (`handshake::protocol_compat`).
- **Транспорт:** `process_incoming_peer_hello` возвращает ошибку с лейблом `protocol_version_major_mismatch`, инкремент метрики `reject_reason_total`.
- **Тесты:** `handshake::tests::compat_major_bad`; `transport::incoming_hello::tests::reject_proto_major_gap`.

Ключевые строки логики отклонения (деталь + метрики):

```59:78:crates/pwmd/src/transport/incoming_hello.rs
                Err(reason) => {
                    hs.metrics.rejected_total = hs.metrics.rejected_total.saturating_add(1);
                    increment_reject_reason_total(&mut hs.metrics, reason.as_label());
                    let detail = if matches!(
                        reason,
                        crate::handshake::HandshakeRejectReason::ProtocolVersionMajorMismatch
                    ) {
                        format!(
                            "protocol_version_major_mismatch expected_version={} received_version={}",
                            PWM_PROTOCOL_VERSION, hello.capabilities.protocol_version
                        )
                    } else {
                        // ...
                    };
                    let reason_label = reason.as_label();
                    return Err(reject_guard(hs, reason_label, detail));
                }
```

## 2) Minor / patch mismatch → только предупреждение, соединение допускается

- **Код:** `protocol_compat` возвращает `Ok(ProtocolCompat::FractionalMismatch)` при расхождении minor или patch при совпадающем major; в `incoming_hello` это ветка `warn!` без `return Err`.
- **Тесты:** `handshake::tests::compat_minor_warn` (`0.2.0` vs локальный `0.1.0`); `incoming_hello::tests::accept_proto_minor_gap`.
- **Замечание:** отдельного интеграционного теста только на **patch** (например `0.1.1`) нет; ветка та же, что для minor — покрытие по коду полное, по сценарию — умеренный остаточный риск.

## 3) Build-control marker: путь к бинарю и mtime

- **Код:** `log_build_control` пишет строку с `binary_path=...` и `binary_mtime_utc_unix=...` (мс с эпохи), плюс `marker` и `pid`.
- **Тесты (бинарь / main):** `tests::binary_meta_reads_mtime` (mtime заканчивается на `ms`), `tests::binary_meta_marks_missing`, `tests::build_ctl_marker_has_ver`.

```493:503:crates/pwmd/src/main.rs
fn log_build_control(log: pwmd::NodeLogger) {
    match std::env::current_exe() {
        Ok(path) => {
            let (path_field, mtime_field) = binary_meta_fields(path.as_path());
            log.info(&format!(
                "build control marker={} binary_path={} binary_mtime_utc_unix={} pid={}",
                build_marker(),
                path_field,
                mtime_field,
                std::process::id()
            ));
```

## 4) Регрессии в затронутых transport-путях

- Прогон модульных тестов для `transport::incoming_hello` (5 тестов, включая semver и прежние сценарии cluster/sync) — все **ok**.
- Изменённый `dial.rs`: `broke_trust_overrides_genesis` — **ok**.

## Команды

```text
cargo test -p pwmd -- compat_major_bad compat_minor_warn reject_proto_major accept_proto_minor binary_meta_reads_mtime binary_meta_marks_missing build_ctl_marker -- --nocapture
# lib: 4 passed; main: 3 passed

cargo test -p pwmd incoming_hello::
# 5 passed

cargo test -p pwmd broke_trust_overrides_genesis
# 1 passed

cargo check -p pwmd
# Finished dev profile (ok)
```

Префлайт `target/debug` по политике оркестратора не запускался: сборка шла в существующий `rust-target-shared` без очистки артефакта.

## Краткие доказательства (PASS)

- Major: `compat_major_bad` + `reject_proto_major_gap` → **ok**.
- Minor warn / accept: `compat_minor_warn` + `accept_proto_minor_gap` → **ok**.
- Build marker: строка лога содержит `binary_path` и `binary_mtime_utc_unix`; юниты `binary_meta_*` → **ok**.
- Transport smoke: весь модуль `incoming_hello::` + dial trust test → **ok**.
