# Sprint 14 — Address generation checkup (final review)

## Scope recap
Проверены изменения по четырём направлениям:
- safety semantics (add-by-default, destructive только explicit),
- корректность `addr-derive --wallet-out`,
- корректность `wallet account remove` (guardrails + active fallback),
- согласованность docs/help.

## Findings (by severity)

### Low
- CLI `--help` для `addr-derive` менее подробный по write semantics, чем `docs/pwm-cli.md`.
- Функционально это не баг, но есть UX-риск ожиданий.

Блокирующих дефектов не найдено.

## Requirement fit
- `addr-bruteforce`: append по умолчанию при существующем `--wallet-out`, destructive путь только с `--overwrite-wallet`.
- `addr-derive`: без `--wallet-out` остаётся stateless, с `--wallet-out` делает create/append через безопасный путь.
- `wallet account remove`: есть guardrails (запрет удаления последнего аккаунта, детерминированный fallback active).
- Backward compatibility: сохранена, destructive поведение только explicit.

## Safety
- Критичных регрессий в целевом scope не обнаружено.
- В append-path есть проверка соответствия seed существующему wallet перед добавлением.

## Tests
- По отчёту testing: целевые проверки пройдены и `cargo test -p pwm-cli` зелёный (`117 passed`).

## Verdict
**PASS (approve with low-severity nit)**.
