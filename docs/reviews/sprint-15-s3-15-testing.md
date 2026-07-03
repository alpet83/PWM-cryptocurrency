# Sprint 15 S3.15 — testing handoff

## Environment

- Repo: `P:/opt/docker/pwm-protocol`
- Date: 2026-05-01

## Commands

| Command | Result | Notes |
|---------|--------|-------|
| `cargo fmt --check -p pwmd` | PASS | |
| `cargo fmt --check -p pwm-tui` | PASS | |
| `cargo test -p pwmd --lib` | PASS | 192 tests |
| `cargo check -p pwm-tui` | PASS | |

После правки ревью (snippet в `RelayErr`): `cargo test -p pwmd --lib` — PASS (192).

## Smoke (ручной)

Из корня репозитория: два окна PowerShell — `.\node-1.ps1` и `.\node-2.ps1`; genesis и пароли как в скриптах; остановка Ctrl+C. Проверить межшардовый перевод и шаг 3 TUI при сбое relay — в логах `pwmd` должны быть строки `relay:` с intent/export id.

---

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-15-testing.md
token_usage:
  source: estimate
  total: 8000
  confidence: low
```
