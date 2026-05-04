# Review: peer_session seed split (`0b1df134`)

**Commit:** `0b1df13443b738a71c7308c77c77b5dd874e8e98`  
**Scope:** `peer_session/seed.rs` → `peer_session/seed/{mod,connect,handshake,session}.rs`; `peer_session/mod.rs` остаётся `mod seed;` (корректно подхватывается `seed/mod.rs`).

_Источник: независимый pwm-review; файл записан оркестратором (субагент не мог сохранить в Ask mode)._

## 1. Scope recap

- Задача: поведение-сохраняющее разбиение большого seed-файла на этапы connect / handshake / connected session без смены внешнего контракта для вызывающего кода.
- Связь с MVP: `docs/reviews/sprint-15-slice-O-checklist.md` §C transport §2.2 continuation.

## 2. Requirements fit

- Публичная точка входа не меняется: **`pub(crate) async fn run_seed_session(...)`** в `seed/mod.rs`, как было в монолите.
- Порядок этапов в цикле: init → **`now_ms`** → TCP (или sticky skip) → handshake (включая **`merge_remote_hello`**) → фаза `send_cross_shard_facts` / `send_account_views` и далее совпадает с исходной последовательностью после переноса `merge_remote_hello` в конец **`seed_finish_handshake`** перед `Some(remote)` — затем модуль **`session`** отправляет те же сообщения с тем же **`now_ms`**, переданным снаружи.
- Внешний контракт для `transport` / spawn: экспорт только **`run_seed_session`** через существующий `mod seed;`.

## 3. Style

- Разбиение по смыслу (подключение / рукопожатие / сессия) читается естественно; краткий модульный doc в `seed/mod.rs`.
- Имена вспомогательных функций (`seed_try_tcp_connect`, `seed_finish_handshake`, `seed_run_connected_session`) укладываются в политику коротких имён.
- Импорты: паттерн `super::super::super::*` из подмодулей `seed/connect|handshake|session` соответствует прежнему `use super::super::*` из бывшего `peer_session/seed.rs`.

## 4. Safety

- Рефакторинг без смены крипто-/wire-контракта по диффу: те же **`write_wire_msg`** / **`read_wire_msg`**, те же счётчики **`session_retrying_total`**, **`session_connected_total`**, **`session_trusted_total`** в тех же ветках.
- Лишнего расширения поверхности нет: вспомогательные функции — **`pub(super)`**, доступны только дереву **`seed`**.
- Новых **`unwrap`** или паник в горячих путях по сравнению с исходником не добавлено.

## 5. Tests

- Полный прогон не дублировался: **pwm-testing PASS** (`cargo fmt`, `cargo test -p pwmd`, `cargo check --workspace`).

## 6. Semantic drift checklist

Существенного дрейфа не найдено: порядок `await`, `now_ms`, метрики / reconnect / close, эквивалентность `HashMap` lookup `get(seed_key)` для `&str`.

## 7. Verdict

**APPROVE**
