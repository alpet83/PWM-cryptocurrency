# Changelog

Формат: краткие записи по спринтам / версиям документации и контрактов.

## [Unreleased]

## [V7-8] (2026-07-02) — conservation transfer UX + wallet tx journal

- **Conservation transfer flow:** finalized the V7-8 conservation sender path: `CONSERVATION` address flag bit 1 routes transfers into the 86400-block delayed queue, exposes `pending_conservation` in `ChainState`, and drains due transfers through `drain_conservation_at_height`.
- **Wallet tx journal:** added the wallet-side `tx-history/<addr>.jsonl` journal with `JournalEntry`, append/write helpers, and `read_journal` so TUI history survives restarts without changing wire/API contracts.
- **TUI status tracking:** journal confirmation now uses `confirm_journal_nonces`; normal sends track nonce advancement, while conservation senders use `track_nonce=false` because the delayed conservation path does not increment nonce in the same way.
- **History overlay:** the `H` overlay now merges persisted journal rows with in-session operations, deduplicates journal-backed session sends, prepends pending conservation rows, converts pending recipient hex to `pwm1-...`, widens the `To` column, and adds an `Info` ETA column for conservation execution height.
- **Detail header and refresh UX:** pending conservation is visible inline in the detail header tail; F6 sends trigger a follow-up poll so newly queued conservation transfers appear after block seal.
- **Post-release fixes:** included the pending-inline positioning corrections, poll-after-F6-send fix, journal/conservation nonce behavior, and the ClickHouse snapshot HTTP field rename fix (`staked_pwm_raw`, `marks_last_block`).
- **Commits:** 59f12c8..8d47e3d.

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