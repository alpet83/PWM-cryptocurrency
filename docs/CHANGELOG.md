# Changelog

Формат: краткие записи по спринтам / версиям документации и контрактов.

## [Unreleased]

### V5-8 (2026-05-28) — integrated devnet gate + MVP v5 closeout

- **Operator smoke harness:** `scripts/devnet_v5_operator_smoke.ps1` covers all four V5 feature lanes — marks/inflation growth, deferred policy activation (height-gated), `ClaimIPv4Batch` IPv4 claim with registry signature, and `pwm account-info` marks output (effective_marks, marks_last_block, saturation %).
- **Smoke results:** slices 1–4 all PASS on live devnet; PASS_EVIDENCE tokens recorded in `tmp/devnet_v5_operator_smoke_20260524_192234.md`, `20260525_143518.md`, `20260528_080852.md`, `20260528_085451.md`.
- **Commits:** fd94191 (slice1 marks smoke), c930024 (slice2 deferred smoke), f5d4535 (slice3 ipv4-claim smoke fix), f21f243 (slice4 account-info smoke).
- **Docs closure:** `docs/MVP-checklist.md` §0v5 V5-8 row marked `[x]`; `docs/GLOSSARY.md` updated with V5 tokenomics terms; final review `docs/reviews/20260524-v5-sprint8-closeout-review.md` verdict PASS.
- **No product Rust changes** in slice5 — docs/tasks only.

### Sprint 14 (план) — wallet schema v3

- **Терминология:** в сериализации wallet v3 человеко-читаемое поле адреса называется **`id_pretty`** вместо исторического **`account_id_human`** в корне YAML (смысл тот же — pretty pwm1-… строка). Обоснование: единая терминологическая база «pretty id»; миграция v2 → v3 копирует значение в новое имя.
- **Спецификация:** [docs/rfc/10-wallet-file-format-v3.md](rfc/10-wallet-file-format-v3.md).
- **Аудит полей v2:** [docs/reviews/sprint-14-wallet-schema-audit.md](reviews/sprint-14-wallet-schema-audit.md).
