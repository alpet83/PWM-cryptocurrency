# Changelog

Формат: краткие записи по спринтам / версиям документации и контрактов.

## [Unreleased]

### Sprint 14 (план) — wallet schema v3

- **Терминология:** в сериализации wallet v3 человеко-читаемое поле адреса называется **`id_pretty`** вместо исторического **`account_id_human`** в корне YAML (смысл тот же — pretty pwm1-… строка). Обоснование: единая терминологическая база «pretty id»; миграция v2 → v3 копирует значение в новое имя.
- **Спецификация:** [docs/rfc/10-wallet-file-format-v3.md](rfc/10-wallet-file-format-v3.md).
- **Аудит полей v2:** [docs/reviews/sprint-14-wallet-schema-audit.md](reviews/sprint-14-wallet-schema-audit.md).
