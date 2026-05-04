# Logger Feasibility Review (Slice 15)

## Verdict
`approve with nits`

## Summary
- Интеграция `p:/opt/docker/trade_report/src/common/basic_logger.rs` в `pwmd` технически возможна.
- Полный перенос "как есть" не рекомендуется: высокий риск усложнения и конфликтов с текущим `tracing`-подходом.

## Recommendation
- Краткосрочно: частичная интеграция (color/file-rotation идеи) поверх `tracing`, без прямого копирования всего логгера.
- Долгосрочно: после стабилизации — общий logging crate с tracing-native API.

## Key risks of full copy
- дополнительные зависимости и lock-contention путь в async runtime,
- смешение custom logger API и `tracing`,
- переносимость/maintenance риски (ANSI, backtrace/path-specific filters).
