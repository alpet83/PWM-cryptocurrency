## Sprint 15 Slice 3.2 Review

## Verdict
`approve with nits`

## Remediation result
1. Добавлен failed-flow lock до ESC: повторный submit по ENTER в failed-состоянии блокируется.
2. Pending `book_prompt` сохраняется до close-handling и показывается после ESC.
3. Добавлены и пройдены целевые тесты на lock/replay/prompt lifecycle.

## Nits
- Добавить отдельный тест на сценарий нескольких успешных submit подряд без ESC, чтобы явно зафиксировать контракт обработки нескольких pending prompt.
