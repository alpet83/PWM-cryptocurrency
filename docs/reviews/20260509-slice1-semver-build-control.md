# Slice 1: semver guard + build control

Дата: 2026-05-09  
Тикет: `tasks/20260509-protocol-versioning-debug-controls.json`

## Что сделано

1. **Build control logging на старте `pwmd`:**
   - после инициализации логгера добавлен startup-marker:
     - `marker=pwmd/<cargo_pkg_version>[+ts:<PWM_BUILD_TIMESTAMP_UTC>][+git:<PWM_GIT_SHA>]`
     - `binary_path=<absolute path>`
     - `binary_mtime_utc_unix=<unix_ms|unavailable>`
     - `pid=<process id>`
   - если `current_exe`/metadata недоступны, startup не падает: пишется `warn` с `binary_mtime_utc_unix=unavailable`.

2. **Protocol semver compatibility guard в handshake пути:**
   - введён `handshake::PWM_PROTOCOL_VERSION` (локальная wire-версия).
   - добавлен semver parser `major.minor.patch` и проверка совместимости:
     - major mismatch -> reject/close c reason `protocol_version_major_mismatch`;
     - malformed remote version -> reject c reason `protocol_version_malformed`;
     - minor/patch mismatch -> **не reject**, `warn`:
       `protocol_version_fractional_mismatch`.
   - в `build_local_node_hello()` убрана хардкод-строка версии, используется `PWM_PROTOCOL_VERSION`.

3. **Metrics/reason labels:**
   - добавлены стабильные labels:
     - `protocol_version_major_mismatch`
     - `protocol_version_malformed`
   - major/malformed reject учитываются в `reject_reason_total`.

4. **Bump discipline для subagents:**
   - `docs/AGENT_PROMPT_coding.md`: добавлен обязательный блок `Protocol semver bump discipline`.
   - `docs/AGENT_PROMPT_review.md`: добавлен review-check на явную semver decision при wire-изменениях.

## Примеры поведения

- **Reject (major mismatch):**
  - local `PWM_PROTOCOL_VERSION=0.1.0`, remote `1.0.0`
  - handshake: `HelloAck { accepted: false, reason: "protocol_version_major_mismatch" }`
  - peer error detail: `expected_version=0.1.0 received_version=1.0.0`

- **Warn-only (fractional mismatch):**
  - local `0.1.0`, remote `0.2.0`
  - handshake accepted, лог `protocol_version_fractional_mismatch`

- **Build control marker:**
  - пример: `build control marker=pwmd/0.1.52+git:abc1234 binary_path=/.../pwmd binary_mtime_utc_unix=1746778335123ms pid=12345`

## Тестовое покрытие (Slice 1)

- semver parser/compat unit tests (`handshake.rs`):
  - parse ok/bad;
  - major mismatch reject;
  - minor mismatch warn-path compatibility.
- inbound hello tests (`transport/incoming_hello.rs`):
  - major mismatch reject + reason bucket increment;
  - minor mismatch accept.
- startup marker helper tests (`main.rs`):
  - marker содержит версию;
  - mtime helper корректно обрабатывает missing path;
  - mtime helper читает timestamp для существующего файла.
