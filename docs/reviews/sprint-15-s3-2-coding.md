# Sprint 15 S3.2 Coding

## Scope
- `pwm-tui`: интерактивный step-flow для cross-shard send в форме F6.
- Без изменений протокола и без изменений контрактов `pwmd`.

## Implemented
- Добавлен UI step-flow после `SubmitDone`: показывает шаги по одному, с продолжением по `Enter` или авто-переходом через 5 секунд.
- На `Err` форма больше не закрывается автоматически; закрытие только по `Esc`.
- Добавлен guard против `Enter`-replay: пока step-flow активен или RPC submit in-flight, повторный submit не стартует.
- Диагностика S15-S3.1 сохранена как staged-steps и показана в более читаемом многострочном статусе.
- Для успешной отправки book-prompt отложен до `Esc` (после закрытия формы), чтобы не прерывать step-flow.

## Remediation (S15-S3.2 review)
- После failed step-flow включён явный lock: до `Esc` новая отправка не стартует.
- В locked failed-состоянии `Enter` больше не перезапускает submit и не шлёт повторно.
- Отложенный `book_prompt` теперь сохраняется до корректного close-handling (`Esc`) и не теряется из-за blocked/retry попыток.

## Tests
- `send_replay_guard_blocks_when_step_flow_is_active`
- `send_step_flow_auto_advances_after_timeout`
- `submit_error_keeps_form_open_until_escape`
- `failed_flow_lock_blocks_replay_until_escape`
- `enter_is_blocked_when_failed_flow_is_locked`
- `pending_prompt_survives_until_close_handling`

## Validation
- `cargo fmt`
- `cargo test -p pwm-tui`
